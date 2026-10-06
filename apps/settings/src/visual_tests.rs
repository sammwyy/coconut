//! Render the real native tree without a display server. The fixture mirrors
//! the reference's connection state, so layout checks don't depend on hardware.
use super::*;
use coconut_api::bluetooth::{BluetoothDevice, BluetoothIntegration};
use coconut_api::network::{NetworkIntegration, WifiNetwork};

struct Network;
impl NetworkIntegration for Network {
    fn connected(&self) -> bool {
        true
    }
    fn network_name(&self) -> Option<String> {
        Some("Movistar WiFi".into())
    }
    fn strength(&self) -> Option<u8> {
        Some(100)
    }
    fn enabled(&self) -> bool {
        true
    }
    fn set_enabled(&self, _: bool) {}
    fn wifi_networks(&self) -> Vec<WifiNetwork> {
        vec![WifiNetwork {
            ssid: "Movistar WiFi".into(),
            strength: 100,
            secured: true,
            active: true,
            band: Some("5 GHz"),
            security: "WPA3".into(),
        }]
    }
    fn saved_wifi_networks(&self) -> Vec<String> {
        vec![
            "Movistar WiFi".into(),
            "Home".into(),
            "Office".into(),
            "Guest".into(),
        ]
    }
}

struct Bluetooth;
impl BluetoothIntegration for Bluetooth {
    fn powered(&self) -> bool {
        true
    }
    fn connected(&self) -> bool {
        true
    }
    fn device_name(&self) -> Option<String> {
        Some("AirPods".into())
    }
    fn set_powered(&self, _: bool) {}
    fn devices(&self) -> Vec<BluetoothDevice> {
        vec![BluetoothDevice {
            address: "00:00:00:00:00:01".into(),
            name: "AirPods".into(),
            paired: true,
            connected: true,
            trusted: true,
            battery_percent: Some(82),
            icon_hint: Some("audio-headset".into()),
            class: None,
        }]
    }
}

