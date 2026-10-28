use crate::components::{group, row, section, section_label};
use crate::routes::destination::Section;
use crate::{components::system_settings::*, routes::detail::Page, services::settings::State};
use blair_client::BlairClient;
use blair_protocol::InputCapabilities;
use coconut_core::{CustomShortcut, ShellShortcut, ShortcutAction, ShortcutConfig};
use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{BoxedWidget, Key, KeyInput, Size, Styled, Widget};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_theme::Color;
use creamui_widgets::{
    Button, ButtonSize, ButtonState, ButtonVariant, ColorPicker, ColorPickerController, Icon,
    IconSource, RawButton, SegmentedControl, Select, SelectController, Switch, Text, TextArea,
    TextController, TextInput, TextSize,
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

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
natural_scroll = false
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
titlebar_color = "blend"
border = "theme"
border_size = "normal"
title_centered = false
show_icon = true
drag_margin = 0
[decorations.buttons]
layout = ["minimize", "maximize", "close"]
side = "right"
style = "normal"
"#;

pub struct WindowState {
    config: Signal<toml::Value>,
    confirmed: Signal<toml::Value>,
    available: bool,
    status: Signal<String>,
    rules_editor: TextController,
    app: Rc<RefCell<Option<creamui_render::AppHandle>>>,
    busy: Signal<bool>,
    titlebar_picker: ColorPickerController,
    active_border_picker: ColorPickerController,
    inactive_border_picker: ColorPickerController,
    numeric_inputs: RefCell<HashMap<String, TextController>>,
    layout_select: SelectController,
    input_capabilities: Signal<InputCapabilityState>,
}

#[derive(Clone, Default)]
struct InputCapabilityState {
    capabilities: Option<InputCapabilities>,
    error: Option<String>,
}

pub struct ShortcutState {
    bindings: Signal<Vec<ShellShortcut>>,
    custom_bindings: Signal<Vec<CustomShortcut>>,
    capturing: Signal<Option<ShortcutCapture>>,
    status: Signal<String>,
    saved: Signal<ShortcutConfig>,
    app: Rc<RefCell<Option<creamui_render::AppHandle>>>,
    busy: Signal<bool>,
    window: Rc<RefCell<Option<creamui_render::WindowHandle>>>,
    recorded_modifiers: Rc<RefCell<creamui_render::platform::Modifiers>>,
    compositor_bindings: Signal<Option<Vec<blair_protocol::ShortcutBinding>>>,
    compositor_saved: Signal<Vec<blair_protocol::ShortcutBinding>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ShortcutCapture {
    BuiltIn(usize),
    Custom(usize),
    Compositor(usize),
}

impl ShortcutState {
    pub fn load() -> Self {
        let config = ShortcutConfig::load();
        Self {
            saved: Signal::new(config.clone()),
            bindings: Signal::new(config.shortcuts),
            custom_bindings: Signal::new(config.custom_shortcuts),
            capturing: Signal::new(None),
            status: Signal::new(String::new()),
            app: Rc::new(RefCell::new(None)),
            busy: Signal::new(false),
            window: Rc::new(RefCell::new(None)),
            recorded_modifiers: Rc::new(RefCell::new(Default::default())),
            compositor_bindings: Signal::new(None),
            compositor_saved: Signal::new(Vec::new()),
        }
    }

    pub fn start(&self, app: creamui_render::AppHandle) {
        *self.app.borrow_mut() = Some(app.clone());
        let status = self.status.clone();
        app.spawn_background(shortcut_service_status, move |result| match result {
            Ok(Some(message)) => status.set(message),
            Ok(None) => {}
            Err(error) => status.set(format!(
                "Could not read shortcut registration status: {error}"
            )),
        });
        let state = self.clone_handles();
        app.spawn_background(
            || {
                blair_runtime()?.block_on(async {
                    BlairClient::connect()
                        .await
                        .map_err(|error| error.to_string())?
                        .configured_shortcuts()
                        .await
                        .map_err(|error| error.to_string())
                })
            },
            move |result: Result<Vec<blair_protocol::ShortcutBinding>, String>| {
                if let Ok(bindings) = result {
                    state.compositor_saved.set(bindings.clone());
                    state.compositor_bindings.set(Some(bindings));
                }
            },
        );
    }

    pub fn attach_window(&self, window: creamui_render::WindowHandle) {
        *self.window.borrow_mut() = Some(window.clone());
        let state = self.clone_handles();
        window.on_keyboard_input(move |event, modifiers| state.record_native(event, modifiers));
        let state = self.clone_handles();
        window.on_focus_lost(move || state.cancel_capture());
    }

    fn begin_capture(&self, target: ShortcutCapture) {
        self.cancel_capture();
        if let Some(window) = self.window.borrow().as_ref() {
            if let Err(error) = window.set_shortcuts_inhibited(true) {
                self.status
                    .set(format!("Cannot safely record global shortcuts: {error}"));
                return;
            }
        }
        self.status.set("Press a shortcut; Escape cancels".into());
        self.capturing.set(Some(target));
    }

    pub fn cancel_capture(&self) {
        if self.capturing.peek().is_some() {
            if let Some(window) = self.window.borrow().as_ref() {
                if let Err(error) = window.set_shortcuts_inhibited(false) {
                    eprintln!("settings: could not release shortcut capture: {error}");
                }
            }
        }
        self.capturing.set(None);
        *self.recorded_modifiers.borrow_mut() = Default::default();
    }

    fn finish_capture(&self, accelerator: String) {
        if let Err(error) = blair_protocol::ShortcutAccelerator::parse(&accelerator) {
            self.status.set(error);
            return;
        }
        match self.capturing.peek() {
            Some(ShortcutCapture::BuiltIn(index)) => self.bindings.update(|items| {
                if let Some(item) = items.get_mut(index) {
                    item.accelerator = accelerator;
                }
            }),
            Some(ShortcutCapture::Custom(index)) => self.custom_bindings.update(|items| {
                if let Some(item) = items.get_mut(index) {
                    item.accelerator = accelerator;
                }
            }),
            Some(ShortcutCapture::Compositor(index)) => self.compositor_bindings.update(|items| {
                if let Some(item) = items.as_mut().and_then(|items| items.get_mut(index)) {
                    item.accelerator = accelerator;
                }
            }),
            None => return,
        }
        self.cancel_capture();
        self.status.set(String::new());
    }

    fn record_native(
        &self,
        event: creamui_render::platform::KeyEvent,
        modifiers: creamui_render::platform::Modifiers,
    ) -> bool {
        use creamui_render::platform::Key;
        if self.capturing.peek().is_none() || event.synthetic {
            return false;
        }
        let mut held = *self.recorded_modifiers.borrow();
        held.ctrl |= modifiers.ctrl;
        held.alt |= modifiers.alt;
        held.shift |= modifiers.shift;
        held.logo |= modifiers.logo;
        let modifier = match event.key {
            Key::Control => Some(&mut held.ctrl),
            Key::Alt => Some(&mut held.alt),
            Key::Shift => Some(&mut held.shift),
            Key::Super => Some(&mut held.logo),
            _ => None,
        };
        if let Some(modifier) = modifier {
            *modifier = true;
            *self.recorded_modifiers.borrow_mut() = held;
            if !event.pressed {
                self.finish_capture(native_accelerator(None, held));
            }
            return true;
        }
        if !event.pressed {
            return true;
        }
        if event.key == Key::Escape
            && !modifiers.ctrl
            && !modifiers.alt
            && !modifiers.shift
            && !modifiers.logo
        {
            self.cancel_capture();
            self.status.set(String::new());
            return true;
        }
        let key = match event.key {
            Key::Character(value) => match value.as_str() {
                " " => "Space".into(),
                "," => "Comma".into(),
                "+" => "Plus".into(),
                _ => value.to_ascii_uppercase(),
            },
            Key::Media(name) => name,
            Key::Space => "Space".into(),
            Key::Enter => "Return".into(),
            Key::Back => {
                self.status.set("Unsupported key".into());
                return true;
            }
            Key::Other => {
                self.status.set("Unsupported key".into());
                return true;
            }
            key => format!("{key:?}"),
        };
        self.finish_capture(native_accelerator(Some(&key), modifiers));
        true
    }

    fn save(&self) {
        if self.busy.get() {
            return;
        }
        let config = ShortcutConfig {
            shortcuts: self.bindings.get(),
            custom_shortcuts: self.custom_bindings.get(),
        };
        if let Err(error) = config.validate() {
            self.status.set(error);
            return;
        }
        self.cancel_capture();
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        let state = self.clone_handles();
        let saved = config.clone();
        self.busy.set(true);
        app.spawn_background(
            move || apply_shell_shortcuts(&config),
            move |result| {
                state.busy.set(false);
                match result {
                    Ok(message) => {
                        state.saved.set(saved);
                        state.status.set(message);
                    }
                    Err(error) => state.status.set(error),
                }
            },
        );
    }
}

fn native_accelerator(key: Option<&str>, modifiers: creamui_render::platform::Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    if modifiers.logo {
        parts.push("Super");
    }
    if let Some(key) = key {
        parts.push(key);
    }
    parts.join("+")
}

fn shortcut_service_status() -> Result<Option<String>, String> {
    let bus = dbus::blocking::Connection::new_session().map_err(|error| error.to_string())?;
    let proxy = bus.with_proxy(
        "org.coconut.Shortcuts",
        "/org/coconut/Shortcuts",
        std::time::Duration::from_secs(5),
    );
    let result: Result<(String,), dbus::Error> =
        proxy.method_call("org.coconut.Shortcuts1", "Status", ());
    match result {
        Ok((status,)) => Ok(Some(status)),
        Err(error) if missing_shortcut_service(&error) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn missing_shortcut_service(error: &dbus::Error) -> bool {
    matches!(
        error.name(),
        Some(
            "org.freedesktop.DBus.Error.ServiceUnknown"
                | "org.freedesktop.DBus.Error.NameHasNoOwner"
        )
    )
}

fn apply_shell_shortcuts(config: &ShortcutConfig) -> Result<String, String> {
    config.validate()?;
    let bus = dbus::blocking::Connection::new_session().map_err(|error| error.to_string())?;
    let proxy = bus.with_proxy(
        "org.coconut.Shortcuts",
        "/org/coconut/Shortcuts",
        std::time::Duration::from_secs(30),
    );
    let document = toml::to_string(config).map_err(|error| error.to_string())?;
    let result: Result<(), dbus::Error> =
        proxy.method_call("org.coconut.Shortcuts1", "Apply", (document,));
    match result {
        Ok(()) => Ok("Shortcuts saved and applied".into()),
        Err(error) if missing_shortcut_service(&error) => {
            config
                .save()
                .map_err(|error| format!("Could not save: {error}"))?;
            Ok("Shortcuts saved. They will apply when Coconut starts in Blair.".into())
        }
        Err(error) => Err(format!("Shortcuts were not applied: {error}")),
    }
}

fn blair_runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())
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

fn load_input_capabilities() -> Result<InputCapabilities, String> {
    let runtime = blair_runtime()?;
    runtime.block_on(async {
        BlairClient::connect()
            .await
            .map_err(|error| error.to_string())?
            .input_capabilities()
            .await
            .map_err(|error| error.to_string())
    })
}

fn set_input_capabilities(
    target: &Signal<InputCapabilityState>,
    result: Result<InputCapabilities, String>,
) {
    target.set(match result {
        Ok(capabilities) => InputCapabilityState {
            capabilities: Some(capabilities),
            error: None,
        },
        Err(error) => InputCapabilityState {
            capabilities: None,
            error: Some(error),
        },
    });
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
    confirmed: Signal<toml::Value>,
    available: bool,
    status: Signal<String>,
    app: Rc<RefCell<Option<creamui_render::AppHandle>>>,
    busy: Signal<bool>,
    input_capabilities: Signal<InputCapabilityState>,
}

impl WindowState {
    pub fn load() -> Self {
        Self::from_configuration(load_configuration())
    }

    #[cfg(test)]
    pub fn preview() -> Self {
        Self::from_configuration(Ok(DEFAULT.to_owned()))
    }

    fn from_configuration(document: Result<String, String>) -> Self {
        let loaded = document.and_then(|document| {
            toml::from_str::<toml::Value>(&document).map_err(|e| e.to_string())
        });
        let available = loaded.is_ok();
        let status = loaded
            .as_ref()
            .err()
            .map(|error| format!("Blair settings are unavailable: {error}"))
            .unwrap_or_default();
        let mut config =
            loaded.unwrap_or_else(|_| toml::from_str(DEFAULT).expect("valid built-in Blair TOML"));
        remove_obsolete_window_size_defaults(&mut config);
        let mut rules = toml::map::Map::new();
        if let Some(value) = config.get("rules") {
            rules.insert("rules".into(), value.clone());
        }
        let rules = toml::to_string_pretty(&toml::Value::Table(rules)).unwrap_or_default();
        Self {
            layout_select: SelectController::new(usize::from(
                str_at(&config, &["window", "layout"], "floating") == "tiling",
            )),
            input_capabilities: Signal::new(InputCapabilityState::default()),
            confirmed: Signal::new(config.clone()),
            config: Signal::new(config),
            available,
            status: Signal::new(status),
            rules_editor: TextController::new(rules),
            app: Rc::new(RefCell::new(None)),
            busy: Signal::new(false),
            titlebar_picker: ColorPickerController::new(),
            active_border_picker: ColorPickerController::new(),
            inactive_border_picker: ColorPickerController::new(),
            numeric_inputs: RefCell::new(HashMap::new()),
        }
    }
    fn numeric_input(&self, path: &[&str], initial: String) -> TextController {
        self.numeric_inputs
            .borrow_mut()
            .entry(path.join("."))
            .or_insert_with(|| TextController::new(initial))
            .clone()
    }
    fn writer(&self) -> Writer {
        Writer {
            config: self.config.clone(),
            confirmed: self.confirmed.clone(),
            available: self.available,
            status: self.status.clone(),
            app: self.app.clone(),
            busy: self.busy.clone(),
            input_capabilities: self.input_capabilities.clone(),
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

    pub fn availability(&self) -> BoxedWidget {
        Box::new(Text::secondary(self.status.get()).size(TextSize::Sm))
    }
    pub fn start(&self, app: creamui_render::AppHandle) {
        *self.app.borrow_mut() = Some(app.clone());
        let input_capabilities = self.input_capabilities.clone();
        app.spawn_background(load_input_capabilities, move |result| {
            set_input_capabilities(&input_capabilities, result);
        });
    }
    pub fn layout_control(&self) -> BoxedWidget {
        if !self.available {
            return Box::new(Text::secondary("Unavailable").size(TextSize::Sm));
        }
        let config = self.config.get();
        let current = str_at(&config, &["window", "layout"], "floating");
        let selected = usize::from(current == "tiling");
        if self.layout_select.peek_selected() != selected {
            self.layout_select.select(selected);
        }
        let writer = self.writer();
        Box::new(
            Select::controlled(&["Floating", "Tiling"], self.layout_select.clone())
                .width(136.0)
                .height(34.0)
                .on_select(move |i| {
                    writer.update(|c| {
                        put(
                            c,
                            &["window", "layout"],
                            toml::Value::String(if i == 0 { "floating" } else { "tiling" }.into()),
                        )
                    })
                }),
        )
    }
    pub fn gaps_control(&self) -> BoxedWidget {
        if !self.available {
            return Box::new(Text::secondary("Unavailable").size(TextSize::Sm));
        }
        let gaps = at(&self.config.get(), &["window", "gap"])
            .and_then(toml::Value::as_integer)
            .unwrap_or(0);
        let writer = self.writer();
        Box::new(creamui_widgets::Slider::new(gaps as f32 / 64.0, move |v| {
            writer.update(|c| {
                put(
                    c,
                    &["window", "gap"],
                    toml::Value::Integer((v * 64.0).round() as i64),
                )
            })
        }))
    }
    fn focus_policy_control(&self) -> BoxedWidget {
        if !self.available {
            return value("Unavailable");
        }
        let follows = str_at(&self.config.get(), &["focus", "policy"], "click") == "sloppy";
        let writer = self.writer();
        Box::new(Switch::new(follows, move || {
            writer.update(|config| {
                put(
                    config,
                    &["focus", "policy"],
                    toml::Value::String(if follows { "click" } else { "sloppy" }.into()),
                )
            });
        }))
    }
    fn centered_control(&self) -> BoxedWidget {
        if !self.available {
            return value("Unavailable");
        }
        let centered = bool_at(&self.config.get(), &["window", "center_new_windows"], true);
        let writer = self.writer();
        Box::new(Switch::new(centered, move || {
            writer.update(|config| {
                put(
                    config,
                    &["window", "center_new_windows"],
                    toml::Value::Boolean(!centered),
                )
            });
        }))
    }
    pub fn effects_control(&self) -> BoxedWidget {
        if !self.available {
            return Box::new(Text::secondary("Unavailable").size(TextSize::Sm));
        }
        let enabled = bool_at(&self.config.get(), &["animations", "enabled"], true);
        let writer = self.writer();
        Box::new(Switch::new(enabled, move || {
            writer.update(|c| {
                put(
                    c,
                    &["animations", "enabled"],
                    toml::Value::Boolean(!enabled),
                )
            })
        }))
    }
    pub fn reduce_motion(&self, fallback: BoxedWidget) -> BoxedWidget {
        if !self.available {
            return fallback;
        }
        let enabled = bool_at(&self.config.get(), &["animations", "enabled"], true);
        let writer = self.writer();
        row(
            "Reduce motion",
            Box::new(Switch::new(!enabled, move || {
                writer.update(|c| {
                    put(
                        c,
                        &["animations", "enabled"],
                        toml::Value::Boolean(!enabled),
                    )
                })
            })),
        )
    }
    pub fn rules(&self, _: &TextController) -> BoxedWidget {
        if !self.available {
            return self.availability();
        }
        let editor = self.rules_editor.clone();
        let writer = self.writer();
        section("", "", vec![Box::new(Text::secondary("Edit [[rules]] entries. Blair validates, applies and persists the document; rejected changes are rolled back.").size(TextSize::Sm)),
            Box::new(
                TextArea::controlled(&self.rules_editor)
                    .layout(Style {
                        size: creamui_core::layout::Size {
                            width: creamui_core::layout::Dimension::Percent(1.0),
                            height: creamui_core::layout::Dimension::Length(280.0),
                        },
                        padding: creamui_core::layout::Rect {
                            left: creamui_core::layout::LengthPercentage::Length(12.0),
                            right: creamui_core::layout::LengthPercentage::Length(12.0),
                            top: creamui_core::layout::LengthPercentage::Length(12.0),
                            bottom: creamui_core::layout::LengthPercentage::Length(12.0),
                        },
                        ..Default::default()
                    })
                    .placeholder("[[rules]]\napp_id = \"org.example.App\"\nfloating = true"),
            ),
            Box::new(Button::new("Apply rules", move || {
                let parsed = toml::from_str::<toml::Value>(&editor.value());
                match parsed {
                    Ok(toml::Value::Table(table)) if table.keys().all(|key| key == "rules") && table.get("rules").is_none_or(|v| v.is_array()) => {
                        writer.config.update(|c| {
                            if let Some(table_config) = c.as_table_mut() {
                                if let Some(rules) = table.get("rules") { table_config.insert("rules".into(), rules.clone()); } else { table_config.remove("rules"); }
                            }
                        }); writer.save();
                    }
                    Ok(_) => writer.status.set("Only [[rules]] entries are accepted here".into()),
                    Err(error) => writer.status.set(format!("Invalid rules: {error}")),
                }
            })), self.availability()])
    }
}
impl Writer {
    fn update(&self, change: impl FnOnce(&mut toml::Value)) {
        if !self.available {
            return;
        }
        self.config.update(change);
        if bool_at(&self.config.get(), &["general", "hot_reload"], true) {
            self.save();
        }
    }
    fn save(&self) {
        if !self.available {
            return;
        }
        if self.busy.peek() {
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            self.status.set("The application is not running".into());
            return;
        };
        let candidate = self.config.peek();
        let baseline = self.confirmed.peek();
        self.busy.set(true);
        self.status.set("Applying with Blair…".into());
        let writer = self.clone();
        let refresh_app = app.clone();
        let attempted = candidate.clone();
        app.spawn_background(
            move || {
                // Preserve unrelated changes made by Startup Applications or
                // another client since this editor loaded its configuration.
                let mut latest: toml::Value =
                    toml::from_str(&load_configuration()?).map_err(|e| e.to_string())?;
                merge_changes(&baseline, &attempted, &mut latest);
                remove_obsolete_window_size_defaults(&mut latest);
                save_configuration(&toml::to_string_pretty(&latest).map_err(|e| e.to_string())?)?;
                Ok::<_, String>(latest)
            },
            move |result| {
                writer.busy.set(false);
                match result {
                    Err(error) => {
                        writer.config.set(writer.confirmed.peek());
                        writer.status.set(format!("Could not apply: {error}"));
                    }
                    Ok(applied) => {
                        let pending = writer.config.peek();
                        let mut next = applied.clone();
                        merge_changes(&candidate, &pending, &mut next);
                        writer.confirmed.set(applied.clone());
                        writer.config.set(next.clone());
                        writer.status.set("Changes applied by Blair".into());
                        let input_capabilities = writer.input_capabilities.clone();
                        refresh_app.spawn_background(load_input_capabilities, move |result| {
                            set_input_capabilities(&input_capabilities, result);
                        });
                        if next != applied {
                            writer.save();
                        }
                    }
                }
            },
        );
    }
}

fn merge_changes(before: &toml::Value, edited: &toml::Value, latest: &mut toml::Value) {
    if before == edited {
        return;
    }
    if let (Some(before), Some(edited), Some(latest)) =
        (before.as_table(), edited.as_table(), latest.as_table_mut())
    {
        for key in before
            .keys()
            .chain(edited.keys())
            .collect::<std::collections::BTreeSet<_>>()
        {
            match (before.get(key), edited.get(key)) {
                (Some(old), Some(new)) if old != new => {
                    if let Some(current) = latest.get_mut(key) {
                        merge_changes(old, new, current);
                    } else {
                        latest.insert(key.clone(), new.clone());
                    }
                }
                (None, Some(new)) => {
                    latest.insert(key.clone(), new.clone());
                }
                (Some(_), None) => {
                    latest.remove(key);
                }
                _ => {}
            }
        }
    } else {
        *latest = edited.clone();
    }
}

fn remove_obsolete_window_size_defaults(config: &mut toml::Value) {
    let Some(window) = config.get_mut("window").and_then(toml::Value::as_table_mut) else {
        return;
    };
    window.remove("default_width");
    window.remove("default_height");
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
                vec![row("Arrangement", layout_control)],
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

    let button_style = choice_at(
        &c,
        &["decorations", "buttons", "style"],
        BUTTON_STYLES,
        "normal",
    );
    let button_style_control = choice_control(
        state,
        &["decorations", "buttons", "style"],
        BUTTON_STYLES,
        button_style,
    );

    let mode = str_at(&c, &["decorations", "mode"], "auto");
    let selected = ["auto", "server", "hybrid", "client", "none"]
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
                    toml::Value::String(["auto", "server", "hybrid", "client", "none"][i].into()),
                )
            })
        })
        .option("Auto")
        .option("Server")
        .option("Hybrid")
        .option("Client")
        .option("None"),
    );

    let titlebar_color = choice_at(
        &c,
        &["decorations", "titlebar_color"],
        TITLEBAR_COLORS,
        "blend",
    );
    let mut appearance_rows = vec![row(
        "Titlebar color",
        choice_control(
            state,
            &["decorations", "titlebar_color"],
            TITLEBAR_COLORS,
            titlebar_color,
        ),
    )];
    if TITLEBAR_COLORS[titlebar_color].0 == "color" {
        appearance_rows.push(row(
            "Custom color",
            color_control(
                state,
                &["decorations", "custom_titlebar_color"],
                "#1e1e2e",
                &state.titlebar_picker,
            ),
        ));
    }

    let border = choice_at(&c, &["decorations", "border"], BORDER_COLORS, "theme");
    appearance_rows.push(row(
        "Border",
        choice_control(state, &["decorations", "border"], BORDER_COLORS, border),
    ));
    match BORDER_COLORS[border].0 {
        "none" => {}
        border => {
            if border == "custom" {
                appearance_rows.push(row(
                    "Focused window",
                    color_control(
                        state,
                        &["decorations", "active_border"],
                        "#89b4fa",
                        &state.active_border_picker,
                    ),
                ));
                appearance_rows.push(row(
                    "Other windows",
                    color_control(
                        state,
                        &["decorations", "inactive_border"],
                        "#313244",
                        &state.inactive_border_picker,
                    ),
                ));
            }
            let size = choice_at(&c, &["decorations", "border_size"], BORDER_SIZES, "normal");
            appearance_rows.push(row(
                "Border size",
                choice_control(state, &["decorations", "border_size"], BORDER_SIZES, size),
            ));
        }
    }

    finish(
        state,
        "Titlebar",
        "Choose whether apps or the system draw titlebars. App titlebars can look different from one app to another.",
        vec![
            setting_group("Titlebar source", "Auto chooses the best option for each app. Hybrid draws server controls over the app's top-right corner.", vec![row("Use titlebar", mode_control)]),
            setting_group(
                "Look",
                "Auto uses the most repeated color on the app edge. Gradient blends six averaged edge zones across the bar; Blend copies the complete edge. Theme uses a blurred surface. Title text and buttons adjust automatically for contrast.",
                appearance_rows,
            ),
            setting_group("System titlebar", "These options apply when the system draws the titlebar.", vec![
                row("Buttons on", side_control),
                row("Control style", button_style_control),
                row("Center title", centered_control),
                row("Show app icon", show_icon_control),
                integer_field(state, "Titlebar height", &["decorations", "titlebar_height"], 0, Some(96)),
                integer_field(state, "Non-draggable edge", &["decorations", "drag_margin"], 0, Some(256)),
            ]),
        ],
    )
}

