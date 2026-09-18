//! Blair settings edited through its D-Bus client. Blair validates, applies,
//! and persists its own configuration.
use crate::common::{group, row, section};
use blair_client::BlairClient;
use blair_protocol::{ShortcutArgument, ShortcutBinding, ShortcutCommand};
use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{BoxedWidget, Key, KeyInput, Size, Styled, Widget};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, RawButton, SegmentedControl, Select,
    SelectController, Switch, Text, TextInput, TextSize,
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
follow_system_theme = true
title_centered = false
show_icon = true
drag_margin = 0
[decorations.buttons]
layout = ["minimize", "maximize", "close"]
side = "right"
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
            Err(error) => (
                Vec::new(),
                format!("Window settings are unavailable: {error}"),
            ),
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
            Ok(true) => self.status.set("Shortcuts saved".to_owned()),
            Ok(false) => self
                .status
                .set("These shortcuts could not be saved".to_owned()),
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
        .option("Automatic")
        .option("Direct display")
        .option("Test session"),
    );
    finish(
        state,
        "Advanced",
        "These options control how the desktop starts and applies changes. Most settings take effect immediately.",
        vec![
            setting_group(
                "Applying changes",
                "Turn this off only while making several coordinated changes. Use Apply when you are ready.",
                vec![row("Apply changes automatically", hot_control)],
            ),
            setting_group(
                "Advanced startup",
                "Automatic is recommended. Direct display is for regular desktop use; Test session is for trying changes inside another session and normally requires a restart.",
                vec![row("Startup mode", backend_control)],
            ),
        ],
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
    finish(
        state,
        "Window layout",
        "Choose how new windows are arranged. Tiling settings matter only when Tiling is selected.",
        vec![
            setting_group(
                "New windows",
                "Floating leaves placement to you. Tiling places windows automatically.",
                vec![
                    row("Arrangement", layout_control),
                    integer_field(
                        state,
                        "Default width",
                        &["window", "default_width"],
                        1,
                        None,
                    ),
                    integer_field(
                        state,
                        "Default height",
                        &["window", "default_height"],
                        1,
                        None,
                    ),
                ],
            ),
            setting_group(
                "Tiling",
                "Spacing and master width are exact values, not presets.",
                vec![
                    integer_field(
                        state,
                        "Outer padding",
                        &["window", "work_area_padding"],
                        0,
                        Some(512),
                    ),
                    integer_field(
                        state,
                        "Gap between windows",
                        &["window", "gap"],
                        0,
                        Some(512),
                    ),
                    ratio_field(state, "Master width", &["window", "master_ratio"]),
                ],
            ),
        ],
    )
}

