use coconut_theme::{AdapterBase, Color, ResolvedTheme, Scheme};
use delib::xdg::XdgPaths;
use std::{
    collections::HashSet,
    fs,
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

const SCHEMA: &str = "org.gnome.desktop.interface";
const A11Y_SCHEMA: &str = "org.gnome.desktop.a11y.interface";
const SLOTS: [&str; 2] = ["Coconut", "Coconut-Alt"];

pub fn dark(appearance: &ResolvedTheme) -> bool {
    appearance.scheme == Scheme::Dark
}

pub fn font_name(appearance: &ResolvedTheme) -> String {
    let family = appearance.adapters["gtk"]
        .tokens
        .typography
        .family
        .as_str()
        .split(',')
        .next()
        .unwrap_or("Sans")
        .trim()
        .trim_matches(['\'', '"']);
    let family = match family {
        "" | "system-ui" | "sans-serif" => "Sans",
        "serif" => "Serif",
        "monospace" => "Monospace",
        other => other,
    };
    let size = appearance.adapters["gtk"].tokens.typography.body_size
        / appearance.selection.accessibility.text_scale
        * 0.75;
    format!("{} {:.2}", family.replace(['\n', '\r', '\0'], ""), size)
}

pub struct Bridge {
    paths: XdgPaths,
    keys: HashSet<String>,
    a11y_keys: HashSet<String>,
    xsettings: Option<Child>,
    xsettings_attempted: bool,
}

impl Bridge {
    pub fn new(paths: XdgPaths) -> Self {
        let keys = match command("gsettings", &["list-keys", SCHEMA]) {
            Ok(keys) => keys.lines().map(str::to_owned).collect(),
            Err(error) => {
                eprintln!("coconut-appearance: GSettings unavailable (install glib2, dconf and gsettings-desktop-schemas): {error}");
                HashSet::new()
            }
        };
        let a11y_keys = command("gsettings", &["list-keys", A11Y_SCHEMA])
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        Self {
            paths,
            keys,
            a11y_keys,
            xsettings: None,
            xsettings_attempted: false,
        }
    }

    pub fn sync(&mut self, appearance: &ResolvedTheme) -> Result<String, String> {
        let name = self
            .write_theme(appearance)
            .map_err(|error| format!("could not generate GTK theme: {error}"))?;
        let font = font_name(appearance);
        let entries = [
            ("gtk-theme-name", name.clone()),
            (
                "gtk-application-prefer-dark-theme",
                dark(appearance).to_string(),
            ),
            ("gtk-font-name", font.clone()),
        ];
        for version in ["gtk-3.0", "gtk-4.0"] {
            let path = self.paths.config_home.join(version).join("settings.ini");
            let result = (|| {
                let source = match fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(error) => return Err(error),
                };
                write_if_changed(&path, &update_ini(&source, &entries))
            })();
            if let Err(error) = result {
                eprintln!("coconut-appearance: {}: {error}", path.display());
            }
        }
        self.set(SCHEMA, "gtk-theme", &quote(&name));
        self.set(
            SCHEMA,
            "color-scheme",
            &quote(if dark(appearance) {
                "prefer-dark"
            } else {
                "prefer-light"
            }),
        );
        self.set(SCHEMA, "font-name", &quote(&font));
        self.set(
            SCHEMA,
            "text-scaling-factor",
            &f64::from(appearance.selection.accessibility.text_scale).to_string(),
        );
        self.set(
            A11Y_SCHEMA,
            "high-contrast",
            &appearance.selection.accessibility.high_contrast.to_string(),
        );

        // X11 GTK clients receive XSettings rather than the Wayland GSettings
        // notifications. Only signal the xsettingsd process owned by us.
        let xconfig = format!(
            "Net/ThemeName {}\nGtk/FontName {}\nXft/DPI {}\n",
            quote_xsettings(&name),
            quote_xsettings(&font),
            (96.0 * 1024.0 * appearance.selection.accessibility.text_scale).round() as u32
        );
        match write_if_changed(
            &self.paths.user_config_file("coconut", "xsettingsd.conf"),
            &xconfig,
        ) {
            Ok(true) => {
                if let Some(child) = &self.xsettings {
                    // SAFETY: child is the process we spawned and have not reaped.
                    unsafe {
                        libc::kill(child.id() as i32, libc::SIGHUP);
                    }
                }
            }
            Ok(false) => {}
            Err(error) => eprintln!("coconut-appearance: XSettings config: {error}"),
        }
        self.maintain_xsettings();
        Ok(name)
    }

    fn set(&self, schema: &str, key: &str, value: &str) {
        let keys = if schema == SCHEMA {
            &self.keys
        } else {
            &self.a11y_keys
        };
        if !keys.contains(key) {
            return;
        }
        if command("gsettings", &["get", schema, key]).is_ok_and(|current| current == value) {
            return;
        }
        if let Err(error) = command("gsettings", &["set", schema, key, value]) {
            eprintln!("coconut-appearance: cannot set {schema}.{key}: {error}");
        }
    }

    fn write_theme(&self, appearance: &ResolvedTheme) -> std::io::Result<String> {
        let marker = self.paths.user_config_file("coconut", "gtk-theme");
        let saved = fs::read_to_string(&marker).unwrap_or_default();
        let current = if SLOTS.contains(&saved.trim()) {
            saved.trim()
        } else {
            SLOTS[0]
        };
        let css3 = stylesheet(appearance, 3);
        let css4 = stylesheet(appearance, 4);
        let themes = self.paths.data_home.join("themes");
        if fs::read_to_string(themes.join(current).join("gtk-3.0/gtk.css"))
            .ok()
            .as_deref()
            == Some(&css3)
            && fs::read_to_string(themes.join(current).join("gtk-4.0/gtk.css"))
                .ok()
                .as_deref()
                == Some(&css4)
        {
            return Ok(current.into());
        }
        // Alternate two bounded slots so changing only an accent still changes
        // gtk-theme-name and makes running GTK apps reload their stylesheet.
        let next = if current == SLOTS[0] {
            SLOTS[1]
        } else {
            SLOTS[0]
        };
        for (version, css) in [("gtk-3.0", css3), ("gtk-4.0", css4)] {
            for file in ["gtk.css", "gtk-dark.css"] {
                write_if_changed(&themes.join(next).join(version).join(file), &css)?;
            }
        }
        write_if_changed(&themes.join(next).join("index.theme"),
            &format!("[Desktop Entry]\nType=X-GNOME-Metatheme\nName=Coconut\n[X-GNOME-Metatheme]\nGtkTheme={next}\n"))?;
        write_if_changed(&marker, &format!("{next}\n"))?;
        Ok(next.into())
    }

    pub fn maintain_xsettings(&mut self) {
        if let Some(child) = &mut self.xsettings {
            if matches!(child.try_wait(), Ok(Some(_))) {
                self.xsettings = None;
            }
        }
        if self.xsettings.is_some()
            || self.xsettings_attempted
            || std::env::var_os("DISPLAY").is_none()
        {
            return;
        }
        self.xsettings_attempted = true;
        match Command::new("xsettingsd")
            .arg("--config")
            .arg(self.paths.user_config_file("coconut", "xsettingsd.conf"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
        {
            Ok(child) => self.xsettings = Some(child),
            Err(error) => {
                eprintln!("coconut-appearance: X11 live updates require xsettingsd: {error}")
            }
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        if let Some(child) = &mut self.xsettings {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn stylesheet(appearance: &ResolvedTheme, version: u8) -> String {
    let suffix = if dark(appearance) { "-dark" } else { "" };
    let resource = if version == 3 {
        format!("Adwaita/gtk-contained{suffix}.css")
    } else {
        format!("Default/gtk{suffix}.css")
    };
    let adapter = &appearance.adapters["gtk"];
    let extra = if version == 3 {
        adapter.gtk3.as_deref()
    } else {
        adapter.gtk4.as_deref()
    }
    .unwrap_or("");
    if adapter.base == AdapterBase::Manual {
        return extra.to_owned();
    }
    let mut c = adapter.tokens.colors.clone();
    let background = if dark(appearance) {
        Color::rgb(0, 0, 0)
    } else {
        Color::rgb(255, 255, 255)
    };
    c.surface = c.surface.opaque_on(background);
    c.surface_elevated = c.surface_elevated.opaque_on(c.surface);
    c.surface_hover = c.surface_hover.opaque_on(c.surface_elevated);
    c.border = c.border.opaque_on(c.surface);
    c.border_strong = c.border_strong.opaque_on(c.surface);
    c.selection_background = c.accent;
    c.selection_text = c.on_accent;
    let mut css = format!("/* Generated by Coconut. Edit cream/appearance.toml instead. */\n@import url(\"resource:///org/gtk/libgtk/theme/{resource}\");\n");
    for (name, color) in [
        ("theme_bg_color", c.surface),
        ("theme_fg_color", c.text_primary),
        ("theme_base_color", c.surface_elevated),
        ("theme_text_color", c.text_primary),
        ("theme_selected_bg_color", c.selection_background),
        ("theme_selected_fg_color", c.selection_text),
        ("theme_unfocused_bg_color", c.surface),
        ("theme_unfocused_fg_color", c.text_secondary),
        ("theme_unfocused_base_color", c.surface_elevated),
        ("theme_unfocused_text_color", c.text_secondary),
        ("theme_unfocused_selected_bg_color", c.selection_background),
        ("theme_unfocused_selected_fg_color", c.selection_text),
        ("insensitive_fg_color", c.text_disabled),
        ("insensitive_bg_color", c.surface),
        ("borders", c.border),
        ("unfocused_borders", c.border),
        ("accent_color", c.accent),
        ("accent_bg_color", c.accent),
        ("accent_fg_color", c.selection_text),
        ("window_bg_color", c.surface),
        ("window_fg_color", c.text_primary),
        ("view_bg_color", c.surface_elevated),
        ("view_fg_color", c.text_primary),
        ("headerbar_bg_color", c.surface_elevated),
        ("headerbar_fg_color", c.text_primary),
        ("card_bg_color", c.surface_elevated),
        ("card_fg_color", c.text_primary),
        ("popover_bg_color", c.surface_elevated),
        ("popover_fg_color", c.text_primary),
        ("dialog_bg_color", c.surface_elevated),
        ("dialog_fg_color", c.text_primary),
        ("warning_color", c.warning),
        ("error_color", c.danger),
        ("success_color", c.success),
    ] {
        css.push_str(&format!("@define-color {name} {};\n", rgb(color)));
    }
    css.push_str(&format!(r#"
window, .background {{ background-color: {}; color: {}; }}
headerbar, popover, menu {{ background-color: {}; color: {}; }}
entry, textview.view, treeview.view {{ background-color: {}; color: {}; }}
button:not(.flat):not(.suggested-action):not(.destructive-action) {{ background-image: none; background-color: {}; color: {}; border-color: {}; }}
button:hover {{ background-image: none; background-color: {}; }}
button.suggested-action, selection {{ background-image: none; background-color: {}; color: {}; }}
button.suggested-action:hover {{ background-color: {}; }}
button.suggested-action:active {{ background-color: {}; }}
entry, button {{ border-radius: {}px; }}
*:disabled {{ color: {}; }}
"#, rgb(c.surface), rgb(c.text_primary), rgb(c.surface_elevated), rgb(c.text_primary),
        rgb(c.surface_elevated), rgb(c.text_primary), rgb(c.surface_elevated), rgb(c.text_primary),
        rgb(c.border), rgb(c.surface_hover), rgb(c.accent), rgb(c.selection_text),
        rgb(c.accent_hover), rgb(c.accent_pressed), appearance.adapters["gtk"].tokens.geometry.control_radius, rgb(c.text_disabled)));
    css.push_str(extra);
    css
}

fn rgb(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Update owned keys only, preserving other sections, settings, and comments.
fn update_ini(source: &str, entries: &[(&str, String)]) -> String {
    let mut output = Vec::new();
    let mut in_settings = false;
    let mut found = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_settings = trimmed == "[Settings]";
            if in_settings && !found {
                output.push(line.to_owned());
                output.extend(entries.iter().map(|(key, value)| format!("{key}={value}")));
                found = true;
                continue;
            }
        }
        if in_settings
            && trimmed
                .split_once('=')
                .is_some_and(|(key, _)| entries.iter().any(|(owned, _)| *owned == key.trim()))
        {
            continue;
        }
        output.push(line.to_owned());
    }
    if !found {
        output.push("[Settings]".into());
        output.extend(entries.iter().map(|(key, value)| format!("{key}={value}")));
    }
    output.join("\n") + "\n"
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn quote_xsettings(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn write_if_changed(path: &Path, contents: &str) -> std::io::Result<bool> {
    coconut_theme::atomic_write(path, contents).map_err(std::io::Error::other)
}

fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error.to_string())?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < Duration::from_secs(2) => {
                std::thread::sleep(Duration::from_millis(10))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} timed out"));
            }
        }
    };
    let mut output = String::new();
    if status.success() {
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .map_err(|error| error.to_string())?;
        Ok(output.trim().to_owned())
    } else {
        child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .map_err(|error| error.to_string())?;
        Err(output.trim().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_theme::{Package, Selection};
    fn appearance(variant: &str) -> ResolvedTheme {
        Package::builtin()
            .unwrap()
            .resolve(&Selection {
                variant: variant.into(),
                ..Selection::default()
            })
            .unwrap()
    }
    #[test]
    fn native_styles_flatten_translucent_tokens_and_preserve_polarity() {
        for (variant, expected) in [("light", false), ("dark", true), ("midnight", true)] {
            let a = appearance(variant);
            assert_eq!(dark(&a), expected);
            for version in [3, 4] {
                let css = stylesheet(&a, version);
                assert!(css.contains("@define-color accent_bg_color #0099d3"));
                assert!(css.contains(if expected { "-dark.css" } else { ".css\"" }));
                assert!(!css.contains("background-color: #ffffff; }\nbutton.suggested-action"));
            }
        }
    }
    #[test]
    fn ini_preserves_other_keys_and_sections_and_is_idempotent() {
        let source = "# prefs\n[Settings]\ngtk-icon-theme-name=Custom\ngtk-theme-name=Old\n[Other]\ngtk-theme-name=Keep\n[Settings]\ngtk-theme-name=Duplicate\n";
        let entries = [("gtk-theme-name", "Coconut".into())];
        let updated = update_ini(source, &entries);
        assert!(updated.contains("# prefs"));
        assert!(updated.contains("gtk-icon-theme-name=Custom"));
        assert!(updated.contains("[Other]\ngtk-theme-name=Keep"));
        assert!(!updated.contains("Duplicate"));
        assert_eq!(updated.matches("gtk-theme-name=Coconut").count(), 1);
        assert_eq!(update_ini(&updated, &entries), updated);
    }
    #[test]
    fn bounded_theme_slots_force_reload_on_accent_only_changes() {
        let root = tempfile::tempdir().unwrap();
        let paths = XdgPaths {
            config_home: root.path().join("config"),
            data_home: root.path().join("data"),
            config_dirs: vec![],
            data_dirs: vec![],
        };
        let bridge = Bridge {
            paths,
            keys: HashSet::new(),
            a11y_keys: HashSet::new(),
            xsettings: None,
            xsettings_attempted: true,
        };
        let a = appearance("light");
        let first = bridge.write_theme(&a).unwrap();
        assert_eq!(bridge.write_theme(&a).unwrap(), first);
        let mut selection = a.selection;
        selection.overrides.accent = Some(Color::rgb(255, 0, 0));
        let a = Package::builtin().unwrap().resolve(&selection).unwrap();
        let second = bridge.write_theme(&a).unwrap();
        assert_ne!(first, second);
        assert_eq!(bridge.write_theme(&appearance("midnight")).unwrap(), first);
        assert_eq!(
            fs::read_dir(bridge.paths.data_home.join("themes"))
                .unwrap()
                .count(),
            2
        );
        assert!(!bridge.paths.config_home.join("gtk-4.0/gtk.css").exists());
    }
}