/// `(config value, label)` pairs for single-choice settings stored as
/// strings. Blair reads these case-insensitively.
const TITLEBAR_COLORS: &[(&str, &str)] = &[
    ("auto", "Auto"),
    ("gradient", "Gradient"),
    ("blend", "Blend"),
    ("theme", "Theme"),
    ("color", "Color"),
];
const BORDER_COLORS: &[(&str, &str)] =
    &[("none", "None"), ("theme", "Theme"), ("custom", "Custom")];
const BORDER_SIZES: &[(&str, &str)] = &[("thin", "Thin"), ("normal", "Normal"), ("bold", "Bold")];
const BUTTON_STYLES: &[(&str, &str)] =
    &[("normal", "Normal"), ("traffic_lights", "Traffic lights")];

/// Index of the stored choice, or of Blair's `default` when unset.
fn choice_at(
    config: &toml::Value,
    path: &[&str],
    choices: &[(&str, &str)],
    default: &str,
) -> usize {
    let default = choices
        .iter()
        .position(|(id, _)| *id == default)
        .unwrap_or(0);
    at(config, path)
        .and_then(toml::Value::as_str)
        .and_then(|value| {
            choices
                .iter()
                .position(|(id, _)| id.eq_ignore_ascii_case(value.trim()))
        })
        .unwrap_or(default)
}

