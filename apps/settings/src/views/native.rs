//! Native counterparts of the concept's category and drill-down pages.
//! Service discovery and all privileged writes happen outside the UI thread.
use crate::{
    components::{group, section, section_label, update_config},
    users, Section,
};
use coconut_api::settings::{Action, SettingsIntegration, Snapshot, Value};
use coconut_core::{DockPosition, ShellConfig};
use creamui_core::layout::{FlexDirection, Style};
use creamui_core::{BoxedWidget, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::AppHandle;
use creamui_widgets::{
    Button, Icon, RawView, Select, SelectController, Slider, Switch, Symbol, Text, TextController,
    TextInput, TextSize,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

#[derive(Clone, PartialEq, Debug)]
pub enum Page {
    Displays,
    Devices,
    Printers,
    AddPrinter,
    Fonts,
    InstalledApps,
    App(String),
    FileTypes,
    Startup,
    AddStartup,
    Password,
    AddUser,
    OnlineAccounts,
    Fingerprint,
    Camera,
    Microphone,
    Notifications,
    HotCorners,
    WindowRules,
    Updates,
    Storage,
    Activity,
    DateTime,
    Language,
    Reset,
}
impl Page {
    pub fn title(&self) -> &str {
        match self {
            Self::Displays => "Displays",
            Self::Devices => "Connected devices",
            Self::Printers => "Printers",
            Self::AddPrinter => "Add printer",
            Self::Fonts => "Fonts",
            Self::InstalledApps => "Installed applications",
            Self::App(_) => "Application details",
            Self::FileTypes => "File types",
            Self::Startup => "Startup applications",
            Self::AddStartup => "Add startup application",
            Self::Password => "Change password",
            Self::AddUser => "Add user",
            Self::OnlineAccounts => "Online accounts",
            Self::Fingerprint => "Fingerprint",
            Self::Camera => "Camera permissions",
            Self::Microphone => "Microphone permissions",
            Self::Notifications => "Notifications & popups",
            Self::HotCorners => "Hot corners",
            Self::WindowRules => "Window rules",
            Self::Updates => "Updates",
            Self::Storage => "Storage",
            Self::Activity => "Activity monitor",
            Self::DateTime => "Date & time",
            Self::Language => "Language & region",
            Self::Reset => "Reset shell settings",
        }
    }
    pub(super) fn parent(&self) -> Section {
        match self {
            Self::Displays | Self::Devices | Self::Printers | Self::AddPrinter => Section::Hardware,
            Self::Fonts => Section::Personalization,
            Self::InstalledApps
            | Self::App(_)
            | Self::FileTypes
            | Self::Startup
            | Self::AddStartup => Section::Applications,
            Self::Password | Self::AddUser | Self::OnlineAccounts => Section::Users,
            Self::Fingerprint | Self::Camera | Self::Microphone => Section::Privacy,
            Self::Notifications | Self::HotCorners => Section::Desktop,
            Self::WindowRules => Section::Windows,
            _ => Section::System,
        }
    }
}

#[derive(Clone)]
pub struct State {
    pub snapshot: Signal<Snapshot>,
    pub status: Signal<String>,
    busy: Signal<bool>,
    app: Rc<RefCell<Option<AppHandle>>>,
    backend: Arc<dyn SettingsIntegration>,
    selects: Rc<RefCell<BTreeMap<String, SelectController>>>,
    pending: Rc<RefCell<BTreeMap<String, Value>>>,
    accounts: Rc<RefCell<Option<Signal<Vec<users::Account>>>>>,
    pub search: TextController,
    pub hostname: TextController,
    pub timezone: TextController,
    pub printer_name: TextController,
    pub printer_uri: TextController,
    pub username: TextController,
    pub real_name: TextController,
    pub administrator: Signal<bool>,
    pub password: TextController,
    pub confirmation: TextController,
    pub rules: TextController,
}
impl State {
    pub fn new(backend: Arc<dyn SettingsIntegration>) -> Self {
        Self {
            snapshot: Signal::new(Snapshot::default()),
            status: Signal::new(String::new()),
            busy: Signal::new(false),
            app: Rc::new(RefCell::new(None)),
            backend,
            selects: Rc::new(RefCell::new(BTreeMap::new())),
            pending: Rc::new(RefCell::new(BTreeMap::new())),
            accounts: Rc::new(RefCell::new(None)),
            search: TextController::new(""),
            hostname: TextController::new(""),
            timezone: TextController::new(""),
            printer_name: TextController::new(""),
            printer_uri: TextController::new(""),
            username: TextController::new(""),
            real_name: TextController::new(""),
            administrator: Signal::new(false),
            password: TextController::new(""),
            confirmation: TextController::new(""),
            rules: TextController::new(""),
        }
    }
    pub fn start(&self, app: AppHandle, accounts: Signal<Vec<users::Account>>) {
        *self.app.borrow_mut() = Some(app);
        *self.accounts.borrow_mut() = Some(accounts);
        self.refresh();
    }
    fn accept(&self, snapshot: Snapshot) {
        if self.hostname.value().is_empty() {
            self.hostname.set_value(snapshot.fact("hostname"));
        }
        if self.timezone.value().is_empty() {
            if let Some(Value::Text(zone)) = snapshot
                .preferences
                .get("timezone")
                .and_then(|p| p.value.as_ref())
            {
                self.timezone.set_value(zone);
            }
        }
        self.snapshot.set(snapshot);
    }
    pub fn refresh(&self) {
        if self.busy.peek() {
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.busy.set(true);
        let backend = self.backend.clone();
        let state = self.clone();
        app.spawn_background(
            move || (backend.snapshot(), users::list_accounts()),
            move |(snapshot, accounts)| {
                state.accept(snapshot);
                if let Some(target) = state.accounts.borrow().as_ref() {
                    target.set(accounts);
                }
                state.busy.set(false);
                state.flush_pending();
            },
        );
    }
    fn apply(&self, action: Action) {
        if self.busy.peek() {
            if let Action::Set { key, value } = action {
                self.pending.borrow_mut().insert(key, value);
            }
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.busy.set(true);
        self.status.set("Applying changes…".into());
        let backend = self.backend.clone();
        let state = self.clone();
        app.spawn_background(
            move || {
                let result = backend.apply(action);
                let snapshot = backend.snapshot();
                (result, snapshot, users::list_accounts())
            },
            move |(result, snapshot, accounts)| {
                state.accept(snapshot);
                if let Some(target) = state.accounts.borrow().as_ref() {
                    target.set(accounts);
                }
                state.busy.set(false);
                state
                    .status
                    .set(result.unwrap_or_else(|error| format!("Could not apply: {error}")));
                state.flush_pending();
            },
        );
    }
    fn flush_pending(&self) {
        let next = self.pending.borrow_mut().pop_first();
        if let Some((key, value)) = next {
            self.apply(Action::Set { key, value });
        }
    }
    fn select(&self, key: &str, index: usize) -> SelectController {
        let mut controllers = self.selects.borrow_mut();
        let controller = controllers
            .entry(key.into())
            .or_insert_with(|| SelectController::new(index));
        // Keep popup/search state across renders. Backend remains authoritative.
        if !controller.peek_open() && controller.peek_selected() != index {
            controller.select(index);
        }
        controller.clone()
    }
    fn note(&self) -> BoxedWidget {
        let snapshot = self.snapshot.get();
        let status = self.status.get();
        let busy = self.busy.get();
        if status.is_empty() && !busy {
            return empty();
        }
        Box::new(
            Text::secondary(if !snapshot.loaded {
                "Reading system services…".into()
            } else if status.is_empty() {
                "Refreshing…".into()
            } else {
                status
            })
            .size(TextSize::Sm),
        )
    }
    fn control(&self, key: &str) -> BoxedWidget {
        let snapshot = self.snapshot.get();
        let Some(pref) = snapshot.preferences.get(key) else {
            return value(if snapshot.loaded {
                "Not supported"
            } else {
                "Loading…"
            });
        };
        if !pref.writable {
            return value(match &pref.value {
                Some(Value::Bool(v)) => {
                    if *v {
                        "On".into()
                    } else {
                        "Off".into()
                    }
                }
                Some(Value::Number(v)) => format!("{v:.2}"),
                Some(Value::Text(v)) => v.clone(),
                None => "Unavailable".into(),
            });
        }
        let state = self.clone();
        let key = key.to_owned();
        if !pref.choices.is_empty() {
            let current = pref.value.as_ref().and_then(|v| {
                if let Value::Text(v) = v {
                    Some(v.as_str())
                } else {
                    None
                }
            });
            let selected = pref
                .choices
                .iter()
                .position(|c| Some(c.id.as_str()) == current);
            let mut choices = pref.choices.clone();
            let index = selected.unwrap_or_else(|| {
                choices.insert(
                    0,
                    coconut_api::settings::Choice {
                        id: String::new(),
                        label: "Choose…".into(),
                    },
                );
                0
            });
            let labels: Vec<_> = choices.iter().map(|c| c.label.as_str()).collect();
            let controller = self.select(&key, index);
            return Box::new(
                Select::controlled(&labels, controller)
                    .searchable()
                    .width(176.0)
                    .height(34.0)
                    .on_select(move |index| {
                        if let Some(choice) = choices.get(index).filter(|c| !c.id.is_empty()) {
                            state.apply(if key == "browser" {
                                Action::SetDefaultApp {
                                    mime: "x-scheme-handler/https".into(),
                                    desktop_id: choice.id.clone(),
                                }
                            } else {
                                Action::Set {
                                    key: key.clone(),
                                    value: Value::Text(choice.id.clone()),
                                }
                            });
                        }
                    }),
            );
        }
        match &pref.value {
            Some(Value::Bool(checked)) => {
                let checked = *checked;
                Box::new(Switch::new(checked, move || {
                    state.apply(Action::Set {
                        key: key.clone(),
                        value: Value::Bool(!checked),
                    })
                }))
            }
            Some(Value::Number(number))
                if matches!(key.as_str(), "volume" | "brightness" | "text-scale") =>
            {
                let (base, scale) = if key == "text-scale" {
                    (0.75, 1.25)
                } else {
                    (0.0, 1.0)
                };
                Box::new(Slider::new(
                    ((*number - base) / scale).clamp(0.0, 1.0) as f32,
                    move |v| {
                        state.apply(Action::Set {
                            key: key.clone(),
                            value: Value::Number(base + v as f64 * scale),
                        })
                    },
                ))
            }
            Some(Value::Number(v)) => value(format!("{v:.0} seconds")),
            Some(Value::Text(v)) => value(v),
            None => value("Unavailable"),
        }
    }
    fn preference(&self, label: &str, key: &str, hint: &str) -> BoxedWidget {
        let snapshot = self.snapshot.get();
        let reason = snapshot
            .preferences
            .get(key)
            .map(|p| p.reason.as_str())
            .unwrap_or("");
        item(
            label,
            if reason.is_empty() { hint } else { reason },
            self.control(key),
            || {},
        )
    }
}
fn empty() -> BoxedWidget {
    Box::new(RawView::new(Style::default()))
}
fn item(
    label: impl Into<String>,
    hint: impl Into<String>,
    trailing: BoxedWidget,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let label = label.into();
    let glyph = match label.as_str() {
        "Displays" => {
            r#"<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>"#
        }
        "Brightness" => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M12 2v2m0 16v2M2 12h2m16 0h2M5 5l1.5 1.5m11 11L19 19M5 19l1.5-1.5m11-11L19 5"/>"#
        }
        "Sound output" | "Input device" => {
            r#"<path d="M3 9h4l5-4v14l-5-4H3zM16 9a5 5 0 0 1 0 6m3-9a9 9 0 0 1 0 12"/>"#
        }
        "Mouse & touchpad" => {
            r#"<rect x="6" y="2" width="12" height="20" rx="6"/><path d="M12 2v7"/>"#
        }
        "Cursor" => r#"<path d="m4 3 7 18 3-7 7-3Z"/>"#,
        "Keyboard" => {
            r#"<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 13h.01M10 13h.01M14 13h.01M18 13h.01M7 16h10"/>"#
        }
        "Connected devices" => {
            r#"<path d="M12 3v14a3 3 0 1 0 0 6m0-20-3 4m3-4 3 4M12 14l-5-4V7m5 11 5-5v-3"/><circle cx="7" cy="5" r="2"/><rect x="15" y="6" width="4" height="4"/>"#
        }
        "Printers" => r#"<path d="M6 9V3h12v6M6 18H3V9h18v9h-3M6 14h12v8H6zM17 11h.01"/>"#,
        "Power mode" => {
            r#"<rect x="2" y="6" width="18" height="12" rx="2"/><path d="M23 10v4M11 8l-3 5h6l-3 4"/>"#
        }
        "Wallpaper" => {
            r#"<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8" cy="8" r="1"/><path d="m3 17 6-6 5 5 3-3 4 4"/>"#
        }
        "Theme" => r#"<path d="m14 4 6 6M3 21l4-1L21 6l-3-3L4 17Z"/>"#,
        "Icon pack" => {
            r#"<circle cx="6" cy="17" r="3"/><rect x="14" y="14" width="6" height="6" rx="1"/><path d="m12 3 4 6H8Z"/>"#
        }
        "Fonts" => r#"<path d="M4 3h16M12 3v18M8 21h8"/>"#,
        "Sound pack" | "Notifications" => {
            r#"<path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4"/>"#
        }
        "Top panel" => r#"<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M3 8h18"/>"#,
        "Dock position" => {
            r#"<rect x="2" y="3" width="20" height="18" rx="2"/><rect x="6" y="16" width="12" height="3" rx="1"/>"#
        }
        "Widgets" | "Installed applications" => {
            r#"<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>"#
        }
        "Web browser" | "Location services" => {
            r#"<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a18 18 0 0 1 0 20 18 18 0 0 1 0-20"/>"#
        }
        "Storage" => {
            r#"<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 13h18M7 16h.01M11 16h.01"/>"#
        }
        "Activity monitor" => r#"<path d="M2 12h4l3-8 6 16 3-8h4"/>"#,
        "Updates" => {
            r#"<path d="M21 3v6h-6M3 21v-6h6M4 8a8 8 0 0 1 13-4l4 5M3 15l4 5a8 8 0 0 0 13-4"/>"#
        }
        "Date & time" | "Automatic date & time" => {
            r#"<circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/>"#
        }
        "Camera permissions" => {
            r#"<path d="M3 6h4l2-3h6l2 3h4v14H3z"/><circle cx="12" cy="13" r="4"/>"#
        }
        "Microphone permissions" => {
            r#"<rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3M8 22h8"/>"#
        }
        "Password" | "Require password after sleep" => {
            r#"<rect x="4" y="10" width="16" height="12" rx="2"/><path d="M8 10V6a4 4 0 0 1 8 0v4"/>"#
        }
        _ => "",
    };
    let badge = Some(crate::components::icon_badge(
        if glyph.is_empty() {
            crate::components::setting_icon(&label)
        } else {
            crate::icons::outline(glyph)
        },
        crate::components::category_color(),
    ));
    crate::views::connectivity::icon_item(label, hint, badge, trailing, click)
}
fn value(text: impl Into<String>) -> BoxedWidget {
    Box::new(Text::secondary(text.into()).size(TextSize::Sm))
}
fn key_value(label: impl Into<String>, text: impl Into<String>) -> BoxedWidget {
    item(
        label,
        "",
        Box::new(
            Text::secondary(text.into())
                .size(TextSize::Sm)
                .max_width(320.0),
        ),
        || {},
    )
}
fn action(label: &str, click: impl Fn() + 'static) -> BoxedWidget {
    Box::new(Button::new(label, click))
}
fn card(label: &str, rows: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(
        jsx! { <Flex direction={FlexDirection::Column} gap={8.0}> {section_label(label)} {group(rows)} </Flex> },
    )
}
fn page(body: Vec<BoxedWidget>, state: &State) -> BoxedWidget {
    let mut body = body;
    body.push(state.note());
    section("", "", body)
}
fn link(
    label: &str,
    hint: impl Into<String>,
    target: Section,
    nav: &Signal<Section>,
) -> BoxedWidget {
    let nav = nav.clone();
    let theme = creamui_theme::use_theme();
    item(
        label,
        hint,
        Box::new(Icon::new(Symbol::ChevronRight, theme.colors.text_secondary).size(16.0)),
        move || nav.set(target.clone()),
    )
}
fn detail_link(
    label: &str,
    hint: impl Into<String>,
    target: Page,
    nav: &Signal<Section>,
) -> BoxedWidget {
    link(label, hint, Section::Detail(target), nav)
}
fn unsupported(label: &str, reason: &str) -> BoxedWidget {
    item(label, reason, value("Not supported"), || {})
}
fn input(controller: &TextController, placeholder: &str) -> BoxedWidget {
    Box::new(crate::components::form_input(
        TextInput::controlled(controller).placeholder(placeholder),
        240.0,
    ))
}

pub fn hardware(state: &State, nav: &Signal<Section>) -> BoxedWidget {
    let snapshot = state.snapshot.get();
    let tray: coconut_plugin_tray::TrayConfig = coconut_core::modules::load_module("tray");
    let percentage = tray.show_battery_percentage;
    let state_write = state.clone();
    let percentage_control: BoxedWidget = Box::new(Switch::new(percentage, move || {
        let mut tray: coconut_plugin_tray::TrayConfig = coconut_core::modules::load_module("tray");
        tray.show_battery_percentage = !tray.show_battery_percentage;
        let result = coconut_core::modules::save_module("tray", &tray)
            .map_err(|e| e.to_string())
            .and_then(|_| coconut_core::ipc::publish_module_config_changed());
        state_write.status.set(
            result
                .map(|_| "Battery widget updated".into())
                .unwrap_or_else(|e| format!("Could not apply: {e}")),
        );
    }));
    page(
        vec![
            card(
                "Displays & sound",
                vec![
                    detail_link(
                        "Displays",
                        collection_summary(&snapshot, "displays", "No connected displays reported"),
                        Page::Displays,
                        nav,
                    ),
                    state.preference("Brightness", "brightness", "Screen backlight"),
                    state.preference("Sound output", "sound-output", "Default playback device"),
                    state.preference("Volume", "volume", "Output volume"),
                ],
            ),
            card(
                "Input & devices",
                vec![
                    link(
                        "Mouse & touchpad",
                        "Pointer sensitivity, scrolling and gestures",
                        Section::Mouse,
                        nav,
                    ),
                    link(
                        "Keyboard",
                        "Layout, repeat and input preferences",
                        Section::Keyboard,
                        nav,
                    ),
                    detail_link(
                        "Connected devices",
                        format!("{} USB devices", snapshot.entries("devices").len()),
                        Page::Devices,
                        nav,
                    ),
                    detail_link(
                        "Printers",
                        collection_summary(
                            &snapshot,
                            "printers",
                            "Manage printers and add an IPP printer",
                        ),
                        Page::Printers,
                        nav,
                    ),
                ],
            ),
            card(
                "Power",
                vec![
                    state.preference("Power mode", "power-mode", "Performance and battery life"),
                    item(
                        "Show battery percentage",
                        "Show charge next to the battery icon",
                        percentage_control,
                        || {},
                    ),
                    state.preference(
                        "Dim screen when idle",
                        "idle-dim",
                        "Reduce backlight while idle",
                    ),
                ],
            ),
        ],
        state,
    )
}
fn collection_summary(snapshot: &Snapshot, key: &str, fallback: &str) -> String {
    let entries = snapshot.entries(key);
    if entries.is_empty() {
        fallback.into()
    } else {
        entries
            .iter()
            .map(|e| e.name.as_str())
            .take(2)
            .collect::<Vec<_>>()
            .join(" · ")
    }
}
pub fn personalization(
    state: &State,
    config: &Signal<ShellConfig>,
    appearance: &Signal<creamui_theme::ResolvedAppearance>,
    nav: &Signal<Section>,
    appearance_controls: crate::views::personalization::appearance::OverviewControls,
) -> BoxedWidget {
    let config = config.get();
    let selected = appearance.get();
    let crate::views::personalization::appearance::OverviewControls {
        theme,
        style,
        accent,
        corners,
    } = appearance_controls;
    page(
        vec![
            card(
                "Appearance",
                vec![
                    theme,
                    style,
                    link(
                        "Wallpaper",
                        match config.desktop.wallpaper_mode {
                            coconut_core::WallpaperMode::SolidColor => "Solid color".into(),
                            _ => config
                                .desktop
                                .wallpaper
                                .as_ref()
                                .and_then(|p| p.file_stem())
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "Image, slideshow or solid color".into()),
                        },
                        Section::Wallpaper,
                        nav,
                    ),
                    accent,
                ],
            ),
            card(
                "Details",
                vec![
                    link(
                        "Icon pack",
                        config.appearance.icon_theme,
                        Section::IconPack,
                        nav,
                    ),
                    link(
                        "Cursor",
                        config.appearance.cursor_theme,
                        Section::CursorTheme,
                        nav,
                    ),
                    detail_link(
                        "Fonts",
                        format!(
                            "Interface · {} {}",
                            selected
                                .font_family
                                .unwrap_or_else(|| "Plus Jakarta Sans".into()),
                            creamui_theme::use_theme().typography.body
                        ),
                        Page::Fonts,
                        nav,
                    ),
                    link(
                        "Sound pack",
                        config.appearance.sound_theme,
                        Section::Sound,
                        nav,
                    ),
                ],
            ),
            card("Additional settings", vec![corners]),
        ],
        state,
    )
}
pub fn desktop(state: &State, config: &Signal<ShellConfig>, nav: &Signal<Section>) -> BoxedWidget {
    let c = config.get();
    let write = config.clone();
    let panel: BoxedWidget = Box::new(Switch::new(c.statusbar.enabled, move || {
        update_config(&write, |c| c.statusbar.enabled = !c.statusbar.enabled)
    }));
    let positions = [
        DockPosition::Bottom,
        DockPosition::Left,
        DockPosition::Right,
        DockPosition::Top,
    ];
    let selected = positions
        .iter()
        .position(|p| *p == c.dockbar.position)
        .unwrap_or(0);
    let write = config.clone();
    let position: BoxedWidget = Box::new(
        Select::controlled(
            &["Bottom", "Left", "Right", "Top"],
            state.select("dock-position", selected),
        )
        .width(136.0)
        .height(34.0)
        .on_select(move |i| update_config(&write, |c| c.dockbar.position = positions[i])),
    );
    let write = config.clone();
    let size: BoxedWidget = Box::new(Slider::new((c.dockbar.thickness - 32.0) / 64.0, move |v| {
        update_config(&write, |c| c.dockbar.thickness = 32.0 + v * 64.0)
    }));
    page(
        vec![
            card(
                "Top panel",
                vec![
                    item("Top panel", "Show the status bar", panel, || {}),
                    link(
                        "Panel settings",
                        "Position, appearance and contents",
                        Section::Statusbar,
                        nav,
                    ),
                ],
            ),
            card(
                "Dock",
                vec![
                    item("Dock position", "Screen edge", position, || {}),
                    unsupported("Auto-hide dock", "Automatic hiding is not available yet"),
                    item(
                        "Dock size",
                        format!("{:.0} px", c.dockbar.thickness),
                        size,
                        || {},
                    ),
                    link(
                        "Dock settings",
                        "Appearance and contents",
                        Section::Dockbar,
                        nav,
                    ),
                ],
            ),
            card(
                "Extras",
                vec![
                    link(
                        "Widgets",
                        "Clock, weather, launcher and more",
                        Section::Islands,
                        nav,
                    ),
                    detail_link(
                        "Notifications & popups",
                        "Application messages, volume and brightness indicators",
                        Page::Notifications,
                        nav,
                    ),
                    detail_link(
                        "Hot corners",
                        "Actions at screen corners",
                        Page::HotCorners,
                        nav,
                    ),
                    link(
                        "Desktop icons",
                        "Folders, shortcuts and layout",
                        Section::DesktopIcons,
                        nav,
                    ),
                    link(
                        "Status icons",
                        "Network, battery, audio and brightness",
                        Section::Tray,
                        nav,
                    ),
                ],
            ),
        ],
        state,
    )
}
pub fn windows(
    state: &State,
    nav: &Signal<Section>,
    window: &crate::views::windows::WindowState,
) -> BoxedWidget {
    page(
        vec![
            card(
                "Behavior & layout",
                vec![
                    unsupported("Focus follows mouse", "Only click-to-focus is available"),
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
                    unsupported(
                        "Center new floating windows",
                        "Centered placement is not available yet",
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
pub fn applications(state: &State, nav: &Signal<Section>) -> BoxedWidget {
    let snapshot = state.snapshot.get();
    page(
        vec![
            card(
                "Defaults",
                vec![
                    state.preference("Web browser", "browser", "Open web links with"),
                    detail_link(
                        "File types",
                        "Default applications by MIME type",
                        Page::FileTypes,
                        nav,
                    ),
                ],
            ),
            card(
                "Management",
                vec![
                    detail_link(
                        "Installed applications",
                        format!(
                            "{} desktop applications",
                            snapshot.entries("applications").len()
                        ),
                        Page::InstalledApps,
                        nav,
                    ),
                    detail_link(
                        "Startup applications",
                        format!("{} entries", snapshot.entries("startup").len()),
                        Page::Startup,
                        nav,
                    ),
                ],
            ),
            card(
                "Search",
                vec![item(
                    "Find applications",
                    "Search installed applications by name",
                    input(&state.search, "Search applications…"),
                    || {},
                )],
            ),
            if state.search.value().is_empty() {
                empty()
            } else {
                app_list(state, nav, false)
            },
        ],
        state,
    )
}
pub fn accounts(state: &State, accounts: &[users::Account], nav: &Signal<Section>) -> BoxedWidget {
    let username = std::env::var("USER").unwrap_or_default();
    let own = accounts.iter().find(|a| a.username == username);
    let display = own
        .map(|a| {
            if a.real_name.is_empty() {
                a.username.as_str()
            } else {
                a.real_name.as_str()
            }
        })
        .unwrap_or(&username);
    let others = accounts
        .iter()
        .filter(|a| a.username != username)
        .map(|a| {
            link(
                &a.real_name,
                &a.username,
                Section::User(a.username.clone()),
                nav,
            )
        })
        .collect::<Vec<_>>();
    page(
        vec![
            card(
                "You",
                vec![
                    link(display, "Edit profile and avatar", Section::Profile, nav),
                    key_value("Username", &username),
                    key_value(
                        "Account type",
                        own.map(|a| {
                            if a.administrator {
                                "Administrator"
                            } else {
                                "Standard"
                            }
                        })
                        .unwrap_or("Unavailable"),
                    ),
                    detail_link(
                        "Password",
                        "Change the current account password",
                        Page::Password,
                        nav,
                    ),
                ],
            ),
            card(
                "Other users",
                if others.is_empty() {
                    vec![key_value(
                        "Accounts",
                        "No other accounts reported by AccountsService",
                    )]
                } else {
                    others
                },
            ),
            card(
                "Management",
                vec![
                    detail_link(
                        "Add user",
                        "Requires administrator authorization",
                        Page::AddUser,
                        nav,
                    ),
                    detail_link(
                        "Online accounts",
                        "Accounts managed by the active desktop",
                        Page::OnlineAccounts,
                        nav,
                    ),
                ],
            ),
        ],
        state,
    )
}
pub fn privacy(state: &State, nav: &Signal<Section>) -> BoxedWidget {
    let s = state.snapshot.get();
    page(
        vec![
            card(
                "Security",
                vec![
                    detail_link("Fingerprint", s.fact("fingerprint"), Page::Fingerprint, nav),
                    state.preference(
                        "Require password after sleep",
                        "lock-after-sleep",
                        "Session screen-lock policy",
                    ),
                ],
            ),
            card(
                "Permissions",
                vec![
                    state.preference(
                        "Location services",
                        "location",
                        "Applications may request your location",
                    ),
                    detail_link(
                        "Camera permissions",
                        "Saved desktop-portal permissions",
                        Page::Camera,
                        nav,
                    ),
                    detail_link(
                        "Microphone permissions",
                        "Saved permissions and input device",
                        Page::Microphone,
                        nav,
                    ),
                    unsupported(
                        "Usage analytics",
                        "Coconut has no analytics setting or telemetry service to enable",
                    ),
                ],
            ),
        ],
        state,
    )
}
pub fn accessibility(state: &State, window: &crate::views::windows::WindowState) -> BoxedWidget {
    page(
        vec![
            card(
                "Vision",
                vec![
                    state.preference(
                        "High contrast",
                        "contrast",
                        "Increase contrast between interface elements",
                    ),
                    state.preference("Screen zoom", "zoom", "Magnify the desktop"),
                    state.preference("Text size", "text-scale", "Interface text scaling"),
                    window.reduce_motion(state.preference(
                        "Reduce motion",
                        "reduce-motion",
                        "Limit animation and transitions",
                    )),
                ],
            ),
            card(
                "Interaction",
                vec![
                    state.preference(
                        "Visual alerts",
                        "visual-alerts",
                        "Flash the screen instead of ringing a bell",
                    ),
                    state.preference(
                        "Sticky keys",
                        "sticky-keys",
                        "Enter modifier keys one at a time",
                    ),
                ],
            ),
        ],
        state,
    )
}
pub fn system(state: &State, nav: &Signal<Section>) -> BoxedWidget {
    let snapshot = state.snapshot.get();
    page(
        vec![
            card(
                "Maintenance",
                vec![
                    detail_link(
                        "Updates",
                        "Check system and application updates",
                        Page::Updates,
                        nav,
                    ),
                    detail_link("Storage", snapshot.fact("storage"), Page::Storage, nav),
                    detail_link(
                        "Activity monitor",
                        "CPU, memory and running processes",
                        Page::Activity,
                        nav,
                    ),
                ],
            ),
            card(
                "Region",
                vec![
                    state.preference(
                        "Automatic date & time",
                        "ntp",
                        "Synchronize with network time servers",
                    ),
                    detail_link(
                        "Date & time",
                        "Time zone and clock synchronization",
                        Page::DateTime,
                        nav,
                    ),
                    state.preference(
                        "Language",
                        "locale",
                        "Installed system locales; authorization may be required",
                    ),
                    detail_link(
                        "Language & region",
                        "System locale and regional format",
                        Page::Language,
                        nav,
                    ),
                ],
            ),
            card(
                "Reset",
                vec![detail_link(
                    "Reset shell settings",
                    "Only Coconut shell layout; a backup is kept",
                    Page::Reset,
                    nav,
                )],
            ),
        ],
        state,
    )
}
pub fn about(state: &State) -> BoxedWidget {
    let s = state.snapshot.get();
    let write = state.clone();
    let mut identity = vec![item(
        "Hostname",
        "Device name on the network",
        input(&state.hostname, s.fact("hostname")),
        || {},
    )];
    if s.preferences.get("hostname").is_some_and(|p| p.writable) {
        identity.push(item(
            "",
            "Requires authorization",
            action("Apply", move || {
                write.apply(Action::Set {
                    key: "hostname".into(),
                    value: Value::Text(write.hostname.value()),
                })
            }),
            || {},
        ));
    }
    page(
        vec![
            card("Device", identity),
            card(
                "System",
                vec![
                    key_value("Operating system", s.fact("os")),
                    key_value("Kernel", s.fact("kernel")),
                    key_value("Desktop", &s.session),
                ],
            ),
            card(
                "Hardware",
                vec![
                    key_value("Processor", s.fact("processor")),
                    key_value("Logical CPUs", s.fact("cores")),
                    key_value("Memory", s.fact("memory")),
                    key_value("Graphics", s.fact("graphics")),
                ],
            ),
            card(
                "Software",
                vec![key_value("Coconut", env!("CARGO_PKG_VERSION"))],
            ),
        ],
        state,
    )
}
fn entries(state: &State, key: &str, empty_message: &str) -> BoxedWidget {
    let s = state.snapshot.get();
    let rows = s
        .entries(key)
        .iter()
        .map(|e| item(&e.name, &e.description, empty(), || {}))
        .collect::<Vec<_>>();
    group(if rows.is_empty() {
        vec![item(
            if s.loaded {
                empty_message
            } else {
                "Loading…"
            },
            "",
            empty(),
            || {},
        )]
    } else {
        rows
    })
}
fn app_list(state: &State, nav: &Signal<Section>, startup: bool) -> BoxedWidget {
    let s = state.snapshot.get();
    let query = state.search.value().to_lowercase();
    let rows = s
        .entries("applications")
        .iter()
        .filter(|e| {
            query.is_empty()
                || e.name.to_lowercase().contains(&query)
                || e.id.to_lowercase().contains(&query)
        })
        .map(|e| {
            if startup {
                let write = state.clone();
                let id = e.id.clone();
                item(
                    &e.name,
                    &e.description,
                    action("Add", move || {
                        write.apply(Action::AddStartup {
                            desktop_id: id.clone(),
                        })
                    }),
                    || {},
                )
            } else {
                detail_link(&e.name, &e.description, Page::App(e.id.clone()), nav)
            }
        })
        .collect::<Vec<_>>();
    group(if rows.is_empty() {
        vec![key_value(
            "Applications",
            if s.loaded {
                "No matching desktop applications"
            } else {
                "Loading…"
            },
        )]
    } else {
        rows
    })
}
pub fn detail(
    which: &Page,
    state: &State,
    nav: &Signal<Section>,
    config: &Signal<ShellConfig>,
    window: &crate::views::windows::WindowState,
    appearance: &Signal<creamui_theme::ResolvedAppearance>,
    handle: &Rc<RefCell<Option<creamui_render::WindowHandle>>>,
) -> BoxedWidget {
    if *which == Page::Notifications {
        return crate::views::desktop::notifications::build(config);
    }
    let s = state.snapshot.get();
    let refresh = state.clone();
    let body = match which {
        Page::Displays => vec![value("Connected outputs reported by the compositor. DRM fallback lists supported modes, not the active mode."),
            entries(state, "displays", "No connected outputs reported"), value("Changing modes is unavailable until this compositor exposes safe apply/revert support.")],
        Page::Devices => vec![card("USB devices", vec![entries(state, "devices", "No USB devices reported")]), card("Input devices", vec![entries(state, "input-devices", "No input devices reported")])],
        Page::Printers => vec![value(s.fact("printer-service")), entries(state, "printers", "No printers reported"), detail_link("Add printer", "IPP / IPPS · driverless printing", Page::AddPrinter, nav)],
        Page::AddPrinter => { let write = state.clone(); vec![card("Printer", vec![item("Name", "Letters, digits, hyphens and underscores", input(&state.printer_name, "Office-printer"), || {}), item("Address", "Driverless IPP or IPPS endpoint", input(&state.printer_uri, "ipps://printer.local/ipp/print"), || {}),
            item("", "CUPS must be running; permission is checked by the server", action("Add printer", move || write.apply(Action::AddPrinter { name: write.printer_name.value(), uri: write.printer_uri.value() })), || {})])] },
        Page::Fonts => {
            let selected = appearance.get(); let family = selected.font_family.as_deref().unwrap_or("Plus Jakarta Sans").split(',').next().unwrap_or_default().trim();
            let fonts = s.entries("fonts").to_vec(); let labels: Vec<_> = fonts.iter().map(|f| f.name.as_str()).collect();
            let index = fonts.iter().position(|f| f.name == family).unwrap_or(0); let appearance = appearance.clone(); let handle = handle.clone();
            let control = if fonts.is_empty() { value(family) } else { Box::new(Select::controlled(&labels, state.select("interface-font", index)).searchable().on_select(move |i| {
                if let Some(font) = fonts.get(i) { let current = appearance.peek(); crate::views::personalization::appearance::apply(creamui_theme::AppearanceSelection { theme: Some(current.theme_id), variant: Some(current.variant_id), accent: Some(current.accent), font_family: Some(format!("{}, system-ui", font.name)), corners: Some(current.corners) }, &appearance, &handle); }
            })) as BoxedWidget };
            vec![card("Typography", vec![item("Interface font", "Installed fonts and Coconut's bundled font", control, || {}), state.preference("Text scaling", "text-scale", "Controlled by the active desktop")])]
        },
        Page::InstalledApps | Page::AddStartup => vec![input(&state.search, "Search applications…"), app_list(state, nav, matches!(which, Page::AddStartup))],
        Page::App(id) => {
            let app = s.entries("applications").iter().find(|e| &e.id == id);
            app.map(|app| vec![card("Application", vec![key_value("Name", &app.name), key_value("Desktop ID", &app.id), key_value("Description", &app.description),
                key_value("Executable", app.properties.get("Exec").cloned().unwrap_or_default()), key_value("Desktop entry", app.properties.get("path").cloned().unwrap_or_default())])]).unwrap_or_else(|| vec![value("This application is no longer installed")])
        }
        Page::FileTypes => {
            let rows = s.entries("file-types").iter().map(|e| {
                let mut apps = s.entries("applications").iter().filter(|a| a.properties.get("MimeType").is_some_and(|types| types.split(';').any(|mime| mime == e.id))).cloned().collect::<Vec<_>>();
                if apps.is_empty() { return key_value(&e.name, &e.description); }
                let current = e.properties.get("desktop-id");
                let selected = apps.iter().position(|a| Some(&a.id) == current).unwrap_or_else(|| { apps.insert(0, coconut_api::settings::Entry { name: format!("Current: {}", e.description), ..Default::default() }); 0 });
                let labels: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect(); let write = state.clone(); let mime = e.id.clone();
                item(&e.name, &e.description, Box::new(Select::controlled(&labels, state.select(&format!("mime:{}", e.id), selected)).searchable().on_select(move |i| { if let Some(app) = apps.get(i).filter(|app| !app.id.is_empty()) { write.apply(Action::SetDefaultApp { mime: mime.clone(), desktop_id: app.id.clone() }); } })), || {})
            }).collect::<Vec<_>>();
            vec![value("Explicit default associations saved by the desktop. Choose an installed application that advertises the file type."), group(if rows.is_empty() { vec![key_value("Associations", "No explicit defaults reported")] } else { rows })]
        }
        Page::Startup => {
            let rows = s.entries("startup").iter().map(|e| { let write = state.clone(); let id = e.id.clone(); let enabled = e.enabled;
                item(&e.name, &e.description, Box::new(Switch::new(enabled, move || write.apply(Action::SetStartup { id: id.clone(), enabled: !enabled }))), || {}) }).collect::<Vec<_>>();
            vec![value(s.fact("startup-service")), group(if rows.is_empty() { vec![key_value("Startup", "No startup entries reported")] } else { rows }), detail_link("Add application", "Choose an installed desktop application", Page::AddStartup, nav)]
        }
        Page::Password => password_form(state),
        Page::AddUser => account_form(state),
        Page::OnlineAccounts => vec![entries(state, "online-accounts", "No online accounts reported"), value(s.fact("online-accounts-service"))],
        Page::Fingerprint => vec![key_value("Reader", s.fact("fingerprint")), entries(state, "fingerprints", "No enrolled fingerprints reported"), value("Fingerprint enrollment and unlocking are managed by fprintd and the system PAM policy. Coconut does not override authentication policy.")],
        Page::Camera | Page::Microphone => {
            let camera = matches!(which, Page::Camera); let key = if camera { "camera-permissions" } else { "microphone-permissions" };
            let device = if camera { "camera" } else { "microphone" };
            let rows = s.entries(key).iter().map(|e| { let write = state.clone(); let app = e.id.clone(); let allowed = e.enabled;
                item(&e.name, &e.description, Box::new(Switch::new(allowed, move || write.apply(Action::SetDevicePermission { device: device.into(), app: app.clone(), allowed: !allowed }))), || {}) }).collect::<Vec<_>>();
            let mut body = vec![value("Permissions saved by the desktop portal. These permissions do not restrict applications that access hardware directly."), value(s.fact(if camera { "camera-service" } else { "microphone-service" })),
                group(if rows.is_empty() { vec![key_value("Applications", "No saved permissions reported")] } else { rows })];
            if camera { body.push(key_value("Camera devices", s.fact("camera-devices"))); }
            else { body.push(card("Audio input", vec![state.preference("Input device", "sound-input", "Default recording device"), state.preference("Mute microphone", "microphone-muted", "Mute the default audio source")])); }
            body
        }
        Page::Notifications => unreachable!(),
        Page::HotCorners => vec![unsupported("Hot corners", "Blair does not expose a hot-corner service. No synthetic preference is saved.")],
        Page::WindowRules => vec![window.rules(&state.rules)],
        Page::Updates => { let write = state.clone(); vec![value("Checks PackageKit when available, otherwise Flatpak applications. No packages are installed or removed automatically."), action("Check now", move || write.apply(Action::CheckUpdates))] },
        Page::Storage => vec![value("Mounted non-temporary filesystems. Capacities are read from the operating system."), entries(state, "storage", "Storage information unavailable")],
        Page::Activity => vec![value("A snapshot of the 50 processes using the most CPU. Refresh to read current values."), entries(state, "activity", "Process information unavailable")],
        Page::DateTime => { let write = state.clone(); vec![card("Clock", vec![state.preference("Automatic date & time", "ntp", "Network time synchronization"), item("Time zone", "IANA name, such as America/Argentina/Buenos_Aires", input(&state.timezone, "Area/City"), || {}),
            item("", "Requires authorization", action("Apply time zone", move || write.apply(Action::Set { key: "timezone".into(), value: Value::Text(write.timezone.value()) })), || {})]), key_value("Uptime", s.fact("uptime"))] },
        Page::Language => vec![card("System locale", vec![state.preference("Language & region", "locale", "Choose an installed locale; new sessions pick up the change")])],
        Page::Reset => { let state = state.clone(); let config = config.clone(); vec![value("Reset only Coconut's panels, dock, desktop and asset-pack configuration. Your files, accounts, OS, Blair configuration and system theme are not reset. The previous shell configuration is backed up first."),
            action("Back up and reset shell settings", move || {
                match config.peek().reset_with_backup() { Ok(path) => { config.set(ShellConfig::default()); let _ = coconut_core::ipc::publish_shell_config(&config.peek()); state.status.set(format!("Reset complete. Backup: {}", path.display())); }, Err(error) => state.status.set(format!("Could not reset: {error}")) }
            })] },
    };
    let mut body = body;
    body.push(action("Refresh", move || refresh.refresh()));
    page(body, state)
}
fn password_form(state: &State) -> Vec<BoxedWidget> {
    let write = state.clone();
    vec![value("Changes only your own system account password through AccountsService. System authorization is required."),
        card("New password", vec![item("Password", "At least 8 characters", Box::new(TextInput::controlled(&state.password).password().width(260.0)), || {}),
            item("Confirm password", "Enter the same password again", Box::new(TextInput::controlled(&state.confirmation).password().width(260.0)), || {}),
            item("", "", action("Change password", move || {
                let password = write.password.value(); if password != write.confirmation.value() { write.status.set("The passwords do not match".into()); return; }
                if password.len() < 8 { write.status.set("Use at least 8 characters".into()); return; }
                if write.busy.peek() { return; }
                write.password.set_value(""); write.confirmation.set_value(""); write.apply(Action::ChangePassword { password });
            }), || {})])]
}
fn account_form(state: &State) -> Vec<BoxedWidget> {
    let write = state.clone();
    let admin = state.administrator.get();
    let admin_write = state.administrator.clone();
    vec![value("Creates a system account through AccountsService after administrator authorization. Set a password with the system account manager before signing in."),
        card("Account", vec![item("Username", "Lowercase letters, digits, hyphens and underscores", input(&state.username, "username"), || {}), item("Display name", "Name shown on this device", input(&state.real_name, "Full name"), || {}),
            item("Administrator", "Allow administrative operations", Box::new(Switch::new(admin, move || admin_write.set(!admin))), || {}),
            item("", "", action("Create account", move || write.apply(Action::CreateUser { username: write.username.value(), real_name: write.real_name.value(), administrator: write.administrator.peek() })), || {})])]
}

pub(crate) fn parent(section: &Section) -> Section {
    match section {
        Section::Detail(page) => page.parent(),
        Section::IconPack | Section::Sound | Section::Wallpaper | Section::CursorTheme => {
            Section::Personalization
        }
        Section::DesktopIcons
        | Section::Statusbar
        | Section::Dockbar
        | Section::Tray
        | Section::Islands => Section::Desktop,
        Section::Layout
        | Section::Titlebar
        | Section::Compositor
        | Section::WorkingArea
        | Section::Effects
        | Section::Focus => Section::Windows,
        Section::Keyboard | Section::Mouse | Section::Touchpad => Section::Hardware,
        Section::Shortcuts => Section::ShortcutsCategory,
        Section::Profile | Section::User(_) | Section::CreateUser => Section::Users,
        other => other.clone(),
    }
}

pub fn detail_header(current: &Section, nav: &Signal<Section>, width: f32) -> BoxedWidget {
    let parent = parent(current);
    let title = crate::page_title(current);
    let back = nav.clone();
    let decorations = creamui_render::use_window_decorations();
    let controls = decorations.controls;
    let right = if decorations.mode == creamui_render::WindowDecorationMode::Client {
        128.0
    } else if controls.width > 0 && controls.x as f32 > width / 2.0 {
        (width - controls.x as f32 + 16.0).max(36.0)
    } else {
        36.0
    };
    let left = if controls.width > 0 && (controls.x as f32) < width / 2.0 {
        ((controls.x + controls.width) as f32 - 256.0 + 16.0).max(36.0)
    } else {
        36.0
    };
    Box::new(
        jsx! { <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} height={56.0} shrink={0.0} padding_left={left} padding_right={right}>
            {crate::components::back_button(move || back.set(parent.clone()))}
            {Box::new(Text::new(title).font_size(14.0).bold(true)) as BoxedWidget}
        </Flex> },
    )
}