pub fn build_titlebar(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let follow_theme = bool_at(&c, &["decorations", "follow_system_theme"], true);
    let writer = state.writer();
    let follow_theme_control: BoxedWidget = Box::new(Switch::new(follow_theme, move || {
        writer.update(|c| {
            put(
                c,
                &["decorations", "follow_system_theme"],
                toml::Value::Boolean(!follow_theme),
            )
        })
    }));

    let side = str_at(&c, &["decorations", "buttons", "side"], "right");
    let side_selected = usize::from(side == "left");
    let writer = state.writer();
    let side_control: BoxedWidget = Box::new(
        SegmentedControl::new(side_selected, move |i| {
            writer.update(|c| {
                put(
                    c,
                    &["decorations", "buttons", "side"],
                    toml::Value::String(["right", "left"][i].into()),
                )
            })
        })
        .option("Right")
        .option("Left"),
    );

    let centered = bool_at(&c, &["decorations", "title_centered"], false);
    let writer = state.writer();
    let centered_control: BoxedWidget = Box::new(Switch::new(centered, move || {
        writer.update(|c| {
            put(
                c,
                &["decorations", "title_centered"],
                toml::Value::Boolean(!centered),
            )
        })
    }));

    let show_icon = bool_at(&c, &["decorations", "show_icon"], true);
    let writer = state.writer();
    let show_icon_control: BoxedWidget = Box::new(Switch::new(show_icon, move || {
        writer.update(|c| {
            put(
                c,
                &["decorations", "show_icon"],
                toml::Value::Boolean(!show_icon),
            )
        })
    }));

    let mode = str_at(&c, &["decorations", "mode"], "auto");
    let selected = ["auto", "server", "client", "none"]
        .iter()
        .position(|value| *value == mode)
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
        "Titlebar",
        "Choose whether apps or the system draw titlebars. App titlebars can look different from one app to another.",
        vec![
            setting_group("Titlebar source", "Auto chooses the best option for each app.", vec![row("Use titlebar", mode_control)]),
            setting_group("System titlebar", "These options apply when the system draws the titlebar.", vec![
                row("Match system theme colors", follow_theme_control),
                row("Buttons on", side_control),
                row("Center title", centered_control),
                row("Show app icon", show_icon_control),
                integer_field(state, "Border width", &["decorations", "border_width"], 0, Some(16)),
                integer_field(state, "Corner radius", &["decorations", "corner_radius"], 0, Some(64)),
                integer_field(state, "Titlebar height", &["decorations", "titlebar_height"], 0, Some(96)),
                integer_field(state, "Non-draggable edge", &["decorations", "drag_margin"], 0, Some(256)),
            ]),
        ],
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
        "Decide how many workspaces are available and how moving between their ends behaves.",
        vec![setting_group("Workspace navigation", "Dynamic workspaces are created as needed; wrapping goes from the last workspace back to the first.", vec![
            integer_field(state, "Workspace count", &["workspaces", "count"], 1, Some(100)),
            row("Dynamic workspaces", dynamic_control),
            row("Wrap around", wrap_control),
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
        "Animations",
        "Durations are in milliseconds. Set a duration to 0 for an instant transition.",
        vec![setting_group(
            "Motion",
            "These transitions are used when animations are enabled.",
            vec![
                row("Enable animations", control),
                integer_field(
                    state,
                    "Opening",
                    &["animations", "window_open", "duration"],
                    0,
                    Some(10_000),
                ),
                integer_field(
                    state,
                    "Closing",
                    &["animations", "window_close", "duration"],
                    0,
                    Some(10_000),
                ),
                integer_field(
                    state,
                    "Workspace switch",
                    &["animations", "workspace", "duration"],
                    0,
                    Some(10_000),
                ),
                integer_field(
                    state,
                    "Minimize",
                    &["animations", "minimize", "duration"],
                    0,
                    Some(10_000),
                ),
            ],
        )],
    )
}

pub fn build_input(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let touchpad_controls = [
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
    let keyboard_layout = str_at(&c, &["input", "keyboard", "layout"], "us").to_owned();
    let writer = state.writer();
    let layout_control: BoxedWidget = Box::new(
        TextInput::new(keyboard_layout, move |value| {
            if !value.trim().is_empty() {
                writer.update(|c| {
                    put(
                        c,
                        &["input", "keyboard", "layout"],
                        toml::Value::String(value),
                    )
                });
            }
        })
        .placeholder("us"),
    );
    let mouse_sensitivity = at(&c, &["input", "mouse", "sensitivity"])
        .and_then(toml::Value::as_float)
        .unwrap_or(0.0);
    let writer = state.writer();
    let sensitivity_control: BoxedWidget = Box::new(
        TextInput::new(mouse_sensitivity.to_string(), move |value| {
            if let Ok(value) = value.parse::<f64>() {
                if value.is_finite() && (-1.0..=1.0).contains(&value) {
                    writer.update(|c| {
                        put(
                            c,
                            &["input", "mouse", "sensitivity"],
                            toml::Value::Float(value),
                        )
                    });
                }
            }
        })
        .placeholder("0.0"),
    );
    finish(
        state,
        "Input",
        "Set up the keyboard, mouse and touchpad.",
        vec![
            setting_group(
                "Keyboard",
                "Use an XKB layout code, for example us, es or latam.",
                vec![
                    row("Layout", layout_control),
                    integer_field(
                        state,
                        "Repeat delay (ms)",
                        &["input", "keyboard", "repeat_delay"],
                        0,
                        Some(2_000),
                    ),
                    integer_field(
                        state,
                        "Repeat rate",
                        &["input", "keyboard", "repeat_rate"],
                        1,
                        Some(100),
                    ),
                ],
            ),
            setting_group(
                "Mouse",
                "Sensitivity is a precise value from -1.0 (slower) to 1.0 (faster).",
                vec![row("Sensitivity", sensitivity_control)],
            ),
            setting_group(
                "Touchpad",
                "These only affect touchpad devices.",
                touchpad_controls,
            ),
        ],
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
        "Choose how windows receive focus and come to the front.",
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
        "Record shortcuts and choose what they do.",
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
/// A small heading and explanation above each related set of controls.  The
/// compositor has several interacting concepts, so one long unlabelled list
/// makes it too easy to change the wrong thing.
fn setting_group(title: &str, description: &str, rows: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={8.0}>
            {Box::new(Text::new(title.to_owned()).size(TextSize::Sm)) as BoxedWidget}
            {Box::new(Text::secondary(description.to_owned()).size(TextSize::Sm)) as BoxedWidget}
            {group(rows)}
        </Flex>
    })
}

fn integer_field(
    state: &WindowState,
    label: &str,
    path: &'static [&'static str],
    min: i64,
    max: Option<i64>,
) -> BoxedWidget {
    let value = int_at(&state.config.get(), path, min);
    let writer = state.writer();
    row(
        label,
        Box::new(
            TextInput::new(value.to_string(), move |input| {
                let Ok(value) = input.parse::<i64>() else {
                    return;
                };
                if value >= min && max.is_none_or(|max| value <= max) {
                    writer.update(|c| put(c, path, toml::Value::Integer(value)));
                }
            })
            .placeholder(&match max {
                Some(max) => format!("{min}–{max}"),
                None => format!("At least {min}"),
            }),
        ) as BoxedWidget,
    )
}

fn ratio_field(state: &WindowState, label: &str, path: &'static [&'static str]) -> BoxedWidget {
    let value = at(&state.config.get(), path)
        .and_then(toml::Value::as_float)
        .unwrap_or(0.5);
    let writer = state.writer();
    row(
        label,
        Box::new(
            TextInput::new(value.to_string(), move |input| {
                let Ok(value) = input.parse::<f64>() else {
                    return;
                };
                if value.is_finite() && (0.1..=0.9).contains(&value) {
                    writer.update(|c| put(c, path, toml::Value::Float(value)));
                }
            })
            .placeholder("0.1–0.9"),
        ) as BoxedWidget,
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