fn choice_control(
    state: &WindowState,
    path: &'static [&'static str],
    choices: &'static [(&'static str, &'static str)],
    selected: usize,
) -> BoxedWidget {
    let writer = state.writer();
    let control = SegmentedControl::new(selected, move |index| {
        writer.update(|c| put(c, path, toml::Value::String(choices[index].0.into())))
    });
    Box::new(
        choices
            .iter()
            .fold(control, |control, (_, label)| control.option(*label)),
    )
}

fn color_control(
    state: &WindowState,
    path: &'static [&'static str],
    fallback: &str,
    picker: &ColorPickerController,
) -> BoxedWidget {
    let current = str_at(&state.config.get(), path, fallback)
        .parse::<Color>()
        .or_else(|_| fallback.parse())
        .unwrap_or(Color::rgb(0, 0, 0));
    let writer = state.writer();
    Box::new(ColorPicker::controlled(current, picker, move |color| {
        let hex = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
        writer.update(|c| put(c, path, toml::Value::String(hex)))
    }))
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

pub fn build_keyboard(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
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
    finish(
        state,
        "Keyboard",
        "Use an XKB layout code, for example us, es or latam.",
        vec![group(vec![
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
        ])],
    )
}

pub fn build_mouse(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let capabilities = state.input_capabilities.get();
    let supported = capabilities
        .capabilities
        .as_ref()
        .is_some_and(|info| info.devices.iter().any(|device| device.sensitivity));
    let mouse_sensitivity = at(&c, &["input", "mouse", "sensitivity"])
        .and_then(toml::Value::as_float)
        .unwrap_or(0.0);
    let sensitivity_control: BoxedWidget = if supported {
        let writer = state.writer();
        Box::new(
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
        )
    } else {
        unavailable_input_control(&capabilities, "pointer sensitivity")
    };
    finish(
        state,
        "Mouse",
        &input_capability_description(
            &capabilities,
            "Sensitivity is a precise value from -1.0 (slower) to 1.0 (faster).",
        ),
        vec![group(vec![row("Sensitivity", sensitivity_control)])],
    )
}

pub fn build_touchpad(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let capabilities = state.input_capabilities.get();
    let touchpad_controls = [
        ("Tap to click", "tap"),
        ("Natural scrolling", "natural_scroll"),
        ("Disable while typing", "disable_while_typing"),
    ]
    .into_iter()
    .map(|(label, key)| {
        let value = bool_at(&c, &["input", "touchpad", key], key != "natural_scroll");
        let supported = capabilities.capabilities.as_ref().is_some_and(|info| {
            info.devices.iter().any(|device| match key {
                "tap" => device.tap,
                "natural_scroll" => device.natural_scroll,
                "disable_while_typing" => device.disable_while_typing,
                _ => false,
            })
        });
        let control: BoxedWidget = if supported {
            let writer = state.writer();
            Box::new(Switch::new(value, move || {
                writer.update(|c| put(c, &["input", "touchpad", key], toml::Value::Boolean(!value)))
            }))
        } else {
            unavailable_input_control(&capabilities, label)
        };
        row(label, control)
    })
    .collect();
    finish(
        state,
        "Touchpad",
        &input_capability_description(&capabilities, "These only affect touchpad devices."),
        vec![group(touchpad_controls)],
    )
}

fn unavailable_input_control(state: &InputCapabilityState, setting: &str) -> BoxedWidget {
    let message = match &state.error {
        Some(error) => format!("Unavailable: {error}"),
        None => match &state.capabilities {
            Some(info) if info.backend == "winit" => "Controlled by the host compositor".into(),
            Some(_) => format!("No connected device supports {setting}"),
            None => "Checking connected input devices…".into(),
        },
    };
    Box::new(Text::secondary(message).size(TextSize::Sm))
}

fn input_capability_description(state: &InputCapabilityState, fallback: &str) -> String {
    match &state.error {
        Some(error) => format!("{fallback} Could not inspect Blair input capabilities: {error}"),
        None => match &state.capabilities {
            None => format!("{fallback} Checking connected input devices…"),
            Some(info) => {
                let devices = info.devices.len();
                let errors = if info.errors.is_empty() {
                    String::new()
                } else {
                    format!(" Last input error: {}", info.errors.join("; "))
                };
                if info.backend == "winit" {
                    format!("{fallback} This nested session receives pointer settings from its host compositor.{errors}")
                } else {
                    format!(
                        "{fallback} Blair detected {devices} input device{}.{errors}",
                        if devices == 1 { "" } else { "s" }
                    )
                }
            }
        },
    }
}

pub fn build_focus(_: Size, state: &WindowState) -> BoxedWidget {
    let c = state.config.get();
    let mut controls: Vec<BoxedWidget> = [
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
    controls.insert(0, row("Focus follows mouse", state.focus_policy_control()));
    finish(
        state,
        "Focus",
        "Choose how windows receive focus and come to the front.",
        vec![group(controls)],
    )
}

pub fn build_hot_corners(_: Size, state: &WindowState) -> BoxedWidget {
    const ACTIONS: &[(&str, &str)] = &[
        ("none", "Off"),
        ("next_workspace", "Next workspace"),
        ("previous_workspace", "Previous workspace"),
        ("show_desktop", "Show desktop"),
        ("exec", "Run command"),
    ];
    const CORNERS: &[(&str, &[&str], &[&str])] = &[
        (
            "Top left",
            &["hot_corners", "top_left", "action"],
            &["hot_corners", "top_left", "command"],
        ),
        (
            "Top right",
            &["hot_corners", "top_right", "action"],
            &["hot_corners", "top_right", "command"],
        ),
        (
            "Bottom left",
            &["hot_corners", "bottom_left", "action"],
            &["hot_corners", "bottom_left", "command"],
        ),
        (
            "Bottom right",
            &["hot_corners", "bottom_right", "action"],
            &["hot_corners", "bottom_right", "command"],
        ),
    ];
    let config = state.config.get();
    let enabled = bool_at(&config, &["hot_corners", "enabled"], true);
    let writer = state.writer();
    let mut groups = vec![setting_group(
        "Activation",
        "Leave a corner before activating it again. Commands run in your session.",
        vec![
            row(
                "Enable hot corners",
                Box::new(Switch::new(enabled, move || {
                    writer.update(|config| {
                        put(
                            config,
                            &["hot_corners", "enabled"],
                            toml::Value::Boolean(!enabled),
                        )
                    })
                })),
            ),
            integer_field(
                state,
                "Dwell time (ms)",
                &["hot_corners", "dwell_ms"],
                0,
                Some(10_000),
            ),
            integer_field(
                state,
                "Corner size (px)",
                &["hot_corners", "threshold"],
                1,
                Some(64),
            ),
        ],
    )];
    for &(label, action_path, command_path) in CORNERS {
        let selected = choice_at(&config, action_path, ACTIONS, "none");
        let writer = state.writer();
        let command = str_at(&config, command_path, "").to_owned();
        groups.push(setting_group(
            label,
            "Set a command before choosing Run command.",
            vec![
                row(
                    "Action",
                    choice_control(state, action_path, ACTIONS, selected),
                ),
                row(
                    "Command",
                    Box::new(
                        TextInput::new(command, move |value| {
                            writer.update(|config| {
                                put(config, command_path, toml::Value::String(value))
                            });
                        })
                        .placeholder("foot"),
                    ),
                ),
            ],
        ));
    }
    finish(
        state,
        "Hot corners",
        "Choose an action for each screen corner.",
        groups,
    )
}

pub fn build_shortcuts(_: Size, state: &ShortcutState) -> BoxedWidget {
    let mut body = vec![
        shortcut_group("System", built_in_shortcuts(state)),
        shortcut_group("Custom", custom_shortcuts(state)),
    ];

    let reset_state = state.clone_handles();
    let reset: BoxedWidget = Box::new(Button::styled(
        ButtonVariant::Secondary,
        ButtonSize::Sm,
        "Restore defaults",
        ButtonState::Normal,
        move || {
            reset_state.cancel_capture();
            reset_state.bindings.set(ShellShortcut::defaults());
        },
    ));
    let saved = state.saved.get();
    let mut actions = vec![reset];
    if state.bindings.get() != saved.shortcuts
        || state.custom_bindings.get() != saved.custom_shortcuts
    {
        let save_state = state.clone_handles();
        actions.push(Box::new(Button::styled(
            ButtonVariant::Primary,
            ButtonSize::Sm,
            "Save shortcuts",
            if state.busy.get() {
                ButtonState::Loading
            } else {
                ButtonState::Normal
            },
            move || save_state.save(),
        )) as BoxedWidget);
    }
    body.push(Box::new(jsx! {
        <Flex direction={FlexDirection::Row} justify={creamui_widgets::layout::Justify::End} gap={8.0} children={actions} />
    }) as BoxedWidget);
    let status = state.status.get();
    if !status.is_empty() {
        body.push(Box::new(Text::secondary(status).size(TextSize::Sm)) as BoxedWidget);
    }
    if state.capturing.get().is_some() {
        let cancel = state.clone_handles();
        body.push(Box::new(Button::styled(
            ButtonVariant::Secondary,
            ButtonSize::Sm,
            "Cancel recording",
            ButtonState::Normal,
            move || cancel.cancel_capture(),
        )));
    }
    if let Some(bindings) = state.compositor_bindings.get() {
        body.push(shortcut_group(
            "Compositor (saved in Blair)",
            compositor_shortcuts(state, bindings),
        ));
    } else {
        body.push(Box::new(
            Text::secondary(
                "Persistent compositor bindings require Blair's configuration service.",
            )
            .size(TextSize::Sm),
        ));
    }
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={28.0} children={body} />
    })
}

