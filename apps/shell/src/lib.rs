mod bar;
mod desktop;
mod plugins;
mod process;

use crate::bar::{build_dock, DOCK_HEIGHT, DOCK_WIDTH};
use crate::plugins::all_plugins;
use chrono::Local;
use coconut_api::audio::{AudioIntegration, Playback};
use coconut_api::desktop::{DesktopIntegration, DesktopWorkArea, OpenWindow, WindowChangeListener};
use coconut_core::{ipc::RuntimeEvent, DockConfig, DockPosition, ShellConfig, UserProfile};
use coconut_plugin_app_drawer::AppCatalog;
use coconut_plugin_app_launcher::{LauncherOpenSignal, WindowListState};
use coconut_plugin_clock::{ClockConfig, ClockText};
use coconut_plugin_current_playing::{
    CurrentPlayback, NextPlayback, PreviousPlayback, SeekPlayback, TogglePlayback,
};
use coconut_plugin_kit::{warn_unknown_islands, PanelHost, PluginInitContext, PluginRegistry};
use coconut_plugin_kit::SharedState;
use coconut_plugin_tray::{
    BatteryRevision, BluetoothPowered, BluetoothRevision, BrightnessLevel, KeepAwakeState,
    NetworkRevision, PowerProfileRevision, ToggleKeepAwake, VolumeLevel, WifiEnabled,
};
use coconut_plugin_weather::WeatherState;
use creamui_core::{BoxedWidget, Point, Size};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder, AppHandle, WindowHandle, WindowOptions};
use creamui_theme::{Color, Theme};
use std::rc::Rc;
use std::{
    cell::RefCell,
    process::Child,
    process::Command,
    sync::{mpsc::Receiver, Arc, Mutex, OnceLock},
    time::Duration,
};

