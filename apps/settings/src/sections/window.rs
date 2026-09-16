use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use crate::common::{group, row, section};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::{SegmentedControl, Slider};
use dbus::blocking::SyncConnection;

const SERVICE: &str = "org.blair.Compositor";
const PATH: &str = "/org/blair/Compositor";
const INTERFACE: &str = "org.blair.Compositor1";
const DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowSettings {
    titlebar_height: i32,
    border_width: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutSettings {
    layout: String,
    work_area_padding: i32,
    corner_radius: i32,
}

pub struct WindowState {
    settings: Signal<WindowSettings>,
    controller: WindowSettingsController,
    layout: Signal<LayoutSettings>,
    layout_controller: LayoutSettingsController,
}

#[derive(Clone)]
struct LayoutSettingsController {
    pending: Arc<(Mutex<PendingLayoutSettings>, Condvar)>,
}

#[derive(Default)]
struct PendingLayoutSettings {
    value: Option<LayoutSettings>,
    revision: u64,
}

#[derive(Clone)]
struct WindowSettingsController {
    pending: Arc<(Mutex<PendingSettings>, Condvar)>,
}

#[derive(Default)]
struct PendingSettings {
    value: Option<WindowSettings>,
    revision: u64,
}

impl WindowState {
    pub fn connect() -> Option<Self> {
        let settings = read_settings()?;
        let layout = read_layout_settings()?;
        Some(Self {
            settings: Signal::new(settings),
            controller: WindowSettingsController::new(),
            layout: Signal::new(layout),
            layout_controller: LayoutSettingsController::new(),
        })
    }
}

impl LayoutSettingsController {
    fn new() -> Self {
        let pending = Arc::new((Mutex::new(PendingLayoutSettings::default()), Condvar::new()));
        let worker_pending = pending.clone();
        thread::spawn(move || publish_debounced_layout_settings(worker_pending));
        Self { pending }
    }

    fn schedule(&self, settings: LayoutSettings) {
        let (lock, wake) = &*self.pending;
        let Ok(mut pending) = lock.lock() else {
            return;
        };
        pending.value = Some(settings);
        pending.revision = pending.revision.wrapping_add(1);
        wake.notify_one();
    }
}

impl WindowSettingsController {
    fn new() -> Self {
        let pending = Arc::new((Mutex::new(PendingSettings::default()), Condvar::new()));
        let worker_pending = pending.clone();
        thread::spawn(move || publish_debounced_settings(worker_pending));
        Self { pending }
    }

    fn schedule(&self, settings: WindowSettings) {
        let (lock, wake) = &*self.pending;
        let Ok(mut pending) = lock.lock() else {
            return;
        };
        pending.value = Some(settings);
        pending.revision = pending.revision.wrapping_add(1);
        wake.notify_one();
    }
}

pub fn build_layout(_: Size, state: &WindowState) -> BoxedWidget {
    let settings = state.layout.get();
    let selected = usize::from(settings.layout == "tiling");
    let layout = state.layout.clone();
    let controller = state.layout_controller.clone();
    let mode: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index| {
            let next = update_layout(&layout, |current| {
                current.layout = if index == 0 { "floating" } else { "tiling" }.to_string()
            });
            controller.schedule(next);
        })
        .option("Floating")
        .option("Tiling"),
    );
    let radius = corner_radius_row(&settings, state);
    let border = border_width_row(state);
    section(
        "Layout",
        "Choose how Blair places windows and rounds floating frames.",
        vec![group(vec![row("Window mode", mode), border, radius])],
    )
}

fn border_width_row(state: &WindowState) -> BoxedWidget {
    let settings = state.settings.get();
    let signal = state.settings.clone();
    let controller = state.controller.clone();
    let control: BoxedWidget = Box::new(Slider::new(
        settings.border_width as f32 / 16.0,
        move |value| {
            let next = update(&signal, |current| {
                current.border_width = (value * 16.0).round() as i32
            });
            controller.schedule(next);
        },
    ));
    row(
        &format!("Window border ({} px)", settings.border_width),
        control,
    )
}

pub fn build_working_area(_: Size, state: &WindowState) -> BoxedWidget {
    let settings = state.layout.get();
    let layout = state.layout.clone();
    let controller = state.layout_controller.clone();
    let control: BoxedWidget = Box::new(Slider::new(
        settings.work_area_padding as f32 / 128.0,
        move |value| {
            let next = update_layout(&layout, |current| {
                current.work_area_padding = (value * 128.0).round() as i32
            });
            controller.schedule(next);
        },
    ));
    section(
        "Working area",
        "Leave space around tiled windows.",
        vec![group(vec![row(
            &format!("Padding ({} px)", settings.work_area_padding),
            control,
        )])],
    )
}