impl ShortcutState {
    fn clone_handles(&self) -> Self {
        Self {
            bindings: self.bindings.clone(),
            custom_bindings: self.custom_bindings.clone(),
            capturing: self.capturing.clone(),
            status: self.status.clone(),
            saved: self.saved.clone(),
            app: self.app.clone(),
            busy: self.busy.clone(),
            window: self.window.clone(),
            recorded_modifiers: self.recorded_modifiers.clone(),
            compositor_bindings: self.compositor_bindings.clone(),
            compositor_saved: self.compositor_saved.clone(),
        }
    }
}

fn shortcut_group(label: &str, card: BoxedWidget) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={8.0}>
            {section_label(label)}
            {card}
        </Flex>
    })
}

fn built_in_shortcuts(state: &ShortcutState) -> BoxedWidget {
    let rows = state
        .bindings
        .get()
        .into_iter()
        .enumerate()
        .map(|(index, binding)| built_in_shortcut_editor(index, binding, state))
        .collect();
    group(rows)
}

fn built_in_shortcut_editor(
    index: usize,
    binding: ShellShortcut,
    state: &ShortcutState,
) -> BoxedWidget {
    let capture = state.capturing.get() == Some(ShortcutCapture::BuiltIn(index));
    let capture_state = state.clone_handles();
    let record_state = state.clone_handles();
    let label = shortcut_label(&binding.accelerator);
    let recorder = shortcut_recorder(
        if capture {
            "Press shortcut…"
        } else if binding.accelerator.is_empty() {
            "Record shortcut"
        } else {
            &label
        },
        capture,
        move || capture_state.begin_capture(ShortcutCapture::BuiltIn(index)),
        move |accelerator| record_state.finish_capture(accelerator),
    );

    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} padding={"10.5px 16px"}>
            {shortcut_badge(shortcut_icon(binding.action))}
            <Flex grow={1.0}>{Box::new(Text::new(binding.action.label()).font_size(13.0)) as BoxedWidget}</Flex>
            <Flex shrink={0.0}>{recorder}</Flex>
        </Flex>
    })
}