pub fn run() {
    #[cfg(feature = "perf-metrics")]
    creamui_devtools::init_with(creamui_devtools::DevtoolsOptions {
        initially_visible: true,
        ..Default::default()
    });

    initialize_system_theme();
    let initial_config = ShellConfig::load();
    let config = Signal::new(initial_config.clone());
    let runtime_events = Arc::new(Mutex::new(coconut_core::ipc::listen_for_runtime_events()));
    let themed_windows: Rc<RefCell<Vec<WindowHandle>>> = Rc::new(RefCell::new(Vec::new()));
    let dock_windows: Rc<RefCell<Vec<WindowHandle>>> = Rc::new(RefCell::new(Vec::new()));

    let integrations = coconut_registry::detect();
    let desktop_state = desktop::DesktopState::default();
    let desktop_work_area = Signal::new(integrations.desktop.work_area());

    let shared = SharedState::default();

    // -- app launcher / window list -----------------------------------
    let windows = Signal::new(integrations.desktop.windows());
    let launcher_open = Signal::new(false);
    shared.insert(LauncherOpenSignal(launcher_open.clone()));
    {
        let refresh_backend = integrations.desktop.clone();
        let refresh_windows = windows.clone();
        let refresh: Rc<dyn Fn()> = Rc::new(move || refresh_windows.set(refresh_backend.windows()));
        let activate_backend = integrations.desktop.clone();
        let activate: Rc<dyn Fn(String)> =
            Rc::new(move |id: String| activate_backend.activate_window(&id));
        shared.insert(WindowListState {
            windows: windows.clone(),
            refresh,
            activate,
        });
    }

    // -- current playing -------------------------------------------------
    let playback_backend = integrations.audio.clone();
    let playback_state: Signal<Option<Playback>> = Signal::new(playback_backend.playback());
    {
        let state = playback_state.clone();
        shared.insert(CurrentPlayback(Rc::new(move || state.get())));
        let backend = playback_backend.clone();
        shared.insert(TogglePlayback(Rc::new(move || backend.toggle_playback())));
        let backend = playback_backend.clone();
        shared.insert(PreviousPlayback(Rc::new(move || backend.previous())));
        let backend = playback_backend.clone();
        shared.insert(NextPlayback(Rc::new(move || backend.next())));
        let backend = playback_backend.clone();
        let state = playback_state.clone();
        shared.insert(SeekPlayback(Rc::new(move |progress: f64| {
            if let Some(playback) = state.peek() {
                if let Some(length) = parse_playback_time(&playback.length) {
                    backend.seek(progress * length);
                }
            }
        })));
    }

    // -- clock -------------------------------------------------------------
    let clock_config: ClockConfig = coconut_core::modules::load_module("clock");
    let clock_format = Rc::new(clock_config.format.clone());
    let clock_text = Signal::new(Local::now().format(&clock_config.format).to_string());
    shared.insert(ClockText(clock_text.clone()));

    // -- weather -------------------------------------------------------------
    let weather_state: Signal<WeatherState> = Signal::new(WeatherState::Loading);
    shared.insert(weather_state.clone());

    // -- tray / control center ---------------------------------------------
    shared.insert(integrations.network.clone());
    shared.insert(integrations.bluetooth.clone());
    shared.insert(integrations.battery.clone());
    shared.insert(integrations.volume.clone());
    shared.insert(integrations.brightness.clone());
    shared.insert(integrations.power_profile.clone());

    let wifi_enabled = Signal::new(integrations.network.enabled());
    shared.insert(WifiEnabled(wifi_enabled.clone()));
    let bluetooth_powered = Signal::new(integrations.bluetooth.powered());
    shared.insert(BluetoothPowered(bluetooth_powered.clone()));
    let brightness_level = Signal::new(integrations.brightness.level());
    shared.insert(BrightnessLevel(brightness_level.clone()));
    let volume_level = Signal::new(integrations.volume.level());
    shared.insert(VolumeLevel(volume_level.clone()));

    let network_revision = Signal::new(());
    shared.insert(NetworkRevision(network_revision.clone()));
    let bluetooth_revision = Signal::new(());
    shared.insert(BluetoothRevision(bluetooth_revision.clone()));
    let battery_revision = Signal::new(());
    shared.insert(BatteryRevision(battery_revision.clone()));
    let power_profile_revision = Signal::new(());
    shared.insert(PowerProfileRevision(power_profile_revision.clone()));

    let awake = Signal::new(false);
    shared.insert(KeepAwakeState(awake.clone()));
    let awake_process: Rc<RefCell<Option<Child>>> = Rc::new(RefCell::new(None));
    let keep_awake: Rc<dyn Fn()> = {
        let process = awake_process.clone();
        let awake = awake.clone();
        Rc::new(move || {
            let mut process = process.borrow_mut();
            if let Some(mut child) = process.take() {
                let _ = child.kill();
                awake.set(false);
            } else if let Ok(child) = Command::new("systemd-inhibit")
                .args([
                    "--what=idle:sleep",
                    "--why=Coconut",
                    "--mode=block",
                    "sleep",
                    "infinity",
                ])
                .spawn()
            {
                *process = Some(child);
                awake.set(true);
            }
        })
    };
    shared.insert(ToggleKeepAwake(keep_awake));

    let ready_backend = integrations.desktop.clone();
    let bar_playback_backend = integrations.audio.clone();
    let bar_windows_backend = integrations.desktop.clone();
    let poll_brightness = integrations.brightness.clone();
    let poll_volume = integrations.volume.clone();
    let network_changes = integrations.network.changes();
    let bluetooth_changes = integrations.bluetooth.changes();
    let battery_changes = integrations.battery.changes();
    let power_profile_changes = integrations.power_profile.changes();
    let volume_changes = integrations.volume.changes();

    AppBuilder::new()
        .keep_running()
        .on_started(move |app| {
            let plugins = all_plugins();
            let init_ctx = PluginInitContext {
                shared: shared.clone(),
                app: app.clone(),
            };
            let registry = Rc::new(PluginRegistry::build(&plugins, &init_ctx));
            warn_unknown_islands(&initial_config.docks, &registry);

            let panel_host = PanelHost::new(app.clone(), registry.clone(), shared.clone());
            panel_host.set_theme(system_theme());
            let open_panel = panel_host.dispatcher();

            if island_present(&initial_config.docks, "weather") {
                refresh_weather(app.clone(), weather_state.clone());
            }
            desktop_state.start_loading(&app, initial_config.appearance.icon_theme.clone());
            let desktop_for_window = desktop_state.clone();
            app.append_window(
                WindowOptions {
                    title: "Coconut Desktop".into(),
                    width: 1280,
                    height: 720,
                    decorations: false,
                    resizable: false,
                    transparent: false,
                    role: creamui_render::platform::WindowRole::Desktop,
                    theme: system_theme(),
                    ..Default::default()
                },
                system_theme().colors.surface,
                {
                    let themed_windows = themed_windows.clone();
                    let desktop_state = desktop_state.clone();
                    move |window| {
                        desktop_state.set_desktop_window(window.clone());
                        themed_windows.borrow_mut().push(window);
                    }
                },
                {
                    let config = config.clone();
                    let work_area = desktop_work_area.clone();
                    move |viewport| {
                        desktop::build(
                            viewport,
                            desktop_for_window.clone(),
                            config.clone(),
                            work_area.clone(),
                        )
                    }
                },
            );

            schedule_playback_refresh(app.clone(), playback_backend.clone(), playback_state.clone());
            schedule_clock_refresh(app.clone(), clock_text.clone(), clock_format.clone());
            schedule_change_events(app.clone(), network_changes, network_revision.clone());
            schedule_change_events(app.clone(), bluetooth_changes, bluetooth_revision.clone());
            schedule_change_events(app.clone(), battery_changes, battery_revision.clone());
            schedule_change_events(
                app.clone(),
                power_profile_changes,
                power_profile_revision.clone(),
            );
            schedule_brightness_events(app.clone(), poll_brightness, brightness_level.clone());
            schedule_volume_events(app.clone(), poll_volume, volume_changes, volume_level.clone());
            if !schedule_window_events(
                app.clone(),
                bar_windows_backend.clone(),
                windows.clone(),
                desktop_work_area.clone(),
            ) {
                for delay in [Duration::from_secs(1), Duration::from_secs(3)] {
                    let backend = bar_windows_backend.clone();
                    let target = windows.clone();
                    app.spawn_background(
                        move || std::thread::sleep(delay),
                        move |_| target.set(backend.windows()),
                    );
                }
            }

            append_docks(
                &app,
                &initial_config.docks,
                &registry,
                &shared,
                &open_panel,
                &panel_host,
                &ready_backend,
                &themed_windows,
                &dock_windows,
            );

            schedule_runtime_events(
                app.clone(),
                runtime_events.clone(),
                config.clone(),
                themed_windows.clone(),
                registry,
                shared.clone(),
                panel_host,
                open_panel,
                ready_backend.clone(),
                dock_windows.clone(),
                desktop_work_area,
                weather_state,
                desktop_state,
            );
            let _ = bar_playback_backend;
        })
        .run();
}

