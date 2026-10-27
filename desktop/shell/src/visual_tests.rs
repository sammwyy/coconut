//! Render the actual plugins without connecting to a compositor or services.
use coconut_plugin_kit::{
    design, Island, IslandConfig, IslandRenderContext, Panel, PanelRenderContext, SharedState,
};
use coconut_plugin_tray::{
    BluetoothPowered, BrightnessLevel, ControlCenterPanel, TrayConfig, VolumeLevel, WifiEnabled,
};
use creamui_core::{Point, Renderer, Size};
use creamui_reactive::Signal;
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};
use std::cell::Cell;
use std::rc::Rc;

fn shared_fixture() -> SharedState {
    let shared = SharedState::default();
    shared.insert(WifiEnabled(Signal::new(true)));
    shared.insert(BluetoothPowered(Signal::new(true)));
    shared.insert(BrightnessLevel(Signal::new(0.65)));
    shared.insert(VolumeLevel(Signal::new(0.42)));
    shared.insert(coconut_plugin_app_drawer::AppCatalog::new());
    shared.insert(coconut_plugin_current_playing::CurrentPlayback(Rc::new(
        || {
            Some(coconut_api::audio::Playback {
                app_icon: None,
                art_url: None,
                title: "Borderline".into(),
                artist: "Tame Impala".into(),
                status: "Playing".into(),
                position: "1:32".into(),
                length: "3:57".into(),
            })
        },
    )));
    shared.insert(Signal::new(coconut_plugin_weather::WeatherState::Ready(
        coconut_plugin_weather::Weather {
            city: "Buenos Aires".into(),
            location_codes: "AR · CABA".into(),
            temperature_c: 19.0,
            condition: "clearsky_day".into(),
            high_c: 23.0,
            low_c: 15.0,
            hourly: (6..10)
                .map(|hour| coconut_plugin_weather::HourlyForecast {
                    time: format!("2026-10-06T{hour:02}:00:00-03:00"),
                    temperature_c: 19.0,
                    condition: "clearsky_day".into(),
                })
                .collect(),
        },
    )));
    shared
}

#[test]
fn every_popup_renders_in_both_variants_without_client_corner_clipping() {
    for (variant, base) in [
        ("light", Theme::light()),
        ("dark", Theme::dark()),
        ("midnight", Theme::midnight()),
    ] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let shared = shared_fixture();
            let panels: Vec<Box<dyn Panel>> = vec![
                Box::new(ControlCenterPanel::new(TrayConfig::default())),
                Box::new(coconut_plugin_tray::NetworkPanel),
                Box::new(coconut_plugin_tray::BluetoothPanel),
                Box::new(coconut_plugin_tray::BrightnessPanel),
                Box::new(coconut_plugin_tray::VolumePanel),
                Box::new(coconut_plugin_tray::EnergyPanel),
                Box::new(coconut_plugin_clock::ClockPanel {
                    format: "%H:%M".into(),
                }),
                Box::new(coconut_plugin_weather::WeatherPanel),
                Box::new(coconut_plugin_current_playing::CurrentPlayingPanel),
                Box::new(coconut_plugin_app_drawer::AppDrawerPanel),
            ];
            for panel in panels {
                let size = panel.size();
                let ctx = PanelRenderContext {
                    size,
                    shared: shared.clone(),
                    scratch: SharedState::default(),
                    open_panel: Rc::new(|_, _| {}),
                    close: Rc::new(|| {}),
                };
                let mut recorder = SceneRecorder::new();
                recorder.begin(
                    size.width as u32,
                    size.height as u32,
                    1.0,
                    Color::rgba(0, 0, 0, 0),
                    theme.colors,
                );
                let scene = Renderer::new().render(panel.build(&ctx), size, &mut recorder);
                let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                let list = recorder.finish();
                raster.render(&list, &Damage::Full);
                for (x, y) in [
                    (0, 0),
                    (size.width as u32 - 1, 0),
                    (0, size.height as u32 - 1),
                    (size.width as u32 - 1, size.height as u32 - 1),
                ] {
                    assert!(
                        raster.pixmap().pixel(x, y).unwrap().alpha() > 0,
                        "{}: compositor owns the corners",
                        panel.id()
                    );
                }
                if panel.id() == "control_center" {
                    let wifi = shared.get::<WifiEnabled>().unwrap().0;
                    scene
                        .hit_test(Point { x: 155.0, y: 78.0 })
                        .expect("Wi-Fi switch remains clickable")();
                    assert!(
                        !wifi.peek(),
                        "the child switch must win over the navigation tile"
                    );
                }
                if let Ok(prefix) = std::env::var("COCONUT_SHELL_SNAPSHOT") {
                    raster
                        .pixmap()
                        .save_png(format!("{prefix}-{variant}-{}.png", panel.id()))
                        .unwrap();
                }
            }
        });
    }
}