fn shortcut_badge(icon: IconSource) -> BoxedWidget {
    let color = Color::rgb(207, 91, 36);
    Box::new(jsx! {
        <Flex size={(24.0, 24.0)} shrink={0.0} align={creamui_widgets::layout::Align::Center} justify={creamui_widgets::layout::Justify::Center} corner_radius={8.0} background={creamui_core::LinearGradient::new(180.0, color.mix(Color::rgb(255, 255, 255), 0.22), color)}>
            {Box::new(Icon::new(icon, Color::rgb(255, 255, 255)).size(14.0)) as BoxedWidget}
        </Flex>
    })
}

fn shortcut_icon(action: ShortcutAction) -> IconSource {
    use ShortcutAction::*;
    let paths = match action {
        OpenLauncher => r#"<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>"#,
        MinimizeFocused => {
            r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 9h18M8 16h8"/>"#
        }
        NextWindow | PreviousWindow => {
            r#"<path d="m12 3 10 5-10 5L2 8Zm-10 9 10 5 10-5M2 16l10 5 10-5"/>"#
        }
        CloseFocused => {
            r#"<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m9 9 6 6m0-6-6 6"/>"#
        }
        VolumeUp | VolumeDown => {
            r#"<path d="M11 4 6 8H2v8h4l5 4ZM15 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14"/>"#
        }
        BrightnessUp | BrightnessDown => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>"#
        }
    };
    crate::icons::outline(paths)
}

