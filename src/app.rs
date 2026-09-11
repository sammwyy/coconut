use crate::bar::{build_dock, BarActions, CreamTheme, SystemStatus, DOCK_HEIGHT};
use crate::integrations::Registry;
use crate::panels::{app_drawer, clock, control_center, current_playing, weather};
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder, WindowHandle, WindowOptions};
use creamui_theme::Color;
use std::rc::Rc;
use std::{cell::RefCell, process::Child, process::Command, time::Duration};

pub fn run() {
    let integrations = Registry::detect();
    let applications = app_drawer::AppCatalog::new();
    let drawer_state = app_drawer::DrawerState::default();
    let initial_windows = integrations.desktop.windows();
    let windows = Signal::new(initial_windows);
    let launcher_open = Signal::new(false);
    let active_window = Signal::new(None);
    let ready_backend = integrations.desktop.clone();
    let playback_backend = integrations.audio.clone();
    let bar_playback_backend = integrations.audio.clone();
    let bar_windows_backend = integrations.desktop.clone();
    let control_network = integrations.network.clone();
    let control_brightness = integrations.brightness.clone();
    let control_battery = integrations.battery.clone();
    let control_volume = integrations.volume.clone();
    let control_bluetooth = integrations.bluetooth.clone();
    let status_network = integrations.network.clone();
    let status_battery = integrations.battery.clone();
    let status_bluetooth = integrations.bluetooth.clone();
    let status_volume = integrations.volume.clone();
    let refresh_backend = integrations.desktop.clone();
    let refresh_windows = windows.clone();
    let refresh = Rc::new(move || {
        let next = refresh_backend.windows();
        refresh_windows.set(next);
    });
    let activate_backend = integrations.desktop.clone();
    let activate = Rc::new(move |id: String| activate_backend.activate_window(&id));

    AppBuilder::new()
        .keep_running()
        .on_started(move |app| {
            applications.start_loading(&app);
            for delay in [Duration::from_secs(1), Duration::from_secs(3)] {
                let backend = bar_windows_backend.clone();
                let target = windows.clone();
                app.spawn_background(
                    move || std::thread::sleep(delay),
                    move |_| target.set(backend.windows()),
                );
            }
            let weather_app = app.clone();
            let weather_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let open_weather = Rc::new(move || {
                if let Some(handle) = weather_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let weather_handle = weather_handle.clone();
                weather_app.append_window(
                    popup_options("CreamShell Weather", weather::WIDTH, weather::HEIGHT),
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *weather_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        window.set_position(54, 924);
                    },
                    weather::build,
                );
            });
            let clock_app = app.clone();
            let clock_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let open_clock = Rc::new(move || {
                if let Some(handle) = clock_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let clock_handle = clock_handle.clone();
                clock_app.append_window(
                    popup_options("CreamShell Clock", clock::WIDTH, clock::HEIGHT),
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *clock_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        window.set_position(1644, 744);
                    },
                    clock::build,
                );
            });
            let music_app = app.clone();
            let music_backend = playback_backend.clone();
            let music_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let open_current_playing = Rc::new(move || {
                if let Some(handle) = music_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let music_handle = music_handle.clone();
                let backend = music_backend.clone();
                music_app.append_window(
                    popup_options(
                        "CreamShell Current Playing",
                        current_playing::WIDTH,
                        current_playing::HEIGHT,
                    ),
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *music_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        window.set_position(172, 908);
                    },
                    move |size| {
                        current_playing::build(size, backend.playback(), {
                            let backend = backend.clone();
                            Rc::new(move || backend.toggle_playback())
                        })
                    },
                );
            });
            let drawer_app = app.clone();
            let drawer_catalog = applications.clone();
            let drawer_state = drawer_state.clone();
            let drawer_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let open_app_drawer = Rc::new(move || {
                if let Some(handle) = drawer_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let drawer_handle = drawer_handle.clone();
                let close_handle = drawer_handle.clone();
                let close_drawer: Rc<dyn Fn()> = Rc::new(move || {
                    if let Some(window) = close_handle.borrow_mut().take() {
                        window.close();
                    }
                });
                let catalog = drawer_catalog.clone();
                let state = drawer_state.clone();
                drawer_app.append_window(
                    popup_options(
                        "CreamShell App Drawer",
                        app_drawer::WIDTH,
                        app_drawer::HEIGHT,
                    ),
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *drawer_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        window.set_position(650, 544);
                    },
                    move |size| {
                        app_drawer::build(
                            size,
                            catalog.clone(),
                            state.clone(),
                            close_drawer.clone(),
                        )
                    },
                );
            });
            let awake_process: Rc<RefCell<Option<Child>>> = Rc::new(RefCell::new(None));
            let keep_awake = {
                let process = awake_process.clone();
                Rc::new(move || {
                    let mut process = process.borrow_mut();
                    if let Some(mut child) = process.take() {
                        let _ = child.kill();
                    } else if let Ok(child) = Command::new("systemd-inhibit")
                        .args([
                            "--what=idle:sleep",
                            "--why=CreamShell",
                            "--mode=block",
                            "sleep",
                            "infinity",
                        ])
                        .spawn()
                    {
                        *process = Some(child);
                    }
                })
            };
            let control_app = app.clone();
            let control_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let open_control_center = Rc::new(move || {
                if let Some(handle) = control_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let control_handle = control_handle.clone();
                let toggle = keep_awake.clone();
                let network = control_network.clone();
                let brightness = control_brightness.clone();
                let battery = control_battery.clone();
                let volume = control_volume.clone();
                let bluetooth = control_bluetooth.clone();
                control_app.append_window(
                    popup_options(
                        "CreamShell Control Center",
                        control_center::WIDTH,
                        control_center::HEIGHT,
                    ),
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *control_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        window.set_position(1544, 588);
                    },
                    move |size| {
                        control_center::build_with_integrations(
                            size,
                            network.clone(),
                            brightness.clone(),
                            battery.clone(),
                            volume.clone(),
                            bluetooth.clone(),
                            toggle.clone(),
                        )
                    },
                );
            });

            app.append_window(
                WindowOptions {
                    title: "CreamShell".into(),
                    width: 800,
                    height: DOCK_HEIGHT,
                    decorations: false,
                    resizable: false,
                    transparent: true,
                    theme: CreamTheme::theme(),
                    ..Default::default()
                },
                Color::rgba(0, 0, 0, 0),
                move |window: WindowHandle| ready_backend.prepare_window(&window),
                move |viewport: Size| -> BoxedWidget {
                    let status = SystemStatus {
                        network_connected: status_network.connected(),
                        network_strength: status_network.strength(),
                        battery_percentage: status_battery.percentage(),
                        battery_charging: status_battery.charging(),
                        bluetooth_powered: status_bluetooth.powered(),
                        bluetooth_connected: status_bluetooth.connected(),
                        volume: status_volume.level(),
                        volume_muted: status_volume.muted(),
                    };
                    build_dock(
                        viewport,
                        windows.get(),
                        launcher_open.clone(),
                        active_window.clone(),
                        status,
                        BarActions {
                            refresh_windows: refresh.clone(),
                            activate_window: activate.clone(),
                            current_playback: {
                                let backend = bar_playback_backend.clone();
                                Rc::new(move || backend.playback())
                            },
                            toggle_playback: {
                                let backend = bar_playback_backend.clone();
                                Rc::new(move || backend.toggle_playback())
                            },
                            open_weather: open_weather.clone(),
                            open_clock: open_clock.clone(),
                            open_current_playing: open_current_playing.clone(),
                            open_app_drawer: open_app_drawer.clone(),
                            open_control_center: open_control_center.clone(),
                        },
                    )
                },
            );
        })
        .run();
}

fn popup_options(title: &str, width: u32, height: u32) -> WindowOptions {
    WindowOptions {
        title: title.into(),
        width,
        height,
        decorations: false,
        resizable: false,
        transparent: true,
        theme: CreamTheme::theme(),
        ..Default::default()
    }
}