#[test]
fn connectivity_layout_matches_reference() {
    use creamui_core::{Point, Renderer};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Color, Theme, ThemeProvider};

    creamui_reactive::with_context_scope(|| {
        common::load_settings_fonts();
        let theme = common::settings_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let moves = Rc::new(Cell::new(0));
        creamui_reactive::provide_context(creamui_core::WindowDragHandle::new({
            let moves = moves.clone();
            move || moves.set(moves.get() + 1)
        }));
        let decorations = Signal::new(creamui_render::WindowDecorations {
            mode: creamui_render::WindowDecorationMode::Hybrid,
            controls: creamui_render::CompositorControls {
                x: 992,
                y: 14,
                width: 112,
                height: 28,
            },
        });
        creamui_reactive::provide_context(decorations.clone());
        let integrations = coconut_api::Registry {
            desktop: Rc::new(coconut_api::desktop::Fallback),
            audio: Rc::new(coconut_api::audio::Fallback),
            brightness: Rc::new(coconut_api::brightness::Fallback),
            battery: Rc::new(coconut_api::battery::Fallback),
            volume: Rc::new(coconut_api::volume::Fallback),
            network: Rc::new(Network),
            bluetooth: Rc::new(Bluetooth),
            power_profile: Rc::new(coconut_api::power_profile::Fallback),
            settings: std::sync::Arc::new(coconut_api::settings::Fallback),
        };
        let accounts = Vec::new();
        let profile = users::ProfileControllers::load(&accounts);
        let icons = SettingsIcons::load();
        let connectivity = sections::connectivity::State::new();
        let window_state = sections::window::WindowState::load();
        let shortcuts = sections::window::ShortcutState::load();
        let appearance = Signal::new(creamui_theme::ResolvedAppearance {
            theme_id: "test".into(),
            variant_id: "light".into(),
            accent: theme.colors.accent,
            theme,
            font_family: None,
            corners: Default::default(),
        });
        let content_scroll = ScrollController::new(0.0);
        let sidebar_scroll = ScrollController::new(0.0);
        let view = Signal::new(Section::Connectivity);
        let config = Signal::new(ShellConfig::default());
        let nav = SidebarNavController::new();
        let window = Rc::new(RefCell::new(None));
        let picker = ColorPickerController::new();
        let custom_accent = Signal::new(false);
        let users = Signal::new(accounts);
        let gallery = sections::wallpaper::GalleryState::new();
        let dock_scroll = ScrollController::new(0.0);
        let dock_tab = Signal::new(sections::general::DockTab::Display);
        let islands = Signal::new(None);
        let search = TextController::new("");
        let maximized = Signal::new(false);
        let native = sections::native::State::new(integrations.settings.clone());

        let size = Size {
            width: 1120.0,
            height: 760.0,
        };
        let root = |size| {
            build(
                size,
                &config,
                &integrations,
                &appearance,
                &view,
                &nav,
                &window,
                &picker,
                &picker,
                &picker,
                &custom_accent,
                &users,
                &profile,
                &icons,
                &gallery,
                &window_state,
                &shortcuts,
                &content_scroll,
                &sidebar_scroll,
                &dock_scroll,
                &dock_tab,
                &islands,
                &search,
                &connectivity,
                &maximized,
                &native,
            )
        };
        let mut recorder = SceneRecorder::new();
        recorder.begin(1120, 760, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
        let scene = Renderer::new().render(root(size), size, &mut recorder);
        let list = recorder.finish();
        assert!(
            scene.hit_test(Point { x: 600.0, y: 28.0 }).is_none(),
            "empty header is not an input-grabbing button"
        );
        scene
            .window_drag_at(Point { x: 600.0, y: 28.0 })
            .expect("app-selected empty header moves window")();
        assert_eq!(moves.get(), 1);
        assert!(
            scene.window_drag_at(Point { x: 100.0, y: 92.0 }).is_none(),
            "search is not a window drag region"
        );
        assert!(
            scene.window_drag_at(Point { x: 700.0, y: 190.0 }).is_none(),
            "card content is not a window drag region"
        );
        assert!(
            scene.hit_test(Point { x: 1090.0, y: 28.0 }).is_none(),
            "hybrid controls belong to the compositor, not the client scene"
        );
        for mode in [
            creamui_render::WindowDecorationMode::Pending,
            creamui_render::WindowDecorationMode::Server,
            creamui_render::WindowDecorationMode::None,
            creamui_render::WindowDecorationMode::Client,
        ] {
            decorations.set(creamui_render::WindowDecorations {
                mode,
                ..Default::default()
            });
            let mut mode_recorder = SceneRecorder::new();
            mode_recorder.begin(1120, 760, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
            let mode_scene = Renderer::new().render(root(size), size, &mut mode_recorder);
            assert_eq!(
                mode_scene.hit_test(Point { x: 1090.0, y: 28.0 }).is_some(),
                mode == creamui_render::WindowDecorationMode::Client,
                "only negotiated Client mode draws fallback buttons: {mode:?}"
            );
        }
        decorations.set(creamui_render::WindowDecorations {
            mode: creamui_render::WindowDecorationMode::Hybrid,
            controls: creamui_render::CompositorControls {
                x: 992,
                y: 14,
                width: 112,
                height: 28,
            },
        });
        let mut raster = Rasterizer::new(1120, 760);
        raster.render(&list, &Damage::Full);
        if let Ok(path) = std::env::var("COCONUT_SETTINGS_SNAPSHOT") {
            raster
                .pixmap()
                .save_png(path)
                .expect("save native Settings preview");
        }
        // The category header precedes the cards; its extra height makes the
        // document scrollable while the complete sidebar still fits.
        assert!(
            scene.hit_test(Point { x: 80.0, y: 170.0 }).is_some(),
            "sidebar selection"
        );
        assert!(
            scene.hit_test(Point { x: 700.0, y: 190.0 }).is_some(),
            "Wi-Fi card"
        );
        scene
            .hit_test(Point { x: 700.0, y: 350.0 })
            .expect("known networks row")();
        assert!(matches!(
            connectivity.view.peek(),
            sections::connectivity::View::KnownNetworks
        ));
        assert!(
            content_scroll.max_offset() > 0.0,
            "header and cards can scroll at reference size: {}",
            content_scroll.max_offset()
        );
        assert!(
            sidebar_scroll.max_offset() <= 1.0,
            "all categories fit at reference size"
        );
        // The client paints every corner. Clipping belongs to the compositor,
        // which must apply the same shape to the window, blur and opacity.
        for (x, y) in [(0, 0), (1119, 0), (0, 759), (1119, 759)] {
            assert!(raster.pixmap().pixel(x, y).unwrap().alpha() > 0);
        }

        recorder.begin(1120, 760, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
        let detail = Renderer::new().render(root(size), size, &mut recorder);
        let list = recorder.finish();
        raster.render(&list, &Damage::Full);
        if let Ok(path) = std::env::var("COCONUT_SETTINGS_DETAIL_SNAPSHOT") {
            raster
                .pixmap()
                .save_png(path)
                .expect("save native subpage preview");
        }
        // The compact header's back button must not be swallowed by the
        // transparent window drag area laid over the top of the window.
        assert!(
            detail.window_drag_at(Point { x: 306.0, y: 28.0 }).is_none(),
            "back button automatically takes priority over the drag region"
        );
        assert!(
            detail.window_drag_at(Point { x: 430.0, y: 28.0 }).is_none(),
            "header text is content, not empty background, without needing a click handler"
        );
        detail
            .window_drag_at(Point { x: 600.0, y: 28.0 })
            .expect("subpage empty header moves window")();
        assert_eq!(moves.get(), 2);
        detail
            .hit_test(Point { x: 306.0, y: 28.0 })
            .expect("subpage back button")();
        assert!(matches!(
            connectivity.view.peek(),
            sections::connectivity::View::Overview
        ));
        assert!(
            content_scroll.max_offset() <= 1.0,
            "compact detail fits without category header"
        );

        let small = Size {
            width: 900.0,
            height: 600.0,
        };
        recorder.begin(900, 600, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
        let scene = Renderer::new().render(root(small), small, &mut recorder);
        assert!(
            content_scroll.max_offset() > 0.0,
            "short windows can scroll the content"
        );
        assert!(
            sidebar_scroll.max_offset() > 0.0,
            "short windows can scroll navigation"
        );
        assert!(
            scene.hit_test(Point { x: 500.0, y: 190.0 }).is_some(),
            "content remains usable in a narrow window"
        );

        // Exercise every category and native drill-down using fixture data.
        // Rendering cannot apply changes to the host or invoke privileged APIs.
        use coconut_api::settings::{Choice, Entry, Preference, Snapshot, Value};
        let mut fixture = Snapshot {
            loaded: true,
            session: "Test compositor".into(),
            ..Default::default()
        };
        for (key, text) in [
            ("hostname", "test-device"),
            ("os", "Test Linux"),
            ("kernel", "6.0"),
            ("processor", "Test processor"),
            ("memory", "16 GiB"),
            ("graphics", "Test GPU"),
            ("cores", "8"),
            ("storage", "42 / 128 GiB used"),
            ("printer-service", "CUPS"),
            ("camera-service", "Desktop portal"),
            ("microphone-service", "Desktop portal"),
        ] {
            fixture.facts.insert(key.into(), text.into());
        }
        for key in ["brightness", "volume", "text-scale"] {
            fixture.preferences.insert(
                key.into(),
                Preference {
                    value: Some(Value::Number(if key == "text-scale" { 1.0 } else { 0.65 })),
                    writable: true,
                    ..Default::default()
                },
            );
        }
        for key in [
            "ntp",
            "idle-dim",
            "notifications",
            "location",
            "contrast",
            "zoom",
            "reduce-motion",
            "visual-alerts",
            "sticky-keys",
            "lock-after-sleep",
            "microphone-muted",
        ] {
            fixture.preferences.insert(
                key.into(),
                Preference {
                    value: Some(Value::Bool(false)),
                    writable: true,
                    ..Default::default()
                },
            );
        }
        for key in [
            "browser",
            "power-mode",
            "locale",
            "sound-output",
            "sound-input",
        ] {
            fixture.preferences.insert(
                key.into(),
                Preference {
                    value: Some(Value::Text("one".into())),
                    writable: true,
                    choices: vec![
                        Choice {
                            id: "one".into(),
                            label: "First option".into(),
                        },
                        Choice {
                            id: "two".into(),
                            label: "Second option".into(),
                        },
                    ],
                    ..Default::default()
                },
            );
        }
        for key in [
            "displays",
            "devices",
            "input-devices",
            "printers",
            "applications",
            "startup",
            "fonts",
            "storage",
            "activity",
            "camera-permissions",
            "microphone-permissions",
        ] {
            fixture.collections.insert(
                key.into(),
                vec![Entry {
                    id: "test.desktop".into(),
                    name: "Test device or application".into(),
                    description: "Real backend fixture for layout testing".into(),
                    enabled: true,
                    ..Default::default()
                }],
            );
        }
        native.snapshot.set(fixture);
        let roots = [
            Section::Connectivity,
            Section::Hardware,
            Section::Personalization,
            Section::Desktop,
            Section::Windows,
            Section::ShortcutsCategory,
            Section::Applications,
            Section::Users,
            Section::Privacy,
            Section::Accessibility,
            Section::System,
            Section::About,
        ];
        let details = [
            sections::native::Page::Displays,
            sections::native::Page::Devices,
            sections::native::Page::Printers,
            sections::native::Page::AddPrinter,
            sections::native::Page::Fonts,
            sections::native::Page::InstalledApps,
            sections::native::Page::App("test.desktop".into()),
            sections::native::Page::FileTypes,
            sections::native::Page::Startup,
            sections::native::Page::AddStartup,
            sections::native::Page::Password,
            sections::native::Page::AddUser,
            sections::native::Page::OnlineAccounts,
            sections::native::Page::Fingerprint,
            sections::native::Page::Camera,
            sections::native::Page::Microphone,
            sections::native::Page::Notifications,
            sections::native::Page::HotCorners,
            sections::native::Page::WindowRules,
            sections::native::Page::Updates,
            sections::native::Page::Storage,
            sections::native::Page::Activity,
            sections::native::Page::DateTime,
            sections::native::Page::Language,
            sections::native::Page::Reset,
        ];
        for (name, base) in [
            ("light", Theme::light()),
            ("dark", Theme::dark()),
            ("midnight", Theme::midnight()),
        ] {
            let theme = common::settings_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            appearance.set(creamui_theme::ResolvedAppearance {
                theme_id: "default".into(),
                variant_id: name.into(),
                accent: theme.colors.accent,
                theme,
                font_family: None,
                corners: Default::default(),
            });
            for section in roots
                .iter()
                .cloned()
                .chain(details.iter().cloned().map(Section::Detail))
                .chain([
                    Section::IconPack,
                    Section::CursorTheme,
                    Section::Sound,
                    Section::Wallpaper,
                    Section::DesktopIcons,
                    Section::Focus,
                    Section::Compositor,
                    Section::Mouse,
                    Section::Keyboard,
                    Section::Touchpad,
                    Section::Statusbar,
                    Section::Dockbar,
                    Section::Tray,
                    Section::Islands,
                    Section::Layout,
                    Section::Titlebar,
                    Section::WorkingArea,
                    Section::Effects,
                    Section::Profile,
                ])
            {
                view.set(section.clone());
                content_scroll.set(0.0);
                recorder.begin(1120, 760, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
                let scene = Renderer::new().render(root(size), size, &mut recorder);
                let list = recorder.finish();
                raster.render(&list, &Damage::Full);
                let title = page_title(&section);
                assert!(list.items.len() > 50, "empty native view: {title}");
                for (x, y) in [(0, 0), (1119, 0), (0, 759), (1119, 759)] {
                    assert!(
                        raster.pixmap().pixel(x, y).unwrap().alpha() > 0,
                        "client clips {title}"
                    );
                }
                if sections::native::parent(&section) != section {
                    assert!(
                        scene.window_drag_at(Point { x: 306.0, y: 28.0 }).is_none(),
                        "back button swallowed by drag: {title}"
                    );
                    scene
                        .hit_test(Point { x: 306.0, y: 28.0 })
                        .expect("compact back button")();
                    assert!(
                        view.peek() == sections::native::parent(&section),
                        "incorrect parent for {title}"
                    );
                }
                if let Ok(directory) = std::env::var("COCONUT_SETTINGS_ALL_SNAPSHOTS") {
                    let filename = title.to_lowercase().replace([' ', '&', '/'], "_");
                    raster
                        .pixmap()
                        .save_png(
                            std::path::Path::new(&directory).join(format!("{name}-{filename}.png")),
                        )
                        .expect("save native view");
                }
            }
        }
    });
}