fn shortcut_label(accelerator: &str) -> String {
    accelerator
        .split('+')
        .map(|key| match key {
            "XF86AudioRaiseVolume" => "Volume up",
            "XF86AudioLowerVolume" => "Volume down",
            "XF86MonBrightnessUp" => "Brightness up",
            "XF86MonBrightnessDown" => "Brightness down",
            "Return" => "Enter",
            key => key,
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

fn custom_shortcuts(state: &ShortcutState) -> BoxedWidget {
    let bindings = state.custom_bindings.get();
    let detail = if bindings.is_empty() {
        "None yet".to_owned()
    } else {
        format!("{} configured", bindings.len())
    };
    let mut rows = vec![];
    let add = state.custom_bindings.clone();
    let capture_state = state.clone_handles();
    rows.push(Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} padding={"12px 16px"}>
            {shortcut_badge(crate::icons::outline(r#"<path d="M9 7V4.5A2.5 2.5 0 1 0 6.5 7H17.5A2.5 2.5 0 1 0 15 4.5V19.5A2.5 2.5 0 1 0 17.5 17H6.5A2.5 2.5 0 1 0 9 19.5Z"/>"#))}
            <Flex direction={FlexDirection::Column} grow={1.0} gap={2.0}>
                {Box::new(Text::new("Custom shortcuts").font_size(13.0)) as BoxedWidget}
                {Box::new(Text::secondary(detail).font_size(12.0)) as BoxedWidget}
            </Flex>
            {Box::new(Button::styled(ButtonVariant::Primary, ButtonSize::Sm, "Add shortcut", ButtonState::Normal, move || {
                capture_state.cancel_capture();
                add.update(|items| items.push(CustomShortcut {
                    accelerator: String::new(), command: String::new(), argument: String::new(),
                }));
            }).height(32.0).corner_radius(12.0).padding_left(14.0).padding_right(14.0)) as BoxedWidget}
        </Flex>
    }) as BoxedWidget);
    rows.extend(
        bindings
            .into_iter()
            .enumerate()
            .map(|(index, binding)| custom_shortcut_editor(index, binding, state)),
    );
    group(rows)
}

fn custom_shortcut_editor(
    index: usize,
    binding: CustomShortcut,
    state: &ShortcutState,
) -> BoxedWidget {
    let capture = state.capturing.get() == Some(ShortcutCapture::Custom(index));
    let capture_state = state.clone_handles();
    let record_state = state.clone_handles();
    let label = shortcut_label(&binding.accelerator);
    let recorder = shortcut_recorder(
        if capture {
            "Press shortcut…"
        } else if binding.accelerator.is_empty() {
            "Record shortcut"
        } else {
            &label
        },
        capture,
        move || capture_state.begin_capture(ShortcutCapture::Custom(index)),
        move |accelerator| record_state.finish_capture(accelerator),
    );
    let command_bindings = state.custom_bindings.clone();
    let command: BoxedWidget = Box::new(
        TextInput::new(binding.command, move |value| {
            command_bindings.update(|items| items[index].command = value);
        })
        .placeholder("Command"),
    );
    let argument_bindings = state.custom_bindings.clone();
    let argument: BoxedWidget = Box::new(
        TextInput::new(binding.argument, move |value| {
            argument_bindings.update(|items| items[index].argument = value);
        })
        .placeholder("Argument (optional)"),
    );
    let remove = state.custom_bindings.clone();
    let capture_state = state.clone_handles();
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={12.0} padding={16.0}>
            <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0}>
                <Flex grow={1.0}>{Box::new(Text::new(format!("Shortcut {}", index + 1)).font_size(13.0).bold(true)) as BoxedWidget}</Flex>
                <Flex shrink={0.0}>{recorder}</Flex>
            </Flex>
            {command}
            {argument}
            <Flex direction={FlexDirection::Row} justify={creamui_widgets::layout::Justify::End}>
                {Box::new(Button::styled(ButtonVariant::Destructive, ButtonSize::Sm, "Remove", ButtonState::Normal, move || {
                    capture_state.cancel_capture();
                    remove.update(|items| { if index < items.len() { items.remove(index); } });
                })) as BoxedWidget}
            </Flex>
        </Flex>
    })
}

fn compositor_shortcuts(
    state: &ShortcutState,
    bindings: Vec<blair_protocol::ShortcutBinding>,
) -> BoxedWidget {
    use blair_protocol::{ShortcutArgument, ShortcutBinding, ShortcutCommand};
    let mut rows = Vec::new();
    for (index, binding) in bindings.iter().cloned().enumerate() {
        let capture = state.capturing.get() == Some(ShortcutCapture::Compositor(index));
        let start = state.clone_handles();
        let record = state.clone_handles();
        let label = shortcut_label(&binding.accelerator);
        let recorder = shortcut_recorder(
            if capture {
                "Press shortcut…"
            } else if label.is_empty() {
                "Record shortcut"
            } else {
                &label
            },
            capture,
            move || start.begin_capture(ShortcutCapture::Compositor(index)),
            move |accelerator| record.finish_capture(accelerator),
        );
        let items = state.compositor_bindings.clone();
        let action = SegmentedControl::new(
            ShortcutCommand::ALL
                .iter()
                .position(|command| *command == binding.command)
                .unwrap_or(0),
            move |selected| {
                items.update(|items| {
                    if let Some(binding) = items.as_mut().and_then(|items| items.get_mut(index)) {
                        binding.command = ShortcutCommand::ALL[selected];
                        binding.argument = match binding.command.argument() {
                            ShortcutArgument::None => None,
                            ShortcutArgument::Workspace => Some("1".into()),
                            ShortcutArgument::Command => Some(String::new()),
                        };
                    }
                });
            },
        )
        .option("Close")
        .option("Workspace")
        .option("Move to")
        .option("Command");
        let mut controls: Vec<BoxedWidget> = vec![recorder, Box::new(action)];
        if binding.command.argument() != ShortcutArgument::None {
            let items = state.compositor_bindings.clone();
            controls.push(Box::new(
                TextInput::new(binding.argument.unwrap_or_default(), move |argument| {
                    items.update(|items| {
                        if let Some(binding) = items.as_mut().and_then(|items| items.get_mut(index))
                        {
                            binding.argument = Some(argument);
                        }
                    });
                })
                .placeholder(
                    if binding.command.argument() == ShortcutArgument::Workspace {
                        "Workspace number"
                    } else {
                        "Command"
                    },
                ),
            ));
        }
        let remove = state.clone_handles();
        controls.push(Box::new(Button::styled(
            ButtonVariant::Destructive,
            ButtonSize::Sm,
            "Remove",
            ButtonState::Normal,
            move || {
                remove.cancel_capture();
                remove.compositor_bindings.update(|items| {
                    if let Some(items) = items {
                        if index < items.len() {
                            items.remove(index);
                        }
                    }
                });
            },
        )));
        rows.push(Box::new(jsx! { <Flex direction={FlexDirection::Column} gap={12.0} padding={16.0} children={controls} /> }) as BoxedWidget);
    }
    let add = state.clone_handles();
    rows.push(Box::new(Button::styled(
        ButtonVariant::Secondary,
        ButtonSize::Sm,
        "Add compositor shortcut",
        ButtonState::Normal,
        move || {
            add.cancel_capture();
            add.compositor_bindings.update(|items| {
                if let Some(items) = items {
                    items.push(ShortcutBinding {
                        accelerator: String::new(),
                        command: ShortcutCommand::Close,
                        argument: None,
                    });
                }
            });
        },
    )));
    if bindings != state.compositor_saved.get() {
        let save = state.clone_handles();
        rows.push(Box::new(Button::styled(
            ButtonVariant::Primary,
            ButtonSize::Sm,
            "Save compositor shortcuts",
            if state.busy.get() {
                ButtonState::Loading
            } else {
                ButtonState::Normal
            },
            move || save.save_compositor(),
        )));
    }
    group(rows)
}

fn validate_compositor_shortcuts(
    bindings: &[blair_protocol::ShortcutBinding],
) -> Result<(), String> {
    use blair_protocol::ShortcutArgument;
    if bindings.len() > 256 {
        return Err("At most 256 compositor shortcuts are supported".into());
    }
    let mut accelerators = Vec::new();
    for (index, binding) in bindings.iter().enumerate() {
        let mut accelerator = blair_protocol::ShortcutAccelerator::parse(&binding.accelerator)
            .map_err(|error| format!("Compositor shortcut {}: {error}", index + 1))?;
        accelerator.keys.sort_unstable();
        if accelerators.contains(&accelerator) {
            return Err(format!(
                "Compositor shortcut {} duplicates an earlier binding",
                index + 1
            ));
        }
        accelerators.push(accelerator);
        let valid = match binding.command.argument() {
            ShortcutArgument::None => binding.argument.is_none(),
            ShortcutArgument::Workspace => binding
                .argument
                .as_deref()
                .and_then(|value| value.parse::<u32>().ok())
                .is_some_and(|value| value > 0),
            ShortcutArgument::Command => binding
                .argument
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        };
        if !valid {
            return Err(format!(
                "Invalid argument for compositor shortcut {}",
                index + 1
            ));
        }
    }
    Ok(())
}