/// Whether `id` is placed in any *enabled* section of any *enabled*
/// configured dock — the dynamic-dock analog of the old
/// `config.widgets.<id>.enabled` check. A disabled dock/section's islands
/// don't count as present since nothing actually renders them.
fn island_present(docks: &[DockConfig], id: &str) -> bool {
    docks
        .iter()
        .filter(|dock| dock.enabled)
        .flat_map(|dock| &dock.sections)
        .filter(|section| section.enabled)
        .flat_map(|section| &section.islands)
        .any(|entry| entry.id == id)
}

/// (Re)creates every dock window from scratch. Closes whatever dock windows
/// currently exist first — layer-shell roles/geometry can't be changed in
/// place, so any change to the docks list (position, count, sections,
/// islands) is handled by tearing down and rebuilding all of them, the same
/// way the old single-bar `append_bar` closure recreated its one window on
/// a position change.
///
/// Only the first dock feeds [`PanelHost`]'s popup anchor geometry — popups
/// opened from a second/third dock's islands still anchor against the
/// primary dock until multi-dock popup anchoring gets its own design pass.
#[allow(clippy::too_many_arguments)]
fn append_docks(
    app: &AppHandle,
    docks: &[DockConfig],
    registry: &Rc<PluginRegistry>,
    shared: &SharedState,
    open_panel: &Rc<dyn Fn(&'static str, Point)>,
    panel_host: &Rc<PanelHost>,
    ready_backend: &Rc<dyn DesktopIntegration>,
    themed_windows: &Rc<RefCell<Vec<WindowHandle>>>,
    dock_windows: &Rc<RefCell<Vec<WindowHandle>>>,
) {
    for window in dock_windows.borrow_mut().drain(..) {
        window.close();
    }
    let enabled_docks: Vec<&DockConfig> = docks.iter().filter(|dock| dock.enabled).collect();
    for (index, dock) in enabled_docks.into_iter().enumerate() {
        let position = dock.position;
        let role = match position {
            DockPosition::Top => creamui_render::platform::WindowRole::TopPanel,
            DockPosition::Bottom => creamui_render::platform::WindowRole::BottomPanel,
            DockPosition::Left => creamui_render::platform::WindowRole::LeftPanel,
            DockPosition::Right => creamui_render::platform::WindowRole::RightPanel,
        };
        let (width, height) = if position.is_vertical() {
            (DOCK_WIDTH, 800)
        } else {
            (800, DOCK_HEIGHT)
        };
        let is_primary = index == 0;
        let dock = dock.clone();
        let registry = registry.clone();
        let shared = shared.clone();
        let open_panel = open_panel.clone();
        let panel_host = panel_host.clone();
        let ready_backend = ready_backend.clone();
        let themed_windows = themed_windows.clone();
        let dock_windows = dock_windows.clone();
        app.append_window(
            WindowOptions {
                title: "Coconut".into(),
                width,
                height,
                decorations: false,
                resizable: false,
                transparent: true,
                role,
                theme: system_theme(),
                ..Default::default()
            },
            Color::rgba(0, 0, 0, 0),
            move |window: WindowHandle| {
                ready_backend.prepare_window(&window);
                themed_windows.borrow_mut().push(window.clone());
                dock_windows.borrow_mut().push(window.clone());
                if is_primary {
                    panel_host.set_dock_window(Some(window));
                    let thickness = if position.is_vertical() {
                        DOCK_WIDTH as f32
                    } else {
                        DOCK_HEIGHT as f32
                    };
                    panel_host.set_dock_geometry(position, thickness);
                }
            },
            move |viewport: Size| -> BoxedWidget {
                build_dock(viewport, &dock, &registry, &shared, &open_panel)
            },
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn schedule_runtime_events(
    app: AppHandle,
    events: Arc<Mutex<Receiver<RuntimeEvent>>>,
    config: Signal<ShellConfig>,
    themed_windows: Rc<RefCell<Vec<WindowHandle>>>,
    registry: Rc<PluginRegistry>,
    shared: SharedState,
    panel_host: Rc<PanelHost>,
    open_panel: Rc<dyn Fn(&'static str, Point)>,
    ready_backend: Rc<dyn DesktopIntegration>,
    dock_windows: Rc<RefCell<Vec<WindowHandle>>>,
    work_area: Signal<Option<DesktopWorkArea>>,
    weather_state: Signal<WeatherState>,
    desktop_state: desktop::DesktopState,
) {
    let next_app = app.clone();
    let next_events = events.clone();
    let next_config = config.clone();
    let next_windows = themed_windows.clone();
    let next_registry = registry.clone();
    let next_shared = shared.clone();
    let next_panel_host = panel_host.clone();
    let next_open_panel = open_panel.clone();
    let next_ready_backend = ready_backend.clone();
    let next_dock_windows = dock_windows.clone();
    let next_work_area = work_area.clone();
    let next_weather_state = weather_state.clone();
    let next_desktop_state = desktop_state.clone();
    app.clone().spawn_background(
        move || {
            std::thread::sleep(Duration::from_millis(100));
            events
                .lock()
                .ok()
                .and_then(|receiver| receiver.try_recv().ok())
        },
        move |event| {
            if let Some(event) = event {
                match event {
                    RuntimeEvent::ReloadTheme => {
                        let theme = reload_system_theme();
                        panel_host.set_theme(theme);
                        for window in themed_windows.borrow().iter() {
                            window.set_theme(theme);
                        }
                    }
                    RuntimeEvent::ShellConfig(updated) => {
                        let weather_was_enabled = island_present(&config.peek().docks, "weather");
                        let docks_changed = config.peek().docks != updated.docks;
                        warn_unknown_islands(&updated.docks, &registry);
                        let icon_theme_changed =
                            config.peek().appearance.icon_theme != updated.appearance.icon_theme;
                        config.set(updated);
                        if icon_theme_changed {
                            let icon_theme = config.peek().appearance.icon_theme.clone();
                            if let Some(catalog) = shared.get::<AppCatalog>() {
                                catalog.start_loading(&app, icon_theme.clone());
                            }
                            desktop_state.start_loading(&app, icon_theme);
                        }
                        if !weather_was_enabled && island_present(&config.peek().docks, "weather") {
                            refresh_weather(app.clone(), weather_state.clone());
                        }
                        if docks_changed {
                            work_area.set(None);
                            append_docks(
                                &app,
                                &config.peek().docks,
                                &registry,
                                &shared,
                                &open_panel,
                                &panel_host,
                                &ready_backend,
                                &themed_windows,
                                &dock_windows,
                            );
                        }
                    }
                    RuntimeEvent::UserProfileChanged => {
                        if island_present(&config.peek().docks, "weather") {
                            refresh_weather(app.clone(), weather_state.clone());
                        }
                    }
                }
            }
            schedule_runtime_events(
                next_app,
                next_events,
                next_config,
                next_windows,
                next_registry,
                next_shared,
                next_panel_host,
                next_open_panel,
                next_ready_backend,
                next_dock_windows,
                next_work_area,
                next_weather_state,
                next_desktop_state,
            );
        },
    );
}

fn refresh_weather(app: AppHandle, state: Signal<WeatherState>) {
    state.set(WeatherState::Loading);
    app.spawn_background(
        move || {
            let mut profile = UserProfile::load();
            coconut_plugin_weather::refresh(&mut profile)
        },
        move |next| state.set(next),
    );
}

fn schedule_window_events(
    app: AppHandle,
    backend: Rc<dyn DesktopIntegration>,
    windows: Signal<Vec<OpenWindow>>,
    work_area: Signal<Option<DesktopWorkArea>>,
) -> bool {
    let Some(listener) = backend.window_changes() else {
        return false;
    };
    wait_for_window_event(app, backend, windows, work_area, listener);
    true
}

fn wait_for_window_event(
    app: AppHandle,
    backend: Rc<dyn DesktopIntegration>,
    windows: Signal<Vec<OpenWindow>>,
    work_area: Signal<Option<DesktopWorkArea>>,
    listener: WindowChangeListener,
) {
    let waiting_listener = listener.clone();
    let next_app = app.clone();
    app.spawn_background(
        move || waiting_listener.wait(),
        move |changed| {
            if changed {
                windows.set(backend.windows());
                work_area.set(backend.work_area());
                wait_for_window_event(next_app, backend, windows, work_area, listener);
            }
        },
    );
}

fn schedule_playback_refresh(app: AppHandle, backend: Rc<dyn AudioIntegration>, state: Signal<Option<Playback>>) {
    let next_app = app.clone();
    let next_backend = backend.clone();
    let next_state = state.clone();
    app.spawn_background(
        || std::thread::sleep(Duration::from_millis(250)),
        move |_| {
            state.set(backend.playback());
            schedule_playback_refresh(next_app, next_backend, next_state);
        },
    );
}

/// Wakes an open device panel as soon as its integration's native hook (a
/// D-Bus signal, a platform event) observes a change; falls back to a
/// couple-second poll for integrations that expose no such hook.
fn schedule_change_events(app: AppHandle, listener: Option<coconut_api::ChangeListener>, revision: Signal<()>) {
    match listener {
        Some(listener) => wait_for_change_event(app, listener, revision),
        None => schedule_poll_refresh(app, revision),
    }
}

fn wait_for_change_event(app: AppHandle, listener: coconut_api::ChangeListener, revision: Signal<()>) {
    let next_app = app.clone();
    let next_listener = listener.clone();
    let next_revision = revision.clone();
    app.spawn_background(
        move || listener.wait(),
        move |changed| {
            if changed {
                revision.set(());
                wait_for_change_event(next_app, next_listener, next_revision);
            }
        },
    );
}

/// Pulses every couple of seconds so an open device panel re-reads its
/// integration's cached state, for integrations with no native change hook
/// to react to instead.
fn schedule_poll_refresh(app: AppHandle, tick: Signal<()>) {
    let next_app = app.clone();
    let next_tick = tick.clone();
    app.spawn_background(
        || std::thread::sleep(Duration::from_secs(2)),
        move |_| {
            tick.set(());
            schedule_poll_refresh(next_app, next_tick);
        },
    );
}

fn schedule_brightness_events(
    app: AppHandle,
    backend: Rc<dyn coconut_api::brightness::BrightnessIntegration>,
    level: Signal<f32>,
) {
    match backend.changes() {
        Some(listener) => wait_for_brightness_event(app, backend, listener, level),
        None => schedule_brightness_poll(app, backend, level),
    }
}

fn wait_for_brightness_event(
    app: AppHandle,
    backend: Rc<dyn coconut_api::brightness::BrightnessIntegration>,
    listener: coconut_api::ChangeListener,
    level: Signal<f32>,
) {
    let next_app = app.clone();
    let next_backend = backend.clone();
    let next_listener = listener.clone();
    let next_level = level.clone();
    app.spawn_background(
        move || listener.wait(),
        move |changed| {
            if changed {
                level.set(backend.level());
                wait_for_brightness_event(next_app, next_backend, next_listener, next_level);
            }
        },
    );
}

fn schedule_brightness_poll(
    app: AppHandle,
    backend: Rc<dyn coconut_api::brightness::BrightnessIntegration>,
    level: Signal<f32>,
) {
    let next_app = app.clone();
    let next_backend = backend.clone();
    let next_level = level.clone();
    app.spawn_background(
        || std::thread::sleep(Duration::from_secs(2)),
        move |_| {
            level.set(backend.level());
            schedule_brightness_poll(next_app, next_backend, next_level);
        },
    );
}

fn schedule_volume_events(
    app: AppHandle,
    backend: Rc<dyn coconut_api::volume::VolumeIntegration>,
    listener: Option<coconut_api::ChangeListener>,
    level: Signal<f32>,
) {
    match listener {
        Some(listener) => wait_for_volume_event(app, backend, listener, level),
        None => schedule_volume_poll(app, backend, level),
    }
}

fn wait_for_volume_event(
    app: AppHandle,
    backend: Rc<dyn coconut_api::volume::VolumeIntegration>,
    listener: coconut_api::ChangeListener,
    level: Signal<f32>,
) {
    let next_app = app.clone();
    let next_backend = backend.clone();
    let next_listener = listener.clone();
    let next_level = level.clone();
    app.spawn_background(
        move || listener.wait(),
        move |changed| {
            if changed {
                level.set(backend.level());
                wait_for_volume_event(next_app, next_backend, next_listener, next_level);
            }
        },
    );
}

fn schedule_volume_poll(app: AppHandle, backend: Rc<dyn coconut_api::volume::VolumeIntegration>, level: Signal<f32>) {
    let next_app = app.clone();
    let next_backend = backend.clone();
    let next_level = level.clone();
    app.spawn_background(
        || std::thread::sleep(Duration::from_secs(2)),
        move |_| {
            level.set(backend.level());
            schedule_volume_poll(next_app, next_backend, next_level);
        },
    );
}

fn schedule_clock_refresh(app: AppHandle, state: Signal<String>, format: Rc<String>) {
    let next_app = app.clone();
    let next_state = state.clone();
    let next_format = format.clone();
    app.spawn_background(
        || std::thread::sleep(Duration::from_secs(1)),
        move |_| {
            let formatted = Local::now().format(&format).to_string();
            if state.peek() != formatted {
                state.set(formatted);
            }
            schedule_clock_refresh(next_app, next_state, next_format);
        },
    );
}

fn parse_playback_time(value: &str) -> Option<f64> {
    let (minutes, seconds) = value.split_once(':')?;
    Some(minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?)
}

static SYSTEM_THEME: OnceLock<Mutex<Theme>> = OnceLock::new();

fn initialize_system_theme() {
    let theme = load_system_theme();
    let _ = SYSTEM_THEME.set(Mutex::new(theme));
}

fn system_theme() -> Theme {
    *SYSTEM_THEME
        .get_or_init(|| Mutex::new(load_system_theme()))
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn reload_system_theme() -> Theme {
    let theme = load_system_theme();
    *SYSTEM_THEME
        .get_or_init(|| Mutex::new(theme))
        .lock()
        .unwrap_or_else(|error| error.into_inner()) = theme;
    theme
}

fn load_system_theme() -> Theme {
    match creamui_theme_loader::SystemThemeLoader::new().load() {
        Ok(appearance) => appearance.theme,
        Err(error) => {
            eprintln!("shell: failed to load CreamUI system appearance: {error}");
            Theme::default()
        }
    }
}