#[test]
fn islands_keep_their_dispatch_and_scale_in_horizontal_and_vertical_bars() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let shared = shared_fixture();
        let config = IslandConfig::default();
        let opened = Rc::new(Cell::new(None));
        let islands: Vec<Box<dyn Island>> = vec![
            Box::new(coconut_plugin_clock::ClockIsland {
                format: "%H:%M".into(),
            }),
            Box::new(coconut_plugin_weather::WeatherIsland),
            Box::new(coconut_plugin_current_playing::CurrentPlayingIsland),
            Box::new(coconut_plugin_tray::ControlCenterIsland::new(
                TrayConfig::default(),
            )),
        ];
        for position in [
            coconut_core::DockPosition::Top,
            coconut_core::DockPosition::Left,
        ] {
            for scale in [0.8, 1.0, 1.5] {
                for island in &islands {
                    let ctx = IslandRenderContext {
                        shared: &shared,
                        config: &config,
                        position,
                        scale,
                        open_panel: {
                            let opened = opened.clone();
                            Rc::new(move |id, _| opened.set(Some(id)))
                        },
                    };
                    let size = Size {
                        width: 400.0,
                        height: 64.0,
                    };
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(400, 64, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
                    let scene = Renderer::new().render(island.build(&ctx), size, &mut recorder);
                    // Empty leading padding opens its panel, not a media transport control.
                    let point = Point {
                        x: 5.0 * scale,
                        y: 16.0 * scale,
                    };
                    scene
                        .hit_test_at(point)
                        .expect("island still dispatches its popup")(point);
                    let expected = if island.id() == "control_center" {
                        "energy"
                    } else {
                        island.id()
                    };
                    assert_eq!(opened.get(), Some(expected));
                    if island.id() == "control_center" {
                        let point = Point {
                            x: 49.0 * scale,
                            y: 16.0 * scale,
                        };
                        scene
                            .hit_test_at(point)
                            .expect("system controls remain clickable")(
                            point
                        );
                        assert_eq!(opened.get(), Some("control_center"));
                    }
                }
            }
        }
    });
}

struct SnapshotBattery;

#[test]
fn compact_window_icons_keep_every_open_window_accessible() {
    use coconut_plugin_app_launcher::{OpenWindowsIsland, WindowListState};
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(
            design::coconut_theme(Theme::light()),
        ));
        let shared = SharedState::default();
        let activated = Rc::new(std::cell::RefCell::new(Vec::new()));
        shared.insert(WindowListState {
            windows: Signal::new(
                (0..2)
                    .map(|index| coconut_api::desktop::OpenWindow {
                        id: index.to_string(),
                        app_name: String::new(),
                        title: "Coconut Settings".into(),
                        icon_path: None,
                        active: index == 1,
                    })
                    .collect(),
            ),
            activate: {
                let activated = activated.clone();
                Rc::new(move |id| activated.borrow_mut().push(id))
            },
            refresh: Rc::new(|| {}),
        });
        let config = IslandConfig::default();
        let ctx = IslandRenderContext {
            shared: &shared,
            config: &config,
            position: coconut_core::DockPosition::Top,
            scale: 1.0,
            open_panel: Rc::new(|_, _| {}),
        };
        let mut recorder = SceneRecorder::new();
        recorder.begin(120, 44, 1.0, Color::rgba(0, 0, 0, 0), Theme::light().colors);
        let scene = Renderer::new().render(
            OpenWindowsIsland.build(&ctx),
            Size {
                width: 120.0,
                height: 44.0,
            },
            &mut recorder,
        );
        for x in [23.0, 75.0] {
            scene
                .hit_test(Point { x, y: 16.0 })
                .expect("each compact application is clickable")();
        }
        assert_eq!(&*activated.borrow(), &["0", "1"]);
    });
}

