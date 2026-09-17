//! Blair settings edited through its D-Bus client. Blair validates, applies,
//! and persists its own configuration.
use crate::common::{group, row, section};
use blair_client::BlairClient;
use blair_protocol::{ShortcutArgument, ShortcutBinding, ShortcutCommand};
use creamui_core::layout::Style;
use creamui_core::{BoxedWidget, Key, KeyInput, Size, Styled, Widget};
use creamui_reactive::Signal;
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, RawButton, SegmentedControl, Select,
    SelectController, Slider, Switch, Text, TextInput, TextSize,
};
use std::rc::Rc;

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
}

/// Editable persistent shortcuts. Blair remains the authority: this state is
/// populated and committed through `blair-client`, never by editing its TOML.
pub struct ShortcutState {
    bindings: Signal<Vec<ShortcutBinding>>,
    capturing: Signal<Option<usize>>,
    status: Signal<String>,
}

impl ShortcutState {
    pub fn load() -> Self {
        let (bindings, status) = match load_shortcuts() {
            Ok(bindings) => (bindings, String::new()),
            Err(error) => (Vec::new(), format!("Blair is unavailable: {error}")),
        };
        Self {
            bindings: Signal::new(bindings),
            capturing: Signal::new(None),
            status: Signal::new(status),
        }
    }

    fn save(&self) {
        let bindings = self.bindings.get();
        match save_shortcuts(&bindings) {
            Ok(true) => self.status.set("Saved to Blair".to_owned()),
            Ok(false) => self.status.set("Blair rejected these shortcuts".to_owned()),
            Err(error) => self.status.set(format!("Could not save: {error}")),
        }
    }
}

fn blair_runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
}

fn load_shortcuts() -> Result<Vec<ShortcutBinding>, String> {
    let runtime = blair_runtime()?;
    runtime.block_on(async {
        BlairClient::connect()
            .await
            .map_err(|error| error.to_string())?
            .configured_shortcuts()
            .await
            .map_err(|error| error.to_string())
    })
}

fn save_shortcuts(bindings: &[ShortcutBinding]) -> Result<bool, String> {
    let runtime = blair_runtime()?;
    runtime.block_on(async {
        BlairClient::connect()
            .await
            .map_err(|error| error.to_string())?
            .set_configured_shortcuts(bindings)
            .await
            .map_err(|error| error.to_string())
    })
}

fn load_configuration() -> Result<String, String> {
    let runtime = blair_runtime()?;
    runtime.block_on(async {
        BlairClient::connect()
            .await
            .map_err(|error| error.to_string())?
            .configuration()
            .await
            .map_err(|error| error.to_string())
    })
}

fn save_configuration(configuration: &str) -> Result<(), String> {
    let runtime = blair_runtime()?;
    let saved = runtime.block_on(async {
        BlairClient::connect()
            .await
            .map_err(|error| error.to_string())?
            .set_configuration(configuration)
            .await
            .map_err(|error| error.to_string())
    })?;
    saved
        .then_some(())
        .ok_or_else(|| "Blair rejected the configuration".to_owned())
}
#[derive(Clone)]
struct Writer {
    config: Signal<toml::Value>,
}

