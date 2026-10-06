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
                    assert_eq!(opened.get(), Some(island.id()));
                }
            }
        }
    });
}
