//! File-backed Blair settings.  Blair watches this TOML itself; Settings does
//! not use its optional D-Bus API to apply compositor configuration.
use crate::common::{group, row, section};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, SegmentedControl, Slider, Switch, TextArea,
};
use std::path::PathBuf;

const DEFAULT: &str = r#"
[general]
backend = "auto"
hot_reload = true
[focus]
raise_on_focus = true
focus_new_windows = true
focus_previous_on_close = true
warp_cursor = false
[animations]
enabled = true
[animations.window_open]
duration = 150
[animations.window_close]
duration = 150
[animations.workspace]
duration = 200
[animations.minimize]
duration = 150
[input.mouse]
sensitivity = 0.0
[input.touchpad]
tap = true
natural_scroll = true
disable_while_typing = true
[workspaces]
count = 10
dynamic = false
wrap = true
[window]
layout = "floating"
work_area_padding = 16
[decorations]
mode = "auto"
titlebar_height = 32
border_width = 4
corner_radius = 12
"#;

pub struct WindowState {
    config: Signal<toml::Value>,
    path: PathBuf,
}
#[derive(Clone)]
struct Writer {
    config: Signal<toml::Value>,
    path: PathBuf,
}

impl WindowState {
    pub fn load() -> Self {
        let path = config_path();
        let config = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_else(|| toml::from_str(DEFAULT).expect("valid built-in Blair TOML"));
        Self {
            config: Signal::new(config),
            path,
        }
    }
    fn writer(&self) -> Writer {
        Writer {
            config: self.config.clone(),
            path: self.path.clone(),
        }
    }
    fn hot_reload(&self) -> bool {
        bool_at(&self.config.get(), &["general", "hot_reload"], true)
    }
    fn apply(&self) -> Option<BoxedWidget> {
        (!self.hot_reload()).then(|| {
            let writer = self.writer();
            Box::new(Button::styled(
                ButtonVariant::Primary,
                ButtonSize::Md,
                "Apply",
                ButtonState::Normal,
                move || writer.save(),
            )) as BoxedWidget
        })
    }
}
impl Writer {
    fn update(&self, change: impl FnOnce(&mut toml::Value)) {
        self.config.update(change);
        if bool_at(&self.config.get(), &["general", "hot_reload"], true) {
            self.save();
        }
    }
    fn save(&self) {
        let result = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("Blair config has no parent"))
            .and_then(std::fs::create_dir_all)
            .and_then(|()| {
                toml::to_string_pretty(&self.config.peek())
                    .map_err(|e| std::io::Error::other(e.to_string()))
            })
            .and_then(|contents| std::fs::write(&self.path, contents));
        if let Err(error) = result {
            eprintln!("settings: failed to save Blair TOML: {error}");
        }
    }
}

pub fn build_general(_: Size, state: &WindowState) -> BoxedWidget {
    let config = state.config.get();
    let hot = bool_at(&config, &["general", "hot_reload"], true);
    let writer = state.writer();
    let hot_control: BoxedWidget = Box::new(Switch::new(hot, move || {
        writer.update(|c| put(c, &["general", "hot_reload"], toml::Value::Boolean(!hot)))
    }));
    let backend = str_at(&config, &["general", "backend"], "auto");
    let selected = ["auto", "drm", "winit"]
        .iter()
        .position(|v| *v == backend)
        .unwrap_or(0);
    let writer = state.writer();
    let backend_control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index| {
            writer.update(|c| {
                put(
                    c,
                    &["general", "backend"],
                    toml::Value::String(["auto", "drm", "winit"][index].into()),
                )
            })
        })
        .option("Auto")
        .option("DRM")
        .option("Nested"),
    );
    finish(
        state,
        "Compositor",
        "Changes go directly to ~/.config/blair/config.toml.",
        vec![group(vec![
            row("Hot reload", hot_control),
            row("Backend", backend_control),
        ])],
    )
}

