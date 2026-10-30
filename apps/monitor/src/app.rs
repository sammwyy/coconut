use crate::{
    actions::{self, Action, ProcessAction, ServiceAction},
    system::{self, Process, Snapshot},
    ui::*,
};
use creamui_core::{
    layout::{AlignItems, Dimension, FlexDirection, FlexWrap, JustifyContent, Style},
    BoxedWidget, Size, StateStyle, Styled,
};
use creamui_reactive::{batch, Signal};
use creamui_render::{
    AppBuilder, AppHandle, BlurRegion, RenderBackend, WindowHandle, WindowOptions,
};
use creamui_router::{use_location, use_router, Router, RouterProvider};
use creamui_theme::{use_theme, Color, Theme};
use creamui_widgets::{
    AppTitlebar, RawButton, RawScrollView, RawView, ScrollController, Text, TextController,
    TextInput,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

#[path = "views/network.rs"]
mod network;
#[path = "views/overview.rs"]
mod overview;
#[path = "views/performance.rs"]
mod performance;
#[path = "views/services.rs"]
mod services;
#[path = "views/table.rs"]
mod table;
#[path = "views/tasks.rs"]
mod tasks;

const PAGES: [(&str, &str); 5] = [
    ("/", "Overview"),
    ("/apps", "Apps"),
    ("/performance", "Performance"),
    ("/network", "Networking"),
    ("/services", "Services"),
];
const HISTORY: usize = 30;
const CPU_COLOR: Color = Color::rgb(72, 132, 196);
const MEMORY_COLOR: Color = Color::rgb(151, 111, 193);
const DISK_COLOR: Color = Color::rgb(57, 151, 113);
const GPU_COLOR: Color = Color::rgb(204, 123, 91);
const RX_COLOR: Color = Color::rgb(164, 106, 168);
const TX_COLOR: Color = Color::rgb(68, 154, 177);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Name,
    Pid,
    Cpu,
    Memory,
    Disk,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum Resource {
    Cpu,
    Memory,
    Disk(String),
    Gpu(String),
    Network(String),
}
impl Resource {
    fn key(&self) -> String {
        match self {
            Self::Cpu => "cpu".into(),
            Self::Memory => "memory".into(),
            Self::Disk(id) => format!("disk:{id}"),
            Self::Gpu(id) => format!("gpu:{id}"),
            Self::Network(id) => format!("net:{id}:rx"),
        }
    }
}
#[derive(Clone)]
enum Detail {
    Process(Process),
    Text(String, String),
}

#[derive(Clone)]
struct State {
    snapshot: Signal<Snapshot>,
    histories: Signal<HashMap<String, Vec<f32>>>,
    paused: Signal<bool>,
    refreshing: Signal<bool>,
    app: Rc<RefCell<Option<AppHandle>>>,
    window: Rc<RefCell<Option<WindowHandle>>>,
    scrolls: Rc<RefCell<HashMap<String, ScrollController>>>,
    process_search: TextController,
    app_search: TextController,
    network_search: TextController,
    service_search: TextController,
    sort: Signal<(Sort, bool)>,
    selected_process: Signal<Option<u32>>,
    selected_app: Signal<Option<String>>,
    expanded_app: Signal<Option<String>>,
    task_mode: Signal<usize>,
    resource: Signal<Resource>,
    per_core: Signal<bool>,
    network_mode: Signal<usize>,
    interface: Signal<Option<String>>,
    selected_service: Signal<Option<String>>,
    user_services: Signal<bool>,
    pending: Signal<Option<Action>>,
    detail: Signal<Option<Detail>>,
    action_busy: Signal<bool>,
    message: Signal<Option<(String, bool)>>,
}
impl State {
    fn new(snapshot: Snapshot) -> Self {
        let per_core = snapshot.cpu.cores <= 8;
        let state = Self {
            snapshot: Signal::new(snapshot.clone()),
            histories: Signal::new(HashMap::new()),
            paused: Signal::new(false),
            refreshing: Signal::new(false),
            app: Rc::new(RefCell::new(None)),
            window: Rc::new(RefCell::new(None)),
            scrolls: Rc::new(RefCell::new(HashMap::new())),
            process_search: TextController::new(""),
            app_search: TextController::new(""),
            network_search: TextController::new(""),
            service_search: TextController::new(""),
            sort: Signal::new((Sort::Memory, true)),
            selected_process: Signal::new(None),
            selected_app: Signal::new(None),
            expanded_app: Signal::new(None),
            task_mode: Signal::new(0),
            resource: Signal::new(Resource::Cpu),
            per_core: Signal::new(per_core),
            network_mode: Signal::new(0),
            interface: Signal::new(None),
            selected_service: Signal::new(None),
            user_services: Signal::new(false),
            pending: Signal::new(None),
            detail: Signal::new(None),
            action_busy: Signal::new(false),
            message: Signal::new(None),
        };
        for (query, prefix) in [
            (&state.process_search, "processes"),
            (&state.app_search, "apps"),
            (&state.network_search, "network:"),
            (&state.service_search, "services:"),
        ] {
            let scrolls = state.scrolls.clone();
            query.on_change(move |_, next| {
                for (key, scroll) in scrolls.borrow().iter() {
                    if key.starts_with(prefix) {
                        scroll.set(0.0);
                    }
                }
                Some(next.to_owned())
            });
        }
        state.record(&snapshot);
        state
    }
    fn scroll(&self, key: &str) -> ScrollController {
        self.scrolls
            .borrow_mut()
            .entry(key.into())
            .or_default()
            .clone()
    }
    fn series(&self, key: &str) -> Vec<f32> {
        self.histories.get().get(key).cloned().unwrap_or_default()
    }
    fn record(&self, s: &Snapshot) {
        let mut points = vec![(
            "memory".to_string(),
            percent(
                s.memory.total.saturating_sub(s.memory.available),
                s.memory.total,
            ),
        )];
        if s.cpu.sampled {
            points.push(("cpu".into(), s.cpu.usage));
            for (i, value) in s.cpu.per_core.iter().enumerate() {
                points.push((format!("cpu:{i}"), *value));
            }
        }
        for d in &s.drives {
            if d.sampled {
                points.extend([
                    (format!("disk:{}", d.name), d.usage),
                    (format!("disk:{}:read", d.name), d.read_rate),
                    (format!("disk:{}:write", d.name), d.write_rate),
                ]);
            }
        }
        if s.drives.iter().any(|d| d.sampled) {
            points.push((
                "disk:all".into(),
                s.drives.iter().map(|d| d.usage).fold(0.0_f32, f32::max),
            ));
        }
        for g in &s.gpus {
            if let Some(usage) = g.usage {
                points.push((format!("gpu:{}", g.id), usage));
            }
        }
        let (mut rx, mut tx, mut sampled) = (0.0, 0.0, false);
        for n in &s.network {
            if n.sampled {
                points.extend([
                    (format!("net:{}:rx", n.name), n.receive_rate),
                    (format!("net:{}:tx", n.name), n.transmit_rate),
                ]);
                rx += n.receive_rate;
                tx += n.transmit_rate;
                sampled = true;
            }
        }
        if sampled {
            points.extend([("network:rx".into(), rx), ("network:tx".into(), tx)]);
        }
        self.histories.update(|all| {
            all.retain(|key, _| points.iter().any(|(current, _)| current == key));
            for (key, value) in points {
                let values = all.entry(key).or_default();
                values.push(value);
                if values.len() > HISTORY {
                    values.remove(0);
                }
            }
        });
    }
    fn accept(&self, snapshot: Snapshot) {
        batch(|| {
            self.record(&snapshot);
            self.snapshot.set(snapshot);
            self.refreshing.set(false);
        });
    }
    fn refresh(&self) {
        if self.refreshing.peek() {
            return;
        }
        let Some(app) = self.app.borrow().clone() else {
            return;
        };
        self.refreshing.set(true);
        let state = self.clone();
        let previous = self.snapshot.peek();
        app.spawn_background(
            move || system::collect_since(&previous),
            move |snapshot| state.accept(snapshot),
        );
    }
    fn request(&self, action: Action) {
        if self.action_busy.peek() {
            return;
        }
        self.detail.set(None);
        if action.needs_confirmation() {
            self.pending.set(Some(action));
        } else {
            self.execute(action);
        }
    }
    fn execute(&self, action: Action) {
        self.pending.set(None);
        let Some(app) = self.app.borrow().clone() else {
            self.message.set(Some((
                "The application runtime is unavailable".into(),
                true,
            )));
            return;
        };
        self.action_busy.set(true);
        let state = self.clone();
        app.spawn_background(
            move || action.run(),
            move |result| {
                batch(|| {
                    state.action_busy.set(false);
                    state.message.set(Some(match result {
                        Ok(message) => (message, false),
                        Err(error) => (error, true),
                    }));
                });
                state.refresh();
            },
        );
    }
}

/// Compact logical-pixel default, capped to leave room for the desktop's bars
/// and server frame. CreamUI reports monitor dimensions after scaling.
fn initial_window_size(monitor: Option<(i32, i32)>) -> (u32, u32) {
    // The user's reference client area is 1039 × 648 (excluding KWin's
    // server titlebar). Normalize it to 16:10 on an 8-logical-pixel grid.
    const GRID: u32 = 8;
    const PREFERRED_WIDTH: u32 = 128 * GRID;
    const PREFERRED: (u32, u32) = (PREFERRED_WIDTH, PREFERRED_WIDTH * 10 / 16);
    let Some((width, height)) = monitor.filter(|(w, h)| *w > 0 && *h > 0) else {
        return PREFERRED;
    };
    // Shrink both dimensions together: narrow displays should not change the
    // aspect ratio. Round down to the same grid without exceeding the cap.
    let factor = (width as f64 * 0.85 / PREFERRED.0 as f64)
        .min(height as f64 * 0.85 / PREFERRED.1 as f64)
        .min(1.0);
    let snap = |value: u32| (((value as f64 * factor) as u32 / GRID) * GRID).max(1);
    (snap(PREFERRED.0), snap(PREFERRED.1))
}

pub fn run() {
    let state = State::new(system::collect());
    let provider = RouterProvider::new(router());
    let polling = state.clone();
    let ready = state.clone();
    let theme_window = state.window.clone();
    let listener = Arc::new(Mutex::new(creamui_theme_watch::ThemeListener::start()));
    let (width, height) = initial_window_size(None);
    AppBuilder::new()
        .on_started(move |app| {
            *polling.app.borrow_mut() = Some(app.clone());
            schedule_refresh(app.clone(), polling);
            watch_theme(app, listener, theme_window);
        })
        .window(
            WindowOptions {
                title: "System Monitor".into(),
                width,
                height,
                transparent: true,
                decorations: true,
                blur: Some(BlurRegion::Window),
                theme: load_theme(),
                backend: RenderBackend::Cpu,
                ..Default::default()
            },
            CLEAR,
            move |handle| {
                let size = initial_window_size(handle.monitor_size());
                if size != (width, height) {
                    handle.resize(size.0, size.1);
                }
                let keys = ready.clone();
                handle.on_keyboard_input(move |event, _| {
                    if !event.pressed {
                        return false;
                    }
                    match event.key {
                        creamui_render::platform::Key::F5 => {
                            keys.refresh();
                            true
                        }
                        creamui_render::platform::Key::Escape => {
                            batch(|| {
                                keys.pending.set(None);
                                keys.detail.set(None);
                            });
                            true
                        }
                        _ => false,
                    }
                });
                *ready.window.borrow_mut() = Some(handle);
            },
            move |size| provider.render(|| shell(size, &state)),
        )
        .run();
}
fn router() -> Router {
    let mut builder = Router::builder("/");
    for (path, _) in PAGES {
        builder = builder.route(path, || view(column(0.0), vec![]));
    }
    builder.build().expect("monitor routes")
}
fn schedule_refresh(app: AppHandle, state: State) {
    let next = app.clone();
    app.spawn_background(
        || std::thread::sleep(Duration::from_secs(2)),
        move |_| {
            if !state.paused.peek() {
                state.refresh();
            }
            schedule_refresh(next, state);
        },
    );
}
fn load_theme() -> Theme {
    coconut_integration_cream::load_system_appearance()
        .map(|a| {
            if let Some(family) = &a.font_family {
                creamui_fonts::use_system_font(family);
            }
            a.theme
        })
        .unwrap_or_else(|_| Theme::dark())
}
fn watch_theme(
    app: AppHandle,
    listener: Arc<Mutex<creamui_theme_watch::ThemeListener>>,
    window: Rc<RefCell<Option<WindowHandle>>>,
) {
    let next = app.clone();
    let events = listener.clone();
    app.spawn_background(
        move || events.lock().is_ok_and(|events| events.recv()),
        move |changed| {
            if changed {
                if let Some(window) = window.borrow().as_ref() {
                    window.set_theme(load_theme());
                }
                watch_theme(next, listener, window);
            }
        },
    );
}

fn shell(size: Size, state: &State) -> BoxedWidget {
    let index = PAGES
        .iter()
        .position(|p| p.0 == use_location().pathname)
        .unwrap_or(0);
    let area = Size {
        width: size.width,
        height: (size.height - 53.0).max(0.0),
    };
    let body = match index {
        0 => overview::build(state, area),
        1 => tasks::build(state, area),
        2 => performance::build(state, area),
        3 => network::build(state, area),
        _ => services::build(state, area),
    };
    let mut children = vec![
        topbar(state, size.width, index),
        view(fixed(0.0, 1.0), vec![]),
        body,
    ];
    if let Some((message, error)) = state.message.get() {
        let close = state.message.clone();
        let t = use_theme();
        children.push(Box::new(
            RawView::new(pad(row(12.0), 16.0, 8.0))
                .background(if error {
                    t.colors.danger.with_alpha(0.08)
                } else {
                    t.colors.accent.with_alpha(0.07)
                })
                .with_children(vec![
                    view(grow(), vec![label(message, 12.0, false)]),
                    button("×", 32.0, true, false, move || close.set(None)),
                ]),
        ));
    }
    if let Some(action) = state.pending.get() {
        let cancel = state.pending.clone();
        let confirm = state.clone();
        let mut dialog =
            creamui_widgets::Dialog::new(action.title(), action.description(), move || {
                cancel.set(None)
            })
            .dismiss_action("Cancel");
        dialog = dialog.action(
            "Confirm",
            creamui_widgets::ButtonVariant::Primary,
            move || confirm.execute(action.clone()),
        );
        children.push(Box::new(dialog));
    } else if let Some(detail) = state.detail.get() {
        children.push(detail_dialog(state, size, detail));
    }
    Box::new(
        RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Length(size.width),
                height: Dimension::Length(size.height),
            },
            ..column(0.0)
        })
        .background(use_theme().colors.background_primary())
        .with_children(children),
    )
}
fn topbar(state: &State, width: f32, index: usize) -> BoxedWidget {
    let t = use_theme();
    let wide = width > 950.0;
    let tools = if wide { 160.0 } else { 68.0 };
    let titlebar = AppTitlebar::new(width)
        .height(52.0)
        .padding(8.0)
        .center_margin(tools + 20.0);
    let (start, end) = titlebar.reserved();
    let available = (width - 2.0 * (start.max(end) + tools + 20.0)).max(0.0);
    let tab_width = (available / PAGES.len() as f32).min(116.0);
    let mut colors = creamui_widgets::TabColors::dark();
    colors.background = CLEAR;
    colors.inactive_background = None;
    colors.active_background = t.colors.accent.with_alpha(0.09);
    colors.active_text = t.colors.accent;
    colors.text = t.colors.text_primary;
    colors.muted_text = t.colors.text_secondary;
    colors.radius = t.tab_radius;
    let mut tabs = creamui_widgets::Tabs::new(
        colors,
        Style {
            justify_content: Some(JustifyContent::Center),
            ..row(0.0)
        },
    );
    for (i, (path, title)) in PAGES.iter().enumerate() {
        let router = use_router();
        let path = path.to_string();
        let title = if tab_width < 90.0 && i == 2 {
            "Perf."
        } else if tab_width < 90.0 && i == 3 {
            "Network"
        } else {
            title
        };
        tabs = tabs.child(Box::new(creamui_widgets::Tab::new(
            colors,
            fixed(tab_width, 34.0),
            title,
            i == index,
            move || {
                let _ = router.navigate(&path);
            },
        )));
    }
    let pause = state.clone();
    let refresh = state.clone();
    let paused = state.paused.get();
    let busy = state.refreshing.get();
    let tools = view(
        Style {
            gap: row(4.0).gap,
            ..fixed(tools, 32.0)
        },
        vec![
            icon_button(
                if paused { Glyph::Play } else { Glyph::Pause },
                if wide {
                    Some(if paused { "Resume" } else { "Pause" })
                } else {
                    None
                },
                if wide { 72.0 } else { 32.0 },
                false,
                move || pause.paused.set(!pause.paused.peek()),
            ),
            icon_button(
                Glyph::Refresh,
                if wide { Some("Refresh") } else { None },
                if wide { 84.0 } else { 32.0 },
                busy,
                move || refresh.refresh(),
            ),
        ],
    );
    Box::new(
        titlebar
            .start_boxed(tools)
            .center_boxed(view(fixed(available, 34.0), vec![Box::new(tabs)])),
    )
}