impl coconut_api::battery::BatteryIntegration for SnapshotBattery {
    fn percentage(&self) -> Option<u8> {
        Some(82)
    }
    fn charging(&self) -> bool {
        false
    }
}

#[test]
fn full_bars_render_and_keep_window_and_media_actions() {
    use coconut_api::desktop::OpenWindow;
    use coconut_core::ShellConfig;
    use coconut_plugin_app_launcher::{AppLauncherIsland, OpenWindowsIsland, WindowListState};
    use coconut_plugin_current_playing::{NextPlayback, PreviousPlayback, TogglePlayback};
    use creamui_core::layout::{Position, Style as LayoutStyle};
    use creamui_core::{BoxedWidget, Styled};
    use creamui_image::{Image, ImageData, ImageFit};
    use creamui_widgets::{layout::fixed, RawView};
    use std::cell::RefCell;

    for (variant, base) in [
        ("light", Theme::light()),
        ("dark", Theme::dark()),
        ("midnight", Theme::midnight()),
    ] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let shared = shared_fixture();
            shared.insert(
                Rc::new(SnapshotBattery) as Rc<dyn coconut_api::battery::BatteryIntegration>
            );
            shared.insert(coconut_plugin_clock::ClockText(Signal::new(
                "05:13 AM".into(),
            )));
            let activated = Rc::new(RefCell::new(String::new()));
            let refreshed = Rc::new(Cell::new(0));
            shared.insert(WindowListState {
                windows: Signal::new(
                    [
                        "Weather",
                        "Coconut Settings",
                        "Files",
                        "Music",
                        "Notes",
                        "Calendar",
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(index, title)| OpenWindow {
                        id: index.to_string(),
                        app_name: String::new(),
                        title: title.into(),
                        icon_path: None,
                        active: index == 2,
                    })
                    .collect(),
                ),
                activate: {
                    let activated = activated.clone();
                    Rc::new(move |id| *activated.borrow_mut() = id)
                },
                refresh: {
                    let refreshed = refreshed.clone();
                    Rc::new(move || refreshed.set(refreshed.get() + 1))
                },
            });
            let previous = Rc::new(Cell::new(0));
            let toggled = Rc::new(Cell::new(0));
            let next = Rc::new(Cell::new(0));
            shared.insert(PreviousPlayback({
                let count = previous.clone();
                Rc::new(move || count.set(count.get() + 1))
            }));
            shared.insert(TogglePlayback({
                let count = toggled.clone();
                Rc::new(move || count.set(count.get() + 1))
            }));
            shared.insert(NextPlayback({
                let count = next.clone();
                Rc::new(move || count.set(count.get() + 1))
            }));
            let islands: Vec<Rc<dyn Island>> = vec![
                Rc::new(coconut_plugin_logo::LogoIsland),
                Rc::new(AppLauncherIsland),
                Rc::new(OpenWindowsIsland),
                Rc::new(coconut_plugin_weather::WeatherIsland),
                Rc::new(coconut_plugin_clock::ClockIsland {
                    format: "%I:%M %p".into(),
                }),
                Rc::new(coconut_plugin_current_playing::CurrentPlayingIsland),
                Rc::new(coconut_plugin_tray::ControlCenterIsland::new(
                    TrayConfig::default(),
                )),
            ];
            let lookup = |id: &str| islands.iter().find(|island| island.id() == id).cloned();
            let opened = Rc::new(Cell::new(None));
            let open: Rc<dyn Fn(&'static str, Point)> = {
                let opened = opened.clone();
                Rc::new(move |id, _| opened.set(Some(id)))
            };
            let shell = ShellConfig::default();
            let window_state = shared.get::<WindowListState>().unwrap();
            let windows = window_state.windows.peek();
            for (label, mut dock, size) in [
                (
                    "statusbar",
                    shell.statusbar,
                    Size {
                        width: 1920.0,
                        height: 80.0,
                    },
                ),
                (
                    "dockbar",
                    shell.dockbar,
                    Size {
                        width: 800.0,
                        height: 124.0,
                    },
                ),
            ] {
                window_state.windows.set(if label == "statusbar" {
                    windows
                        .iter()
                        .filter(|window| window.active)
                        .cloned()
                        .collect()
                } else {
                    windows.clone()
                });
                for section in &mut dock.sections {
                    for entry in &mut section.islands {
                        if entry.id == "open_windows" {
                            entry.config.insert(
                                "compact".into(),
                                toml::Value::Boolean(label == "statusbar"),
                            );
                            entry
                                .config
                                .insert("show_background".into(), toml::Value::Boolean(false));
                        }
                    }
                }
                let offset = if label == "dockbar" { 16.0 } else { 0.0 };
                let viewport = Size {
                    width: size.width,
                    height: dock.thickness + dock.margin,
                };
                let widget =
                    super::bar::build_dock_with_lookup(viewport, &dock, &lookup, &shared, &open);
                let absolute = LayoutStyle {
                    position: Position::Absolute,
                    ..Default::default()
                };
                let wallpaper = ImageData::from_bytes(include_bytes!(
                    "../../../assets/wallpapers/dandelion.webp"
                ))
                .unwrap();
                let tree: BoxedWidget = Box::new(
                    RawView::new(LayoutStyle {
                        size: fixed(size.width, size.height),
                        ..Default::default()
                    })
                    .child(Box::new(
                        Image::new(wallpaper)
                            .layout(LayoutStyle {
                                size: fixed(size.width, size.height),
                                ..absolute.clone()
                            })
                            .fit(ImageFit::Cover),
                    ))
                    .child(Box::new(
                        RawView::new(LayoutStyle {
                            size: fixed(viewport.width, viewport.height),
                            inset: creamui_core::layout::Rect {
                                top: creamui_core::layout::LengthPercentageAuto::Length(offset),
                                ..LayoutStyle::default().inset
                            },
                            ..absolute
                        })
                        .child(widget),
                    )),
                );
                let mut recorder = SceneRecorder::new();
                recorder.begin(
                    size.width as u32,
                    size.height as u32,
                    1.0,
                    Color::rgba(0, 0, 0, 0),
                    theme.colors,
                );
                let scene = Renderer::new().render(tree, size, &mut recorder);
                let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                raster.render(&recorder.finish(), &Damage::Full);
                if let Ok(prefix) = std::env::var("COCONUT_SHELL_SNAPSHOT") {
                    raster
                        .pixmap()
                        .save_png(format!("{prefix}-{variant}-{label}.png"))
                        .unwrap();
                }
                if label == "statusbar" {
                    let active_window = Point { x: 176.0, y: 22.0 };
                    scene
                        .hit_test(active_window)
                        .expect("active window is clickable")();
                    assert_eq!(&*activated.borrow(), "2");
                    let weather = Point { x: 755.0, y: 22.0 };
                    scene
                        .hit_test_at(weather)
                        .expect("weather pill is clickable")(weather);
                    assert_eq!(opened.get(), Some("weather"));
                    for (point, count) in [
                        (Point { x: 1018.0, y: 22.0 }, previous.clone()),
                        (Point { x: 1044.0, y: 22.0 }, toggled.clone()),
                        (Point { x: 1070.0, y: 22.0 }, next.clone()),
                    ] {
                        scene
                            .hit_test_at(point)
                            .expect("transport remains clickable")(point);
                        assert_eq!(count.get(), 1);
                    }
                } else {
                    let drawer = Point { x: 200.0, y: 50.0 };
                    scene
                        .hit_test_at(drawer)
                        .expect("launcher remains clickable")(drawer);
                    assert_eq!(opened.get(), Some("app_drawer"));
                    for (index, x) in [288.0, 347.0, 406.0, 465.0, 524.0, 583.0]
                        .into_iter()
                        .enumerate()
                    {
                        let point = Point { x, y: 50.0 };
                        scene
                            .hit_test(point)
                            .expect("each dock application remains clickable")(
                        );
                        assert_eq!(&*activated.borrow(), &index.to_string());
                    }
                    assert_eq!(refreshed.get(), 7);
                }
            }
        });
    }
}