impl ShortcutState {
    fn save_compositor(&self) {
        if self.busy.get() {
            return;
        }
        let Some(bindings) = self.compositor_bindings.get() else {
            return;
        };
        if let Err(error) = validate_compositor_shortcuts(&bindings) {
            self.status.set(error);
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.cancel_capture();
        self.busy.set(true);
        let saved = bindings.clone();
        let state = self.clone_handles();
        app.spawn_background(
            move || {
                blair_runtime()?.block_on(async {
                    let client = BlairClient::connect()
                        .await
                        .map_err(|error| error.to_string())?;
                    match client.set_configured_shortcuts(&bindings).await {
                        Ok(true) => Ok(()),
                        Ok(false) => {
                            Err("Blair rejected the bindings or could not persist them".to_owned())
                        }
                        Err(error) => Err(error.to_string()),
                    }
                })
            },
            move |result: Result<(), String>| {
                state.busy.set(false);
                match result {
                    Ok(()) => {
                        state.compositor_saved.set(saved);
                        state
                            .status
                            .set("Compositor shortcuts saved and applied".into());
                    }
                    Err(error) => state.status.set(error),
                }
            },
        );
    }
}

fn shortcut_recorder(
    label: &str,
    capturing: bool,
    on_click: impl Fn() + 'static,
    on_record: impl Fn(String) + 'static,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let style = Style {
        size: creamui_core::layout::Size {
            width: creamui_core::layout::Dimension::Auto,
            height: creamui_core::layout::Dimension::Length(34.0),
        },
        align_items: Some(creamui_core::layout::AlignItems::Center),
        justify_content: Some(creamui_core::layout::JustifyContent::Center),
        ..Default::default()
    };
    let button = RawButton::new(style, on_click)
        .padding_left(8.0)
        .padding_right(8.0)
        .background(if capturing {
            theme.colors.selection_background
        } else {
            Color::rgba(0, 0, 0, 0)
        })
        .corner_radius(8.0)
        .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
        .focus_style(creamui_core::StateStyle::new().outline(theme.colors.accent, 1.0))
        .child(Box::new(Text::secondary(label.to_owned()).font_size(12.0)) as BoxedWidget);
    Box::new(ShortcutRecorder {
        inner: button,
        on_record: Rc::new(on_record),
        capturing,
    })
}