impl WindowState {
    pub fn load() -> Self {
        let config = load_configuration()
            .ok()
            .and_then(|document| toml::from_str(&document).ok())
            .unwrap_or_else(|| toml::from_str(DEFAULT).expect("valid built-in Blair TOML"));
        Self {
            config: Signal::new(config),
        }
    }
    fn writer(&self) -> Writer {
        Writer {
            config: self.config.clone(),
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
        let result = toml::to_string_pretty(&self.config.peek())
            .map_err(|error| error.to_string())
            .and_then(|contents| save_configuration(&contents));
        if let Err(error) = result {
            eprintln!("settings: failed to save Blair configuration: {error}");
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

pub fn build_shortcuts(_: Size, state: &ShortcutState) -> BoxedWidget {
    let bindings = state.bindings.get();
    let mut body = Vec::new();
    let add_state = state.bindings.clone();
    body.push(Box::new(Button::styled(
        ButtonVariant::Primary,
        ButtonSize::Sm,
        "Add shortcut",
        ButtonState::Normal,
        move || {
            add_state.update(|bindings| {
                bindings.push(ShortcutBinding {
                    accelerator: String::new(),
                    command: ShortcutCommand::Close,
                    argument: None,
                });
            });
        },
    )) as BoxedWidget);

    for (index, binding) in bindings.into_iter().enumerate() {
        body.push(shortcut_editor(index, binding, state));
    }

    let save_state = state.clone_handles();
    body.push(Box::new(Button::styled(
        ButtonVariant::Primary,
        ButtonSize::Md,
        "Save shortcuts",
        ButtonState::Normal,
        move || save_state.save(),
    )) as BoxedWidget);
    let status = state.status.get();
    if !status.is_empty() {
        body.push(Box::new(Text::secondary(status).size(TextSize::Sm)) as BoxedWidget);
    }
    section(
        "Shortcuts",
        "Shortcuts are recorded here and saved through Blair's live compositor API.",
        body,
    )
}

impl ShortcutState {
    fn clone_handles(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
            capturing: self.capturing.clone(),
            status: self.status.clone(),
        }
    }
}

fn shortcut_editor(index: usize, binding: ShortcutBinding, state: &ShortcutState) -> BoxedWidget {
    let capture = state.capturing.get() == Some(index);
    let bindings = state.bindings.clone();
    let capturing = state.capturing.clone();
    let start_capture = capturing.clone();
    let recorder = shortcut_recorder(
        if capture {
            "Press shortcut…"
        } else if binding.accelerator.is_empty() {
            "Record shortcut"
        } else {
            &binding.accelerator
        },
        move || start_capture.set(Some(index)),
        move |accelerator| {
            bindings.update(|items| items[index].accelerator = accelerator);
            capturing.set(None);
        },
    );

    let selected = ShortcutCommand::ALL
        .iter()
        .position(|command| *command == binding.command)
        .unwrap_or(0);
    let controller = SelectController::new(selected);
    let command_state = state.bindings.clone();
    let labels: Vec<&str> = ShortcutCommand::ALL
        .iter()
        .map(|command| command.label())
        .collect();
    let command: BoxedWidget = Box::new(Select::controlled(&labels, controller).on_select(
        move |selected| {
            let command = ShortcutCommand::ALL[selected];
            command_state.update(|items| {
                let binding = &mut items[index];
                binding.command = command;
                binding.argument = match command.argument() {
                    ShortcutArgument::None => None,
                    ShortcutArgument::Workspace => Some("1".to_owned()),
                    ShortcutArgument::Command => Some(String::new()),
                };
            });
        },
    ));
    let mut rows = vec![row("Shortcut", recorder), row("Action", command)];
    if binding.command.argument() != ShortcutArgument::None {
        let argument_state = state.bindings.clone();
        let placeholder = match binding.command.argument() {
            ShortcutArgument::Workspace => "Workspace number",
            ShortcutArgument::Command => "Command to run",
            ShortcutArgument::None => unreachable!(),
        };
        let value = binding.argument.unwrap_or_default();
        rows.push(row(
            "Argument",
            Box::new(
                TextInput::new(value, move |value| {
                    argument_state.update(|items| items[index].argument = Some(value));
                })
                .placeholder(placeholder),
            ) as BoxedWidget,
        ));
    }
    let clear_state = state.bindings.clone();
    let remove_state = state.bindings.clone();
    rows.push(row(
        "",
        Box::new(creamui_macros::jsx! {
            <Flex direction={creamui_core::layout::FlexDirection::Row} gap={8.0}>
                {Box::new(Button::styled(ButtonVariant::Secondary, ButtonSize::Sm, "Clear", ButtonState::Normal, move || {
                    clear_state.update(|items| items[index].accelerator.clear());
                })) as BoxedWidget}
                {Box::new(Button::styled(ButtonVariant::Destructive, ButtonSize::Sm, "Remove", ButtonState::Normal, move || {
                    remove_state.update(|items| { if index < items.len() { items.remove(index); } });
                })) as BoxedWidget}
            </Flex>
        }) as BoxedWidget,
    ));
    group(rows)
}

fn shortcut_recorder(
    label: &str,
    on_click: impl Fn() + 'static,
    on_record: impl Fn(String) + 'static,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let style = Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Length(200.0),
            height: creamui_core::layout::Dimension::Length(34.0),
        },
        ..Default::default()
    };
    let button = RawButton::new(style, on_click)
        .background(theme.surface_elevated)
        .border(theme.border_strong, 1.0)
        .corner_radius(theme.button_radius)
        .child(Box::new(Text::new(label.to_owned()).size(TextSize::Sm)) as BoxedWidget);
    Box::new(ShortcutRecorder {
        inner: button,
        on_record: Rc::new(on_record),
    })
}

struct ShortcutRecorder {
    inner: RawButton,
    on_record: Rc<dyn Fn(String)>,
}

impl Widget for ShortcutRecorder {
    fn style(&self) -> creamui_core::Style {
        self.inner.style()
    }
    fn style_state(&self) -> creamui_core::StyleState {
        self.inner.style_state()
    }
    fn paint(&self, painter: &mut dyn creamui_core::Painter, rect: creamui_core::Rect) {
        self.inner.paint(painter, rect)
    }
    fn children(&mut self) -> Vec<BoxedWidget> {
        self.inner.children()
    }
    fn focusable(&self) -> bool {
        true
    }
    fn on_click(&self) -> Option<Rc<dyn Fn()>> {
        self.inner.on_click()
    }
    fn cursor_icon(&self) -> Option<creamui_core::CursorIcon> {
        self.inner.cursor_icon()
    }
    fn on_key(&self) -> Option<Rc<dyn Fn(KeyInput)>> {
        let record = self.on_record.clone();
        Some(Rc::new(move |input| {
            let key = match input.key {
                Key::Char(key) => key.to_ascii_uppercase().to_string(),
                Key::Enter => "Return".to_owned(),
                Key::Escape => "Escape".to_owned(),
                _ => return,
            };
            let mut parts = Vec::new();
            if input.modifiers.ctrl {
                parts.push("Ctrl");
            }
            if input.modifiers.alt {
                parts.push("Alt");
            }
            if input.modifiers.shift {
                parts.push("Shift");
            }
            if input.modifiers.logo {
                parts.push("Super");
            }
            if parts.is_empty() {
                return;
            }
            parts.push(&key);
            record(parts.join("+"));
        }))
    }
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
