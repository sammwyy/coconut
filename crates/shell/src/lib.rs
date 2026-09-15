mod bar;
mod desktop;
mod icons;
mod panels;
mod process;

use crate::bar::{
    build_dock, warn_unknown_widgets, BarActions, CreamTheme, SystemStatus, DOCK_HEIGHT,
};
use crate::panels::{
    app_drawer, bluetooth as bluetooth_panel, brightness as brightness_panel, clock,
    control_center, current_playing, energy as energy_panel, network as network_panel,
    volume as volume_panel, weather,
};
use chrono::Local;
use coconut_api::audio::{AudioIntegration, Playback};
use coconut_core::{BarPosition, ShellConfig};
use creamui_core::{BoxedWidget, Point, Rect, Size};
use creamui_reactive::Signal;
use creamui_render::{AppBuilder, AppHandle, PopupOptions, WindowHandle, WindowOptions};
use creamui_theme::Color;
use creamui_widgets::ScrollController;
use std::rc::Rc;
use std::{cell::RefCell, process::Child, process::Command, time::Duration};

pub fn run() {
    let config = Rc::new(ShellConfig::load());
    warn_unknown_widgets(&config.bar.layout);
    let bar_role = match config.bar.position {
        BarPosition::Top => creamui_render::platform::WindowRole::TopPanel,
        BarPosition::Bottom => creamui_render::platform::WindowRole::BottomPanel,
    };
    let popup_opens_below = matches!(config.bar.position, BarPosition::Top);

    let integrations = coconut_registry::detect();
    let applications = app_drawer::AppCatalog::new();
    let drawer_state = app_drawer::DrawerState::default();
    let desktop_state = desktop::DesktopState::default();
    let initial_windows = integrations.desktop.windows();
    let windows = Signal::new(initial_windows);
    let launcher_open = Signal::new(false);
    let ready_backend = integrations.desktop.clone();
    let playback_backend = integrations.audio.clone();
    let playback_state = Signal::new(playback_backend.playback());
    let bar_window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
    let bar_playback_backend = integrations.audio.clone();
    let bar_windows_backend = integrations.desktop.clone();
    let control_network = integrations.network.clone();
    let control_brightness = integrations.brightness.clone();
    let control_battery = integrations.battery.clone();
    let control_volume = integrations.volume.clone();
    let control_bluetooth = integrations.bluetooth.clone();
    let control_power_profile = integrations.power_profile.clone();
    let panel_network = integrations.network.clone();
    let panel_bluetooth = integrations.bluetooth.clone();
    let panel_battery = integrations.battery.clone();
    let panel_power_profile = integrations.power_profile.clone();
    let panel_brightness = integrations.brightness.clone();
    let panel_volume = integrations.volume.clone();
    let poll_brightness = integrations.brightness.clone();
    let poll_volume = integrations.volume.clone();
    let network_changes = integrations.network.changes();
    let bluetooth_changes = integrations.bluetooth.changes();
    let battery_changes = integrations.battery.changes();
    let power_profile_changes = integrations.power_profile.changes();
    let volume_changes = integrations.volume.changes();
    let brightness_level = Signal::new(integrations.brightness.level());
    let volume_level = Signal::new(integrations.volume.level());
    let wifi_enabled = Signal::new(integrations.network.enabled());
    let bluetooth_powered = Signal::new(integrations.bluetooth.powered());
    let poll_brightness_level = brightness_level.clone();
    let poll_volume_level = volume_level.clone();
    let network_wifi_enabled = wifi_enabled.clone();
    let bluetooth_panel_powered = bluetooth_powered.clone();
    let energy_brightness_level = brightness_level.clone();
    let panel_volume_level = volume_level.clone();
    let clock_format = Rc::new(config.widgets.clock.format.clone());
    let clock_text = Signal::new(Local::now().format(&clock_format).to_string());
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
            let desktop_state = desktop_state.clone();
            desktop_state.start_loading(&app);
            let drag_overlay_state = desktop_state.clone();
            app.append_window(
                WindowOptions {
                    title: "Coconut Desktop".into(),
                    width: 1280,
                    height: 720,
                    decorations: false,
                    resizable: false,
                    transparent: false,
                    role: creamui_render::platform::WindowRole::Desktop,
                    theme: CreamTheme::theme(),
                    ..Default::default()
                },
                desktop::BACKGROUND,
                |_| {},
                move |viewport| desktop::build(viewport, desktop_state.clone()),
            );
            app.append_window(
                WindowOptions {
                    title: "Coconut Drag Overlay".into(),
                    width: 1280,
                    height: 720,
                    decorations: false,
                    resizable: false,
                    transparent: true,
                    role: creamui_render::platform::WindowRole::Overlay,
                    theme: CreamTheme::theme(),
                    ..Default::default()
                },
                Color::rgba(0, 0, 0, 0),
                |_| {},
                move |viewport| desktop::build_drag_overlay(viewport, drag_overlay_state.clone()),
            );
            schedule_playback_refresh(
                app.clone(),
                playback_backend.clone(),
                playback_state.clone(),
            );
            schedule_clock_refresh(app.clone(), clock_text.clone(), clock_format.clone());
            let network_revision = Signal::new(());
            schedule_change_events(app.clone(), network_changes, network_revision.clone());
            let bluetooth_revision = Signal::new(());
            schedule_change_events(app.clone(), bluetooth_changes, bluetooth_revision.clone());
            let battery_revision = Signal::new(());
            schedule_change_events(app.clone(), battery_changes, battery_revision.clone());
            let power_profile_revision = Signal::new(());
            schedule_change_events(
                app.clone(),
                power_profile_changes,
                power_profile_revision.clone(),
            );
            schedule_brightness_events(app.clone(), poll_brightness, poll_brightness_level);
            schedule_volume_events(app.clone(), poll_volume, volume_changes, poll_volume_level);
            let bar_network_revision = network_revision.clone();
            let bar_bluetooth_revision = bluetooth_revision.clone();
            let bar_battery_revision = battery_revision.clone();
            let control_network_revision = network_revision.clone();
            let control_bluetooth_revision = bluetooth_revision.clone();
            let control_battery_revision = battery_revision.clone();
            let control_power_profile_revision = power_profile_revision.clone();
            let network_panel_revision = network_revision.clone();
            let bluetooth_panel_revision = bluetooth_revision.clone();
            let energy_panel_revision = battery_revision.clone();
            let energy_panel_power_profile_revision = power_profile_revision.clone();
            applications.start_loading(&app);
            if !schedule_window_events(app.clone(), bar_windows_backend.clone(), windows.clone()) {
                for delay in [Duration::from_secs(1), Duration::from_secs(3)] {
                    let backend = bar_windows_backend.clone();
                    let target = windows.clone();
                    app.spawn_background(
                        move || std::thread::sleep(delay),
                        move |_| target.set(backend.windows()),
                    );
                }
            }
            let weather_app = app.clone();
            let weather_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let weather_bar = bar_window.clone();
            let open_weather = Rc::new(move |anchor: Point| {
                if let Some(handle) = weather_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let weather_handle = weather_handle.clone();
                let bar_for_popup = weather_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                weather_app.append_popup(
                    popup_options("Coconut Weather", weather::WIDTH, weather::HEIGHT),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *weather_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    weather::build,
                );
            });
            let clock_app = app.clone();
            let clock_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let clock_bar = bar_window.clone();
            let open_clock = Rc::new(move |anchor: Point| {
                if let Some(handle) = clock_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let clock_handle = clock_handle.clone();
                let bar_for_popup = clock_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                clock_app.append_popup(
                    popup_options("Coconut Clock", clock::WIDTH, clock::HEIGHT),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *clock_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    clock::build,
                );
            });
            let music_app = app.clone();
            let music_backend = playback_backend.clone();
            let music_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let music_bar = bar_window.clone();
            let open_current_playing = Rc::new(move |anchor: Point| {
                if let Some(handle) = music_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let music_handle = music_handle.clone();
                let backend = music_backend.clone();
                let playback_state = playback_state.clone();
                let bar_for_popup = music_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                music_app.append_popup(
                    popup_options(
                        "Coconut Current Playing",
                        current_playing::WIDTH,
                        current_playing::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *music_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        current_playing::build(
                            size,
                            playback_state.get(),
                            {
                                let backend = backend.clone();
                                Rc::new(move || backend.toggle_playback())
                            },
                            {
                                let backend = backend.clone();
                                Rc::new(move || backend.previous())
                            },
                            {
                                let backend = backend.clone();
                                Rc::new(move || backend.next())
                            },
                            {
                                let backend = backend.clone();
                                let playback_state = playback_state.clone();
                                Rc::new(move |progress| {
                                    if let Some(playback) = playback_state.peek() {
                                        if let Some(length) = parse_playback_time(&playback.length)
                                        {
                                            backend.seek(progress * length);
                                        }
                                    }
                                })
                            },
                        )
                    },
                );
            });
            let drawer_app = app.clone();
            let drawer_catalog = applications.clone();
            let drawer_state = drawer_state.clone();
            let drawer_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let drawer_bar = bar_window.clone();
            let open_app_drawer = Rc::new(move |anchor: Point| {
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
                let bar_for_popup = drawer_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                drawer_app.append_popup(
                    popup_options(
                        "Coconut App Drawer",
                        app_drawer::WIDTH,
                        app_drawer::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *drawer_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
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
            let awake = Signal::new(false);
            let keep_awake = {
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
            let energy_awake = awake.clone();
            let energy_toggle_awake = keep_awake.clone();
            let control_app = app.clone();
            let control_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let dock_volume = volume_level.clone();
            let control_bar = bar_window.clone();
            let control_tray_config = config.clone();
            let open_control_center = Rc::new(move |anchor: Point| {
                if let Some(handle) = control_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let control_handle = control_handle.clone();
                let toggle = keep_awake.clone();
                let awake = awake.clone();
                let network = control_network.clone();
                let brightness = control_brightness.clone();
                let battery = control_battery.clone();
                let volume = control_volume.clone();
                let bluetooth = control_bluetooth.clone();
                let power_profile = control_power_profile.clone();
                let brightness_level = brightness_level.clone();
                let volume_level = volume_level.clone();
                let wifi_enabled = wifi_enabled.clone();
                let bluetooth_powered = bluetooth_powered.clone();
                let control_tray_config = control_tray_config.clone();
                let network_revision = control_network_revision.clone();
                let bluetooth_revision = control_bluetooth_revision.clone();
                let battery_revision = control_battery_revision.clone();
                let power_profile_revision = control_power_profile_revision.clone();
                let view = Signal::new(control_center::PanelView::Main);
                let network_scroll = ScrollController::new(0.0);
                let bluetooth_scroll = ScrollController::new(0.0);
                let network_detail = Signal::new(None);
                let bluetooth_detail = Signal::new(None);
                let network_password = Signal::new(None);
                brightness_level.set(brightness.level());
                volume_level.set(volume.level());
                wifi_enabled.set(network.enabled());
                bluetooth_powered.set(bluetooth.powered());
                let bar_for_popup = control_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                control_app.append_popup(
                    popup_options(
                        "Coconut Control Center",
                        control_center::WIDTH,
                        control_center::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *control_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        network_revision.get();
                        bluetooth_revision.get();
                        battery_revision.get();
                        power_profile_revision.get();
                        control_center::build_with_integrations(
                            size,
                            network.clone(),
                            brightness.clone(),
                            battery.clone(),
                            volume.clone(),
                            bluetooth.clone(),
                            power_profile.clone(),
                            toggle.clone(),
                            awake.get(),
                            brightness_level.clone(),
                            volume_level.clone(),
                            wifi_enabled.clone(),
                            bluetooth_powered.clone(),
                            network_scroll.clone(),
                            bluetooth_scroll.clone(),
                            network_detail.clone(),
                            bluetooth_detail.clone(),
                            network_password.clone(),
                            &control_tray_config.tray,
                            view.clone(),
                        )
                    },
                );
            });

            let network_app = app.clone();
            let network_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let network_bar = bar_window.clone();
            let open_network = Rc::new(move |anchor: Point| {
                if let Some(handle) = network_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let network_handle = network_handle.clone();
                let network = panel_network.clone();
                let wifi_enabled = network_wifi_enabled.clone();
                let revision = network_panel_revision.clone();
                let scroll = ScrollController::new(0.0);
                let detail = Signal::new(None);
                let password = Signal::new(None);
                wifi_enabled.set(network.enabled());
                let bar_for_popup = network_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                network_app.append_popup(
                    popup_options(
                        "Coconut Network",
                        network_panel::WIDTH,
                        network_panel::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *network_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        revision.get();
                        network_panel::build(
                            size,
                            network.clone(),
                            wifi_enabled.clone(),
                            scroll.clone(),
                            detail.clone(),
                            password.clone(),
                            None,
                        )
                    },
                );
            });

            let bluetooth_app = app.clone();
            let bluetooth_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let bluetooth_bar = bar_window.clone();
            let open_bluetooth = Rc::new(move |anchor: Point| {
                if let Some(handle) = bluetooth_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let bluetooth_handle = bluetooth_handle.clone();
                let bluetooth = panel_bluetooth.clone();
                let bluetooth_powered = bluetooth_panel_powered.clone();
                let revision = bluetooth_panel_revision.clone();
                let scroll = ScrollController::new(0.0);
                let detail = Signal::new(None);
                bluetooth_powered.set(bluetooth.powered());
                let bar_for_popup = bluetooth_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                bluetooth_app.append_popup(
                    popup_options(
                        "Coconut Bluetooth",
                        bluetooth_panel::WIDTH,
                        bluetooth_panel::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *bluetooth_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        revision.get();
                        bluetooth_panel::build(
                            size,
                            bluetooth.clone(),
                            bluetooth_powered.clone(),
                            scroll.clone(),
                            detail.clone(),
                            None,
                        )
                    },
                );
            });

            let energy_app = app.clone();
            let energy_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let energy_bar = bar_window.clone();
            let open_energy = Rc::new(move |anchor: Point| {
                if let Some(handle) = energy_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let energy_handle = energy_handle.clone();
                let battery = panel_battery.clone();
                let power_profile = panel_power_profile.clone();
                let toggle_awake = energy_toggle_awake.clone();
                let awake = energy_awake.clone();
                let revision = energy_panel_revision.clone();
                let power_profile_revision = energy_panel_power_profile_revision.clone();
                let bar_for_popup = energy_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                energy_app.append_popup(
                    popup_options(
                        "Coconut Energy",
                        energy_panel::WIDTH,
                        energy_panel::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *energy_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        revision.get();
                        power_profile_revision.get();
                        energy_panel::build(
                            size,
                            battery.clone(),
                            power_profile.clone(),
                            awake.get(),
                            toggle_awake.clone(),
                            None,
                        )
                    },
                );
            });

            let brightness_app = app.clone();
            let brightness_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let brightness_bar = bar_window.clone();
            let open_brightness = Rc::new(move |anchor: Point| {
                if let Some(handle) = brightness_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let brightness_handle = brightness_handle.clone();
                let brightness = panel_brightness.clone();
                let brightness_level = energy_brightness_level.clone();
                brightness_level.set(brightness.level());
                let bar_for_popup = brightness_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                brightness_app.append_popup(
                    popup_options(
                        "Coconut Brightness",
                        brightness_panel::WIDTH,
                        brightness_panel::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *brightness_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        brightness_panel::build(
                            size,
                            brightness.clone(),
                            brightness_level.clone(),
                            None,
                        )
                    },
                );
            });

            let volume_app = app.clone();
            let volume_handle: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
            let volume_bar = bar_window.clone();
            let open_volume = Rc::new(move |anchor: Point| {
                if let Some(handle) = volume_handle.borrow_mut().take() {
                    if handle.is_open() {
                        handle.close();
                        return;
                    }
                }
                let volume_handle = volume_handle.clone();
                let volume = panel_volume.clone();
                let volume_level = panel_volume_level.clone();
                volume_level.set(volume.level());
                let bar_for_popup = volume_bar.clone();
                let Some(popup) = popup_for(&bar_for_popup, anchor, popup_opens_below) else {
                    return;
                };
                volume_app.append_popup(
                    popup_options(
                        "Coconut Volume",
                        volume_panel::WIDTH,
                        volume_panel::HEIGHT,
                    ),
                    popup,
                    Color::rgba(0, 0, 0, 0),
                    move |window| {
                        *volume_handle.borrow_mut() = Some(window.clone());
                        window.set_always_on_top(true);
                        close_on_focus_lost(&window);
                    },
                    move |size| {
                        volume_panel::build(size, volume.clone(), volume_level.clone(), None)
                    },
                );
            });

            app.append_window(
                WindowOptions {
                    title: "Coconut".into(),
                    width: 800,
                    height: DOCK_HEIGHT,
                    decorations: false,
                    resizable: false,
                    transparent: true,
                    role: bar_role,
                    theme: CreamTheme::theme(),
                    ..Default::default()
                },
                Color::rgba(0, 0, 0, 0),
                {
                    let bar_window = bar_window.clone();
                    move |window: WindowHandle| {
                        ready_backend.prepare_window(&window);
                        *bar_window.borrow_mut() = Some(window);
                    }
                },
                {
                    let config = config.clone();
                    let network_revision = bar_network_revision.clone();
                    let bluetooth_revision = bar_bluetooth_revision.clone();
                    let battery_revision = bar_battery_revision.clone();
                    move |viewport: Size| -> BoxedWidget {
                        network_revision.get();
                        bluetooth_revision.get();
                        battery_revision.get();
                        let status = SystemStatus {
                            network_connected: status_network.connected(),
                            network_strength: status_network.strength(),
                            battery_percentage: status_battery.percentage(),
                            battery_charging: status_battery.charging(),
                            bluetooth_powered: status_bluetooth.powered(),
                            bluetooth_connected: status_bluetooth.connected(),
                            volume: dock_volume.get(),
                            volume_muted: status_volume.muted(),
                        };
                        build_dock(
                            viewport,
                            windows.get(),
                            launcher_open.clone(),
                            status,
                            clock_text.clone(),
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
                                previous_playback: {
                                    let backend = bar_playback_backend.clone();
                                    Rc::new(move || backend.previous())
                                },
                                next_playback: {
                                    let backend = bar_playback_backend.clone();
                                    Rc::new(move || backend.next())
                                },
                                open_weather: open_weather.clone(),
                                open_clock: open_clock.clone(),
                                open_current_playing: open_current_playing.clone(),
                                open_app_drawer: open_app_drawer.clone(),
                                open_control_center: open_control_center.clone(),
                                open_network: open_network.clone(),
                                open_bluetooth: open_bluetooth.clone(),
                                open_energy: open_energy.clone(),
                                open_brightness: open_brightness.clone(),
                                open_volume: open_volume.clone(),
                            },
                            &config,
                        )
                    }
                },
            );
        })
        .run();
}

fn schedule_window_events(
    app: AppHandle,
    backend: Rc<dyn coconut_api::desktop::DesktopIntegration>,
    windows: Signal<Vec<coconut_api::desktop::OpenWindow>>,
) -> bool {
    let Some(listener) = backend.window_changes() else {
        return false;
    };
    wait_for_window_event(app, backend, windows, listener);
    true
}

fn wait_for_window_event(
    app: AppHandle,
    backend: Rc<dyn coconut_api::desktop::DesktopIntegration>,
    windows: Signal<Vec<coconut_api::desktop::OpenWindow>>,
    listener: coconut_api::desktop::WindowChangeListener,
) {
    let waiting_listener = listener.clone();
    let next_app = app.clone();
    app.spawn_background(
        move || waiting_listener.wait(),
        move |changed| {
            if changed {
                windows.set(backend.windows());
                wait_for_window_event(next_app, backend, windows, listener);
            }
        },
    );
}

fn schedule_playback_refresh(
    app: AppHandle,
    backend: Rc<dyn AudioIntegration>,
    state: Signal<Option<Playback>>,
) {
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
fn schedule_change_events(
    app: AppHandle,
    listener: Option<coconut_api::ChangeListener>,
    revision: Signal<()>,
) {
    match listener {
        Some(listener) => wait_for_change_event(app, listener, revision),
        None => schedule_poll_refresh(app, revision),
    }
}

fn wait_for_change_event(
    app: AppHandle,
    listener: coconut_api::ChangeListener,
    revision: Signal<()>,
) {
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

/// Wakes as soon as brightness's native hook (an inotify watch on the
/// kernel backlight's sysfs node) observes a change; falls back to a
/// couple-second poll for backends with no such hook (DDC-controlled
/// external monitors, which have no push notifications to watch).
/// Unlike [`schedule_poll_refresh`], both write the fresh value straight
/// into the shared signal every consumer (bar and panels alike) already
/// reads, rather than just poking a separate rebuild pulse.
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

/// Wakes as soon as volume's native hook (a libpipewire registry/node param
/// subscription) observes a change; falls back to a couple-second poll if
/// the backend exposes no such hook. Writes the fresh value straight into
/// the shared signal every consumer (bar and panels alike) already reads,
/// rather than just poking a separate rebuild pulse.
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

fn schedule_volume_poll(
    app: AppHandle,
    backend: Rc<dyn coconut_api::volume::VolumeIntegration>,
    level: Signal<f32>,
) {
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

fn close_on_focus_lost(window: &WindowHandle) {
    let handle = window.clone();
    window.on_focus_lost(move || handle.close());
}

fn popup_for(
    bar: &Rc<RefCell<Option<WindowHandle>>>,
    anchor: Point,
    opens_below: bool,
) -> Option<PopupOptions> {
    let bar_ref = bar.borrow();
    bar_ref.as_ref().map(|bar| {
        let anchor_rect = if opens_below {
            Rect {
                x: anchor.x,
                y: anchor.y,
                width: 1.0,
                height: (DOCK_HEIGHT as f32 - anchor.y).max(1.0),
            }
        } else {
            Rect {
                x: anchor.x,
                y: 0.0,
                width: 1.0,
                height: (anchor.y + 1.0).max(1.0),
            }
        };
        let popup = PopupOptions::new(bar.clone(), anchor_rect);
        if opens_below {
            popup.below()
        } else {
            popup.above()
        }
    })
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