struct ShortcutRecorder {
    inner: RawButton,
    on_record: Rc<dyn Fn(String)>,
    capturing: bool,
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
        let capturing = self.capturing;
        let record = self.on_record.clone();
        Some(Rc::new(move |input| {
            if !capturing {
                return;
            }
            let key = match input.key {
                Key::Char(' ') => "Space".to_owned(),
                Key::Char(',') => "Comma".to_owned(),
                Key::Char('+') => "Plus".to_owned(),
                Key::Char(key) => key.to_ascii_uppercase().to_string(),
                Key::Enter => "Return".to_owned(),
                Key::Escape => "Escape".to_owned(),
                Key::Tab => "Tab".to_owned(),
                Key::Backspace => "Backspace".to_owned(),
                Key::Delete => "Delete".to_owned(),
                Key::Left => "Left".to_owned(),
                Key::Right => "Right".to_owned(),
                Key::Up => "Up".to_owned(),
                Key::Down => "Down".to_owned(),
                Key::PageUp => "PageUp".to_owned(),
                Key::PageDown => "PageDown".to_owned(),
                Key::Home => "Home".to_owned(),
                Key::End => "End".to_owned(),
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
    if !state.available {
        return state.availability();
    }
    if let Some(apply) = state.apply() {
        body.push(apply);
    }
    body.push(state.availability());
    section(title, subtitle, body)
}
/// A quiet label above a related group of controls. The surrounding page
/// header already supplies context, so settings stay scannable rather than
/// reading like documentation.
fn setting_group(title: &str, description: &str, rows: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={8.0}>
            {section_label(title)}
            {group(rows)}
            {Box::new(Text::secondary(description).size(TextSize::Sm).padding_left(4.0).padding_right(4.0)) as BoxedWidget}
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
    let controller = state.numeric_input(path, value.to_string());
    let writer = state.writer();
    controller.on_change(move |_, input| {
        if let Ok(value) = input.parse::<i64>() {
            if value >= min && max.is_none_or(|max| value <= max) {
                writer.update(|c| put(c, path, toml::Value::Integer(value)));
            }
        }
        // The text field may temporarily be empty or below its minimum while
        // the user replaces a value. Keep that draft (and its cursor) rather
        // than rebuilding it from the last valid configuration value.
        Some(input.to_owned())
    });
    row(
        label,
        Box::new(crate::components::form_input(
            TextInput::controlled(&controller).placeholder(&match max {
                Some(max) => format!("{min}–{max}"),
                None => format!("At least {min}"),
            }),
            112.0,
        )) as BoxedWidget,
    )
}

fn ratio_field(state: &WindowState, label: &str, path: &'static [&'static str]) -> BoxedWidget {
    let value = at(&state.config.get(), path)
        .and_then(toml::Value::as_float)
        .unwrap_or(0.5);
    let controller = state.numeric_input(path, value.to_string());
    let writer = state.writer();
    controller.on_change(move |_, input| {
        if let Ok(value) = input.parse::<f64>() {
            if value.is_finite() && (0.1..=0.9).contains(&value) {
                writer.update(|c| put(c, path, toml::Value::Float(value)));
            }
        }
        Some(input.to_owned())
    });
    row(
        label,
        Box::new(crate::components::form_input(
            TextInput::controlled(&controller).placeholder("0.1–0.9"),
            112.0,
        )) as BoxedWidget,
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

#[cfg(test)]
mod shortcut_tests {
    use super::*;
    use creamui_core::{Modifiers, Point, Renderer};
    use creamui_render::SceneRecorder;
    use creamui_theme::{Theme, ThemeProvider};

    fn test_state() -> ShortcutState {
        let config = ShortcutConfig::default();
        ShortcutState {
            saved: Signal::new(config.clone()),
            bindings: Signal::new(config.shortcuts),
            custom_bindings: Signal::new(config.custom_shortcuts),
            capturing: Signal::new(None),
            status: Signal::new(String::new()),
            app: Rc::new(RefCell::new(None)),
            busy: Signal::new(false),
            window: Rc::new(RefCell::new(None)),
            recorded_modifiers: Rc::new(RefCell::new(Default::default())),
            compositor_bindings: Signal::new(None),
            compositor_saved: Signal::new(Vec::new()),
        }
    }

    #[test]
    fn native_recorder_handles_tab_functions_media_modifiers_and_cancellation() {
        use creamui_render::platform::{Key, KeyEvent, Modifiers};
        creamui_reactive::with_context_scope(|| {
            let state = test_state();
            let event = |key, pressed| KeyEvent {
                key,
                pressed,
                synthetic: false,
            };
            assert!(!state.record_native(event(Key::Tab, true), Modifiers::default()));
            for (key, modifiers, expected) in [
                (
                    Key::Tab,
                    Modifiers {
                        alt: true,
                        ..Default::default()
                    },
                    "Alt+Tab",
                ),
                (
                    Key::F12,
                    Modifiers {
                        ctrl: true,
                        ..Default::default()
                    },
                    "Ctrl+F12",
                ),
                (
                    Key::Media("XF86AudioRaiseVolume".into()),
                    Modifiers::default(),
                    "XF86AudioRaiseVolume",
                ),
            ] {
                state.begin_capture(ShortcutCapture::BuiltIn(0));
                assert!(state.record_native(event(key, true), modifiers));
                assert_eq!(state.bindings.peek()[0].accelerator, expected);
                assert!(state.capturing.peek().is_none());
            }
            state.begin_capture(ShortcutCapture::BuiltIn(0));
            state.record_native(event(Key::Super, true), Modifiers::default());
            assert!(state.capturing.peek().is_some());
            state.record_native(event(Key::Super, false), Modifiers::default());
            assert_eq!(state.bindings.peek()[0].accelerator, "Super");
            state.begin_capture(ShortcutCapture::BuiltIn(0));
            state.record_native(event(Key::Control, true), Modifiers::default());
            state.record_native(
                event(Key::Alt, true),
                Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            );
            state.record_native(event(Key::Control, false), Modifiers::default());
            assert_eq!(state.bindings.peek()[0].accelerator, "Ctrl+Alt");
            state.begin_capture(ShortcutCapture::BuiltIn(0));
            state.record_native(
                event(Key::Character("λ".into()), true),
                Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            );
            assert!(state.capturing.peek().is_some());
            assert!(state.status.peek().contains("unsupported"));
            state.record_native(event(Key::Escape, true), Modifiers::default());
            assert!(state.capturing.peek().is_none());
            assert_eq!(state.bindings.peek()[0].accelerator, "Ctrl+Alt");
        });
    }

    #[test]
    fn compositor_editor_validates_arguments_and_records_into_persistent_bindings() {
        use blair_protocol::{ShortcutBinding, ShortcutCommand};
        creamui_reactive::with_context_scope(|| {
            let state = test_state();
            let mut bindings = vec![ShortcutBinding {
                accelerator: "Super+1".into(),
                command: ShortcutCommand::Workspace,
                argument: Some("1".into()),
            }];
            assert!(validate_compositor_shortcuts(&bindings).is_ok());
            bindings[0].argument = Some("0".into());
            assert!(validate_compositor_shortcuts(&bindings).is_err());
            bindings[0].command = ShortcutCommand::Execute;
            bindings[0].argument = Some(" ".into());
            assert!(validate_compositor_shortcuts(&bindings).is_err());
            bindings[0].argument = Some("terminal".into());
            assert!(validate_compositor_shortcuts(&bindings).is_ok());
            bindings.push(bindings[0].clone());
            assert!(validate_compositor_shortcuts(&bindings)
                .unwrap_err()
                .contains("duplicates"));
            bindings.pop();
            state.compositor_bindings.set(Some(bindings));
            state.begin_capture(ShortcutCapture::Compositor(0));
            state.finish_capture("Super+Return".into());
            assert_eq!(
                state.compositor_bindings.peek().unwrap()[0].accelerator,
                "Super+Return"
            );
            assert_eq!(state.bindings.peek(), state.saved.peek().shortcuts);
        });
    }

    fn frame(content: BoxedWidget, size: Size) -> BoxedWidget {
        Box::new(jsx! {
            <Flex direction={FlexDirection::Column} size={(size.width, size.height)}>
                {content}
            </Flex>
        })
    }

    #[test]
    fn shortcut_cards_keep_recording_and_custom_actions_usable() {
        creamui_reactive::with_context_scope(|| {
            crate::components::load_settings_fonts();
            let theme = crate::components::settings_theme(Theme::light());
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let state = test_state();
            let size = Size {
                width: 688.0,
                height: 1000.0,
            };
            let mut renderer = Renderer::new();
            let mut recorder = SceneRecorder::new();
            recorder.begin(688, 1000, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = renderer.render(
                frame(build_shortcuts(size, &state), size),
                size,
                &mut recorder,
            );
            let point = Point { x: 650.0, y: 50.0 };
            scene
                .hit_test(point)
                .expect("right-aligned accelerator is clickable")();
            assert!(state.capturing.peek() == Some(ShortcutCapture::BuiltIn(0)));

            // Updating the recording prompt preserves keyboard focus.
            let focus = scene
                .focus_id_at(scene.focus_hit_test(point).unwrap())
                .unwrap();
            recorder.begin(688, 1000, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = renderer.render(
                frame(build_shortcuts(size, &state), size),
                size,
                &mut recorder,
            );
            let index = scene.focus_index(focus).expect("recorder retains focus");
            scene.on_key_at(index).unwrap()(KeyInput {
                key: Key::Char('k'),
                modifiers: Modifiers {
                    ctrl: true,
                    alt: true,
                    ..Default::default()
                },
            });
            assert_eq!(state.bindings.peek()[0].accelerator, "Ctrl+Alt+K");
            assert_eq!(state.bindings.peek()[1], state.saved.peek().shortcuts[1]);
            assert!(state.capturing.peek().is_none());

            recorder.begin(688, 1000, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = renderer.render(frame(custom_shortcuts(&state), size), size, &mut recorder);
            scene
                .hit_test(Point { x: 650.0, y: 28.0 })
                .expect("add shortcut")();
            assert_eq!(state.custom_bindings.peek().len(), 1);

            // The stacked editor remains usable at a narrower content width.
            let small = Size {
                width: 400.0,
                height: 500.0,
            };
            recorder.begin(400, 500, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let scene = renderer.render(
                frame(
                    custom_shortcut_editor(0, state.custom_bindings.peek()[0].clone(), &state),
                    small,
                ),
                small,
                &mut recorder,
            );
            scene
                .hit_test(Point { x: 350.0, y: 33.0 })
                .expect("custom recorder")();
            assert!(state.capturing.peek() == Some(ShortcutCapture::Custom(0)));
            // Recorder, command, argument, then the remove button.
            scene
                .on_key_at(3)
                .expect("remove button supports keyboard activation")(KeyInput {
                key: Key::Enter,
                modifiers: Modifiers::default(),
            });
            assert!(state.custom_bindings.peek().is_empty());
            assert!(state.capturing.peek().is_none());
        });
    }

    #[test]
    fn obsolete_window_size_defaults_are_removed_from_loaded_configuration() {
        let state = WindowState::from_configuration(Ok(
            "[window]\ndefault_width = 1000\ndefault_height = 700\n".into(),
        ));
        let config = state.config.get();
        assert!(at(&config, &["window", "default_width"]).is_none());
        assert!(at(&config, &["window", "default_height"]).is_none());
    }
}

pub(crate) fn overview() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let view = &context.view;
    let window_settings = &context.window_settings;
    let native = &context.native;
    build(native, view, window_settings)
}

pub(crate) fn layout_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    build_layout(size, window_settings)
}

pub(crate) fn titlebar_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    build_titlebar(size, window_settings)
}

pub(crate) fn advanced_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    build_general(size, window_settings)
}

pub(crate) fn workspaces_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    build_working_area(size, window_settings)
}

pub(crate) fn effects_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    build_effects(size, window_settings)
}

pub(crate) fn focus_view() -> BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let window_settings = &context.window_settings;
    build_focus(size, window_settings)
}

fn build(state: &State, nav: &creamui_router::Router, window: &WindowState) -> BoxedWidget {
    page(
        vec![
            card(
                "Behavior & layout",
                vec![
                    item(
                        "Focus follows mouse",
                        "Focus the window under the pointer",
                        window.focus_policy_control(),
                        || {},
                    ),
                    item(
                        "Default layout",
                        "Applied to new windows",
                        window.layout_control(),
                        || {},
                    ),
                    item(
                        "Tiling gaps",
                        "Spacing between tiled windows",
                        window.gaps_control(),
                        || {},
                    ),
                    item(
                        "Center new floating windows",
                        "Place new floating windows in the center of their work area",
                        window.centered_control(),
                        || {},
                    ),
                ],
            ),
            card(
                "Workspaces & look",
                vec![
                    link(
                        "Workspaces",
                        "Count, wrapping and working area",
                        Section::WorkingArea,
                        nav,
                    ),
                    link(
                        "Decorations",
                        "Titlebars, borders and window controls",
                        Section::Titlebar,
                        nav,
                    ),
                    item(
                        "Effects",
                        "Window and workspace animations",
                        window.effects_control(),
                        || {},
                    ),
                ],
            ),
            card(
                "Additional settings",
                vec![
                    link(
                        "Window focus",
                        "Raise and pointer behavior",
                        Section::Focus,
                        nav,
                    ),
                    link(
                        "Layout settings",
                        "Initial size and master ratio",
                        Section::Layout,
                        nav,
                    ),
                    link(
                        "Effect settings",
                        "Durations and rendering",
                        Section::Effects,
                        nav,
                    ),
                    detail_link(
                        "Window rules",
                        "Match apps and override window behavior",
                        Page::WindowRules,
                        nav,
                    ),
                ],
            ),
            window.availability(),
        ],
        state,
    )
}
pub mod rules;