fn corner_radius_row(settings: &LayoutSettings, state: &WindowState) -> BoxedWidget {
    let selected = match settings.corner_radius {
        0 => 0,
        12 => 1,
        _ => 2,
    };
    let layout = state.layout.clone();
    let controller = state.layout_controller.clone();
    let control: BoxedWidget = Box::new(
        SegmentedControl::new(selected, move |index| {
            let next = update_layout(&layout, |current| {
                current.corner_radius = [0, 12, 24][index]
            });
            controller.schedule(next);
        })
        .option("Square")
        .option("Soft")
        .option("Rounded"),
    );
    row("Corner style", control)
}

fn update(
    settings: &Signal<WindowSettings>,
    mutate: impl FnOnce(&mut WindowSettings),
) -> WindowSettings {
    settings.update(mutate);
    settings.get()
}

fn read_settings() -> Option<WindowSettings> {
    let connection = SyncConnection::new_session().ok()?;
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let (titlebar_height, border_width, _, _): (i32, i32, i32, bool) =
        proxy.method_call(INTERFACE, "WindowSettings", ()).ok()?;
    Some(WindowSettings {
        titlebar_height,
        border_width,
    })
}

fn publish_debounced_settings(pending: Arc<(Mutex<PendingSettings>, Condvar)>) {
    loop {
        let (lock, wake) = &*pending;
        let Ok(mut current) = lock.lock() else {
            return;
        };
        while current.value.is_none() {
            let Ok(next) = wake.wait(current) else {
                return;
            };
            current = next;
        }
        let revision = current.revision;
        let Ok((next, timeout)) = wake.wait_timeout(current, DEBOUNCE) else {
            return;
        };
        current = next;
        if !timeout.timed_out() || current.revision != revision {
            continue;
        }
        let Some(settings) = current.value.take() else {
            continue;
        };
        drop(current);
        if !write_settings(settings) {
            eprintln!("settings: failed to update Blair window settings");
        }
    }
}

fn write_settings(settings: WindowSettings) -> bool {
    let Ok(connection) = SyncConnection::new_session() else {
        return false;
    };
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let Ok((_, _, corner_radius, server_side_decorations)): Result<(i32, i32, i32, bool), _> =
        proxy.method_call(INTERFACE, "WindowSettings", ())
    else {
        return false;
    };
    proxy
        .method_call::<(bool,), _, _, _>(
            INTERFACE,
            "SetWindowSettings",
            (
                settings.titlebar_height,
                settings.border_width,
                corner_radius,
                server_side_decorations,
            ),
        )
        .map(|(updated,)| updated)
        .unwrap_or(false)
}

fn update_layout(
    settings: &Signal<LayoutSettings>,
    mutate: impl FnOnce(&mut LayoutSettings),
) -> LayoutSettings {
    settings.update(mutate);
    settings.get()
}

fn read_layout_settings() -> Option<LayoutSettings> {
    let connection = SyncConnection::new_session().ok()?;
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let (layout, work_area_padding, corner_radius): (String, i32, i32) =
        proxy.method_call(INTERFACE, "LayoutSettings", ()).ok()?;
    Some(LayoutSettings {
        layout,
        work_area_padding,
        corner_radius,
    })
}

fn publish_debounced_layout_settings(pending: Arc<(Mutex<PendingLayoutSettings>, Condvar)>) {
    loop {
        let (lock, wake) = &*pending;
        let Ok(mut current) = lock.lock() else {
            return;
        };
        while current.value.is_none() {
            let Ok(next) = wake.wait(current) else {
                return;
            };
            current = next;
        }
        let revision = current.revision;
        let Ok((next, timeout)) = wake.wait_timeout(current, DEBOUNCE) else {
            return;
        };
        current = next;
        if !timeout.timed_out() || current.revision != revision {
            continue;
        }
        let Some(settings) = current.value.take() else {
            continue;
        };
        drop(current);
        if !write_layout_settings(&settings) {
            eprintln!("settings: failed to update Blair layout settings");
        }
    }
}

fn write_layout_settings(settings: &LayoutSettings) -> bool {
    let Ok(connection) = SyncConnection::new_session() else {
        return false;
    };
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    proxy
        .method_call::<(bool,), _, _, _>(
            INTERFACE,
            "SetLayoutSettings",
            (
                &settings.layout,
                settings.work_area_padding,
                settings.corner_radius,
            ),
        )
        .map(|(updated,)| updated)
        .unwrap_or(false)
}