pub fn build_layout(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let layout = str_at(&c, &["window", "layout"], "floating");
    let writer = state.writer();
    let layout_control: BoxedWidget = Box::new(
        SegmentedControl::new(usize::from(layout == "tiling"), move |i| {
            writer.update(|c| {
                put(
                    c,
                    &["window", "layout"],
                    toml::Value::String(if i == 0 { "floating" } else { "tiling" }.into()),
                )
            })
        })
        .option("Floating")
        .option("Tiling"),
    );
    let mode = str_at(&c, &["decorations", "mode"], "auto");
    let selected = ["auto", "server", "client", "none"]
        .iter()
        .position(|v| *v == mode)
        .unwrap_or(0);
    let writer = state.writer();
    let mode_control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |i| {
            writer.update(|c| {
                put(
                    c,
                    &["decorations", "mode"],
                    toml::Value::String(["auto", "server", "client", "none"][i].into()),
                )
            })
        })
        .option("Auto")
        .option("Server")
        .option("Client")
        .option("None"),
    );
    finish(
        state,
        "Layout",
        "Placement and decorations.",
        vec![group(vec![
            row("Window mode", layout_control),
            row("Decorations", mode_control),
            slider(state, "Border", &["decorations", "border_width"], 16),
            slider(
                state,
                "Corner radius",
                &["decorations", "corner_radius"],
                64,
            ),
            slider(
                state,
                "Titlebar height",
                &["decorations", "titlebar_height"],
                96,
            ),
        ])],
    )
}

pub fn build_working_area(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let dynamic = bool_at(&c, &["workspaces", "dynamic"], false);
    let wrap = bool_at(&c, &["workspaces", "wrap"], true);
    let writer = state.writer();
    let dynamic_control: BoxedWidget = Box::new(Switch::new(dynamic, move || {
        writer.update(|c| {
            put(
                c,
                &["workspaces", "dynamic"],
                toml::Value::Boolean(!dynamic),
            )
        })
    }));
    let writer = state.writer();
    let wrap_control: BoxedWidget = Box::new(Switch::new(wrap, move || {
        writer.update(|c| put(c, &["workspaces", "wrap"], toml::Value::Boolean(!wrap)))
    }));
    finish(
        state,
        "Workspaces",
        "Workspace defaults and tiled-window margins.",
        vec![group(vec![
            slider(state, "Workspace count", &["workspaces", "count"], 100),
            row("Dynamic workspaces", dynamic_control),
            row("Wrap around", wrap_control),
            slider(
                state,
                "Work area padding",
                &["window", "work_area_padding"],
                128,
            ),
        ])],
    )
}

pub fn build_effects(_: Size, state: &WindowState) -> BoxedWidget {
    let enabled = bool_at(&state.config.get(), &["animations", "enabled"], true);
    let writer = state.writer();
    let control: BoxedWidget = Box::new(Switch::new(enabled, move || {
        writer.update(|c| {
            put(
                c,
                &["animations", "enabled"],
                toml::Value::Boolean(!enabled),
            )
        })
    }));
    finish(
        state,
        "Effects",
        "Animations are reloaded by Blair from this file.",
        vec![group(vec![
            row("Enable animations", control),
            slider(
                state,
                "Window open (ms)",
                &["animations", "window_open", "duration"],
                1000,
            ),
            slider(
                state,
                "Window close (ms)",
                &["animations", "window_close", "duration"],
                1000,
            ),
            slider(
                state,
                "Workspace (ms)",
                &["animations", "workspace", "duration"],
                1000,
            ),
            slider(
                state,
                "Minimize (ms)",
                &["animations", "minimize", "duration"],
                1000,
            ),
        ])],
    )
}

pub fn build_input(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let controls = [
        ("Tap to click", "tap"),
        ("Natural scrolling", "natural_scroll"),
        ("Disable while typing", "disable_while_typing"),
    ]
    .into_iter()
    .map(|(label, key)| {
        let value = bool_at(&c, &["input", "touchpad", key], true);
        let writer = state.writer();
        row(
            label,
            Box::new(Switch::new(value, move || {
                writer.update(|c| put(c, &["input", "touchpad", key], toml::Value::Boolean(!value)))
            })) as BoxedWidget,
        )
    })
    .collect();
    finish(
        state,
        "Input",
        "Touchpad and pointer configuration.",
        vec![group(controls)],
    )
}