fn fill() -> Style {
    Style {
        flex_grow: 1.0,
        flex_basis: Dimension::Length(0.0),
        flex_shrink: 1.0,
        ..column(0.0)
    }
}
fn content_panel(children: Vec<BoxedWidget>) -> BoxedWidget {
    ContentSurface::boxed(
        Style {
            margin: creamui_core::layout::Rect {
                left: creamui_core::layout::LengthPercentageAuto::Length(8.0),
                right: creamui_core::layout::LengthPercentageAuto::Length(8.0),
                top: creamui_core::layout::LengthPercentageAuto::Length(0.0),
                bottom: creamui_core::layout::LengthPercentageAuto::Length(8.0),
            },
            ..fill()
        },
        children,
    )
}
fn scroll(state: &State, key: &str, children: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(
        RawScrollView::controlled(fill(), state.scroll(key))
            .scrollbar_width(5.0)
            .scrollbar_margin(2.0)
            .child(view(column(0.0), children)),
    )
}
fn button(
    title: &str,
    width: f32,
    enabled: bool,
    danger: bool,
    action: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                ..fixed(width, 30.0)
            },
            action,
        )
        .disabled(!enabled)
        .background(if danger {
            t.colors.danger.with_alpha(0.08)
        } else {
            card_color()
        })
        .corner_radius(t.button_radius)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .focus_style(StateStyle::new().outline(t.colors.accent, 1.5))
        .child(Box::new(Text::new(title).font_size(11.5).color(
            if !enabled {
                t.colors.text_secondary.with_alpha(0.45)
            } else if danger {
                t.colors.danger
            } else {
                t.colors.text_primary
            },
        ))),
    )
}
fn icon_button(
    glyph: Glyph,
    title: Option<&str>,
    width: f32,
    disabled: bool,
    action: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    let mut children = vec![icon(glyph, t.colors.text_secondary, 15.0)];
    if let Some(title) = title {
        children.push(label(title, 11.0, true));
    }
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                gap: row(7.0).gap,
                ..fixed(width, 32.0)
            },
            action,
        )
        .disabled(disabled)
        .background(CLEAR)
        .corner_radius(t.button_radius)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .focus_style(StateStyle::new().outline(t.colors.accent, 1.5))
        .with_children(children),
    )
}
fn search(controller: &TextController, placeholder: &str) -> BoxedWidget {
    Box::new(
        TextInput::controlled_with_style(
            pad(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(1.0),
                        height: Dimension::Length(30.0),
                    },
                    ..row(0.0)
                },
                10.0,
                0.0,
            ),
            controller,
        )
        .placeholder(placeholder)
        .font_size(12.0)
        .corner_radius(use_theme().input_radius),
    )
}
fn page_title(title: &str, detail: impl Into<String>) -> BoxedWidget {
    view(
        pad(row(12.0), 18.0, 14.0),
        vec![
            heading(title, 20.0),
            view(grow(), vec![line(detail, 11.0, true)]),
        ],
    )
}
/// Section controls live on the same shell material as the global tabs.
fn section_header(title: &str, controls: BoxedWidget) -> BoxedWidget {
    view(
        pad(row(16.0), 26.0, 12.0),
        vec![heading(title, 20.0), view(grow(), vec![]), controls],
    )
}
fn empty(title: &str, message: &str) -> BoxedWidget {
    view(
        pad(column(8.0), 20.0, 28.0),
        vec![heading(title, 14.0), label(message, 12.0, true)],
    )
}
fn rate(value: f32) -> String {
    format!("{}/s", system::bytes(value.max(0.0) as u64))
}
fn cpu_value(s: &Snapshot) -> String {
    if s.cpu.sampled {
        format!("{:.0}%", s.cpu.usage)
    } else {
        "—".into()
    }
}
fn process_state(state: &str) -> &str {
    match state {
        "R" => "Running",
        "S" => "Sleeping",
        "D" => "Waiting",
        "T" | "t" => "Paused",
        "Z" => "Zombie",
        "I" => "Idle",
        _ => state,
    }
}
fn matches_process(p: &Process, query: &str) -> bool {
    query.is_empty()
        || p.name.to_lowercase().contains(query)
        || p.command.to_lowercase().contains(query)
        || p.pid.to_string().contains(query)
        || p.user.contains(query)
}
fn info_grid(rows: Vec<(String, String)>) -> BoxedWidget {
    view(
        Style {
            flex_wrap: FlexWrap::Wrap,
            align_items: Some(AlignItems::Start),
            ..row(18.0)
        },
        rows.into_iter()
            .map(|(title, value)| {
                view(
                    Style {
                        flex_grow: 1.0,
                        flex_basis: Dimension::Length(125.0),
                        ..column(4.0)
                    },
                    vec![label(title, 10.5, true), label(value, 13.0, false)],
                )
            })
            .collect(),
    )
}

