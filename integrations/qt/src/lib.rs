//! Generate the palette consumed by the Coconut Qt platform-theme plugin.
use coconut_theme::{AdapterBase, Color, ResolvedTheme, Scheme, Store};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const ROLES: &[&str] = &[
    "Window",
    "WindowText",
    "Base",
    "AlternateBase",
    "ToolTipBase",
    "ToolTipText",
    "Text",
    "Button",
    "ButtonText",
    "BrightText",
    "Light",
    "Midlight",
    "Dark",
    "Mid",
    "Shadow",
    "Highlight",
    "HighlightedText",
    "Link",
    "LinkVisited",
    "PlaceholderText",
    "Accent",
];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NativeAppearance {
    pub schema: u32,
    pub theme: String,
    pub variant: String,
    pub color_scheme: String,
    pub high_contrast: bool,
    pub font_family: String,
    pub font_point_size: f64,
    pub palette: BTreeMap<String, BTreeMap<String, String>>,
    pub stylesheet: String,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PaletteOverrides {
    base: Option<AdapterBase>,
    active: BTreeMap<String, Color>,
    inactive: BTreeMap<String, Color>,
    disabled: BTreeMap<String, Color>,
}

// The shared format uses #RRGGBBAA, while QColor uses #AARRGGBB.
fn qcolor(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}{:02x}", c.a, c.r, c.g, c.b)
}

pub fn generate(theme: &ResolvedTheme) -> Result<NativeAppearance, String> {
    let adapter = &theme.adapters["qt"];
    let mut overrides: PaletteOverrides = adapter
        .palette
        .clone()
        .map(|v| v.try_into().map_err(|e| format!("invalid Qt palette: {e}")))
        .transpose()?
        .unwrap_or_default();
    let mode = overrides.base.unwrap_or(adapter.base);
    let mut palette = BTreeMap::new();
    let c = &adapter.tokens.colors;
    let fallback = if theme.scheme == Scheme::Dark {
        Color::rgb(0, 0, 0)
    } else {
        Color::rgb(255, 255, 255)
    };
    let surface = c.surface.opaque_on(fallback);
    let elevated = c.surface_elevated.opaque_on(surface);
    let common = BTreeMap::from([
        ("Window", surface),
        ("WindowText", c.text_primary),
        ("Base", elevated),
        ("AlternateBase", c.surface_hover.opaque_on(elevated)),
        ("ToolTipBase", elevated),
        ("ToolTipText", c.text_primary),
        ("Text", c.text_primary),
        ("Button", elevated),
        ("ButtonText", c.text_primary),
        ("BrightText", c.on_accent),
        ("Light", surface.mix(Color::rgb(255, 255, 255), 0.2)),
        ("Midlight", surface.mix(Color::rgb(255, 255, 255), 0.1)),
        ("Dark", surface.mix(Color::rgb(0, 0, 0), 0.2)),
        ("Mid", c.border.opaque_on(surface)),
        ("Shadow", Color::rgb(0, 0, 0)),
        ("Highlight", c.accent),
        ("HighlightedText", c.on_accent),
        ("Link", c.accent),
        ("LinkVisited", c.accent_pressed),
        ("PlaceholderText", c.text_secondary),
        ("Accent", c.accent),
    ]);
    for (group, custom) in [
        ("Active", &mut overrides.active),
        ("Inactive", &mut overrides.inactive),
        ("Disabled", &mut overrides.disabled),
    ] {
        let mut values: BTreeMap<String, String> = if mode == AdapterBase::Auto {
            common
                .iter()
                .map(|(role, color)| ((*role).into(), qcolor(*color)))
                .collect()
        } else {
            BTreeMap::new()
        };
        if mode == AdapterBase::Auto {
            let text = if group == "Disabled" {
                c.text_disabled
            } else if group == "Inactive" {
                c.text_secondary
            } else {
                c.text_primary
            };
            for role in ["WindowText", "Text", "ButtonText"] {
                values.insert(role.into(), qcolor(text));
            }
            if group == "Disabled" {
                values.insert("Highlight".into(), qcolor(c.accent.mix(surface, 0.65)));
            }
        }
        for (role, color) in std::mem::take(custom) {
            if !ROLES.contains(&role.as_str()) {
                return Err(format!("unknown Qt palette role {role}"));
            }
            values.insert(role, qcolor(color));
        }
        for role in ROLES {
            if !values.contains_key(*role) {
                return Err(format!("manual Qt palette is missing {group}.{role}"));
            }
        }
        palette.insert(group.into(), values);
    }
    let typography = &adapter.tokens.typography;
    let family = typography
        .family
        .split(',')
        .next()
        .unwrap_or("Sans")
        .trim()
        .trim_matches(['\'', '"']);
    let family = if matches!(family, "system-ui" | "sans-serif" | "") {
        "Sans"
    } else {
        family
    };
    Ok(NativeAppearance {
        schema: 1,
        theme: theme.theme_id.clone(),
        variant: theme.variant_id.clone(),
        color_scheme: theme.scheme.as_str().into(),
        high_contrast: theme.selection.accessibility.high_contrast,
        font_family: family.into(),
        font_point_size: f64::from(typography.body_size) * 0.75,
        palette,
        stylesheet: adapter.stylesheet.clone().unwrap_or_default(),
    })
}

pub fn publish(store: &Store, theme: &ResolvedTheme) -> Result<bool, String> {
    let source = serde_json::to_string_pretty(&generate(theme)?).map_err(|e| e.to_string())?;
    coconut_theme::atomic_write(&store.generated_dir().join("qt.json"), &source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_theme::{Package, Selection};
    #[test]
    fn automatic_palette_covers_every_group_and_tracks_accent() {
        let mut selection = Selection::default();
        selection.overrides.accent = Some(Color::rgb(255, 0, 255));
        let resolved = Package::builtin().unwrap().resolve(&selection).unwrap();
        let qt = generate(&resolved).unwrap();
        assert_eq!(qt.color_scheme, "dark");
        assert_eq!(qt.palette["Active"]["Highlight"], "#ffff00ff");
        assert_eq!(qt.palette.len(), 3);
        assert!(qt.palette.values().all(|v| v.len() == ROLES.len()));
        assert_ne!(qt.palette["Active"]["Text"], qt.palette["Disabled"]["Text"]);
    }
    #[test]
    fn manual_roles_override_auto_but_invalid_roles_and_incomplete_manual_fail() {
        let mut resolved = Package::builtin()
            .unwrap()
            .resolve(&Selection::default())
            .unwrap();
        let adapter = resolved.adapters.get_mut("qt").unwrap();
        adapter.palette = Some(toml::from_str("[active]\nWindow = '#123456'\n").unwrap());
        assert_eq!(
            generate(&resolved).unwrap().palette["Active"]["Window"],
            "#ff123456"
        );
        resolved.adapters.get_mut("qt").unwrap().base = AdapterBase::Manual;
        assert!(generate(&resolved).unwrap_err().contains("missing"));
        resolved.adapters.get_mut("qt").unwrap().base = AdapterBase::Auto;
        resolved.adapters.get_mut("qt").unwrap().palette =
            Some(toml::from_str("[active]\nNotARole = '#fff'\n").unwrap());
        assert!(generate(&resolved).unwrap_err().contains("unknown"));
    }
}