pub fn build_focus(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let controls = [
        ("Raise on focus", "raise_on_focus", true),
        ("Focus new windows", "focus_new_windows", true),
        ("Focus previous on close", "focus_previous_on_close", true),
        ("Warp cursor", "warp_cursor", false),
    ]
    .into_iter()
    .map(|(label, key, fallback)| {
        let value = bool_at(&c, &["focus", key], fallback);
        let writer = state.writer();
        row(
            label,
            Box::new(Switch::new(value, move || {
                writer.update(|c| put(c, &["focus", key], toml::Value::Boolean(!value)))
            })) as BoxedWidget,
        )
    })
    .collect();
    finish(
        state,
        "Focus",
        "How Blair selects and raises windows.",
        vec![group(controls)],
    )
}

pub fn build_shortcuts(_: Size, state: &WindowState) -> BoxedWidget {
    let bindings = state
        .config
        .get()
        .get("bindings")
        .cloned()
        .unwrap_or(toml::Value::Array(vec![]));
    let mut root = toml::map::Map::new();
    root.insert("bindings".into(), bindings);
    let text = toml::to_string_pretty(&toml::Value::Table(root)).unwrap_or_default();
    let writer = state.writer();
    let editor: BoxedWidget = Box::new(
        TextArea::new(text, move |contents| {
            if let Ok(document) = toml::from_str::<toml::Value>(&contents) {
                if let Some(bindings) = document.get("bindings").cloned() {
                    writer.update(|c| put(c, &["bindings"], bindings));
                }
            }
        })
        .placeholder("[[bindings]]\nkeys = [\"SUPER\", \"RETURN\"]\nexec = \"foot\""),
    );
    finish(
        state,
        "Shortcuts",
        "Edit Blair [[bindings]] TOML. Only valid TOML is committed.",
        vec![editor],
    )
}

fn finish(
    state: &WindowState,
    title: &str,
    subtitle: &str,
    mut body: Vec<BoxedWidget>,
) -> BoxedWidget {
    if let Some(apply) = state.apply() {
        body.push(apply);
    }
    section(title, subtitle, body)
}
fn slider(
    state: &WindowState,
    label: &str,
    path: &'static [&'static str],
    max: i64,
) -> BoxedWidget {
    let value = int_at(&state.config.get(), path, 0).clamp(0, max);
    let writer = state.writer();
    row(
        &format!("{label} ({value})"),
        Box::new(Slider::new(value as f32 / max as f32, move |v| {
            writer.update(|c| {
                put(
                    c,
                    path,
                    toml::Value::Integer((v * max as f32).round() as i64),
                )
            })
        })) as BoxedWidget,
    )
}
fn config_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blair/config.toml")
}
fn at<'a>(v: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
    path.iter().try_fold(v, |current, key| current.get(*key))
}
fn bool_at(v: &toml::Value, path: &[&str], fallback: bool) -> bool {
    at(v, path)
        .and_then(toml::Value::as_bool)
        .unwrap_or(fallback)
}
fn int_at(v: &toml::Value, path: &[&str], fallback: i64) -> i64 {
    at(v, path)
        .and_then(toml::Value::as_integer)
        .unwrap_or(fallback)
}
fn str_at<'a>(v: &'a toml::Value, path: &[&str], fallback: &'a str) -> &'a str {
    at(v, path)
        .and_then(toml::Value::as_str)
        .unwrap_or(fallback)
}
fn put(v: &mut toml::Value, path: &[&str], value: toml::Value) {
    let mut table = v.as_table_mut().expect("Blair config root is a table");
    for key in &path[..path.len() - 1] {
        table = table
            .entry(*key)
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .expect("Blair setting table");
    }
    table.insert(path[path.len() - 1].into(), value);
}