fn detail_dialog(state: &State, size: Size, detail: Detail) -> BoxedWidget {
    let close = state.detail.clone();
    let mut contents = vec![];
    let (title, message, process) = match detail {
        Detail::Process(p) => {
            let message=format!("PID {} · UID {} · {}\nCPU {:.1}% · Memory {}\nRead {} · Write {}\nParent PID {} · {} threads\n\nExecutable\n{}\n\nCommand\n{}",p.pid,p.user,process_state(&p.state),p.cpu,system::bytes(p.memory),rate(p.read_rate),rate(p.write_rate),p.parent_pid,p.threads,p.executable,p.command);
            (p.name.clone(), message, Some(p))
        }
        Detail::Text(title, message) => (title, message, None),
    };
    contents.push(heading(title, 18.0));
    contents.push(scroll(state, "detail", vec![label(message, 12.0, false)]));
    let mut buttons = vec![];
    if let Some(p) = process {
        let state = state.clone();
        let enabled = actions::can_signal(&p);
        buttons.push(button("Force kill", 90.0, enabled, true, move || {
            state.request(Action::Process(p.clone(), ProcessAction::Kill))
        }));
    }
    buttons.push(view(grow(), vec![]));
    let done = close.clone();
    buttons.push(button("Close", 72.0, true, false, move || done.set(None)));
    contents.push(view(row(8.0), buttons));
    let card = Box::new(
        creamui_widgets::Popover::new(pad(
            Style {
                flex_direction: FlexDirection::Column,
                align_items: Some(AlignItems::Stretch),
                gap: column(16.0).gap,
                ..fixed(
                    (size.width - 40.0).clamp(0.0, 560.0),
                    (size.height - 60.0).clamp(0.0, 440.0),
                )
            },
            20.0,
            20.0,
        ))
        .with_children(contents),
    );
    Box::new(Modal(
        creamui_widgets::Overlay::fullscreen(move || close.set(None)).child(card),
    ))
}

#[cfg(test)]
#[path = "visual_tests.rs"]
mod visual_tests;
