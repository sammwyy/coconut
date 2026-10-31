//! Render the real native tree without a display server. The fixture mirrors
//! the reference's connection state, so layout checks don't depend on hardware.
use crate::icons::SettingsIcons;
use crate::routes::destination::{page_title, Section};
use crate::views::layout::build;
use crate::{components, routes};
use coconut_core::ShellConfig;
use creamui_core::Size;
use creamui_reactive::Signal;
use creamui_router::RouterProvider;
use creamui_widgets::{ColorPickerController, ScrollController, TextController};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

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

struct Preview {
    context: crate::views::context::ViewContext,
    decorations: Signal<creamui_render::WindowDecorations>,
    moves: Rc<Cell<i32>>,
}

impl Preview {
    fn new() -> Self {
        use creamui_theme::{Theme, ThemeProvider};
        components::load_settings_fonts();
        let theme = components::settings_theme(Theme::light());
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
            maximized: false,
            hovered: None,
        });
        creamui_reactive::provide_context(decorations.clone());
        let integrations = Rc::new(coconut_api::Registry {
            desktop: Rc::new(coconut_api::desktop::Fallback),
            audio: Rc::new(coconut_api::audio::Fallback),
            brightness: Rc::new(coconut_api::brightness::Fallback),
            battery: Rc::new(coconut_api::battery::Fallback),
            volume: Rc::new(coconut_api::volume::Fallback),
            network: Rc::new(Network),
            bluetooth: Rc::new(Bluetooth),
            power_profile: Rc::new(coconut_api::power_profile::Fallback),
            settings: std::sync::Arc::new(coconut_api::settings::Fallback),
            notifications: Rc::new(coconut_api::notifications::Fallback),
        });
        let accounts = Vec::new();
        let profile = crate::views::users::profile::ProfileControllers::load(&accounts);
        let icons = SettingsIcons::load();
        let view = routes::create(routes::DEFAULT_ROUTE).unwrap();
        let connectivity = crate::views::connectivity::State::new(view.clone());
        // Preview actual controls independently of a running Blair service.
        // Rendering this fixture never applies configuration to the host.
        let window_state = Rc::new(crate::views::windows::WindowState::preview());
        let shortcuts = Rc::new(crate::views::windows::ShortcutState::load());
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
        let config = Signal::new(ShellConfig::default());
        let background = Signal::new(coconut_core::BackgroundConfig::default());
        let window = Rc::new(RefCell::new(None));
        let picker = ColorPickerController::new();
        let custom_accent = Signal::new(false);
        let users = Signal::new(accounts);
        let gallery = crate::views::personalization::wallpaper::GalleryState::new();
        let dock_scroll = ScrollController::new(0.0);
        let island_settings = crate::views::desktop::island_settings::State::default();
        let bar_content = crate::views::desktop::bar_content::State::default();
        let search = TextController::new("");
        let maximized = Signal::new(false);
        let native = crate::services::settings::State::new(integrations.settings.clone());

        let context = crate::views::context::ViewContext {
            size: Size {
                width: 1120.0,
                height: 760.0,
            },
            config: config.clone(),
            background: background.clone(),
            integrations: integrations.clone(),
            appearance: appearance.clone(),
            view: view.clone(),
            window: window.clone(),
            wallpaper_pickers: crate::views::personalization::wallpaper::WallpaperPickers::new(),
            desktop_icons_color_picker: picker.clone(),
            appearance_accent_picker: picker.clone(),
            appearance_custom_accent: custom_accent.clone(),
            users: users.clone(),
            profile: profile.clone(),
            icons: icons.clone(),
            wallpaper_gallery: gallery.clone(),
            window_settings: window_state.clone(),
            shortcut_settings: shortcuts.clone(),
            content_scroll: content_scroll.clone(),
            sidebar_scroll: sidebar_scroll.clone(),
            dock_scroll: dock_scroll.clone(),
            bar_content: bar_content.clone(),
            island_settings: island_settings.clone(),
            settings_search: search.clone(),
            connectivity: connectivity.clone(),
            maximized: maximized.clone(),
            native: native.clone(),
        };
        Self {
            context,
            decorations,
            moves,
        }
    }

    fn render(&self, size: Size) -> creamui_core::BoxedWidget {
        RouterProvider::new(self.context.view.clone()).render(|| {
            let mut context = self.context.clone();
            context.size = size;
            build(&context)
        })
    }
}

#[test]
fn connectivity_layout_matches_reference() {
    use creamui_core::{Point, Renderer};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Color, Theme, ThemeProvider};

    creamui_reactive::with_context_scope(|| {
        let preview = Preview::new();
        let context = &preview.context;
        let theme = components::settings_theme(Theme::light());
        let decorations = &preview.decorations;
        let moves = &preview.moves;
        let appearance = &context.appearance;
        let content_scroll = &context.content_scroll;
        let sidebar_scroll = &context.sidebar_scroll;
        let native = &context.native;
        let connectivity = &context.connectivity;
        let view = &context.view;
        let size = Size {
            width: 1120.0,
            height: 760.0,
        };
        let root = |size| preview.render(size);
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
            maximized: false,
            hovered: None,
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
            connectivity.current(),
            crate::views::connectivity::View::KnownNetworks
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
            connectivity.current(),
            crate::views::connectivity::View::Overview
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
        fixture.preferences.insert(
            "hostname".into(),
            Preference {
                value: Some(Value::Text("test-device".into())),
                writable: true,
                ..Default::default()
            },
        );
        native.snapshot.set(fixture);
        routes::open(view, "/connectivity/network");
        content_scroll.set(0.0);
        recorder.begin(1120, 760, 1.0, Color::rgba(0, 0, 0, 0), theme.colors);
        let network_scene = Renderer::new().render(root(size), size, &mut recorder);
        let network_list = recorder.finish();
        raster.render(&network_list, &Damage::Full);
        if let Ok(path) = std::env::var("COCONUT_SETTINGS_NETWORK_SNAPSHOT") {
            raster
                .pixmap()
                .save_png(path)
                .expect("save Network preview");
        }
        assert!(matches!(
            connectivity.current(),
            crate::views::connectivity::View::Network
        ));
        assert!(
            network_scene
                .hit_test(Point {
                    x: 1000.0,
                    y: 134.0
                })
                .is_some(),
            "hostname Edit button is aligned at the right of its row"
        );
        assert!(
            network_scene
                .hit_test(Point { x: 975.0, y: 293.0 })
                .is_none(),
            "unsupported DNS configuration is disabled"
        );
        network_scene
            .hit_test(Point { x: 306.0, y: 28.0 })
            .expect("Network back button")();
        assert!(matches!(
            connectivity.current(),
            crate::views::connectivity::View::Overview
        ));
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
            routes::detail::Page::Displays,
            routes::detail::Page::Devices,
            routes::detail::Page::Printers,
            routes::detail::Page::AddPrinter,
            routes::detail::Page::Fonts,
            routes::detail::Page::InstalledApps,
            routes::detail::Page::App("test.desktop".into()),
            routes::detail::Page::FileTypes,
            routes::detail::Page::Startup,
            routes::detail::Page::AddStartup,
            routes::detail::Page::Password,
            routes::detail::Page::AddUser,
            routes::detail::Page::OnlineAccounts,
            routes::detail::Page::Fingerprint,
            routes::detail::Page::Camera,
            routes::detail::Page::Microphone,
            routes::detail::Page::Notifications,
            routes::detail::Page::HotCorners,
            routes::detail::Page::WindowRules,
            routes::detail::Page::Updates,
            routes::detail::Page::Storage,
            routes::detail::Page::Activity,
            routes::detail::Page::DateTime,
            routes::detail::Page::Language,
            routes::detail::Page::Reset,
        ];
        for (name, base) in [
            ("light", Theme::light()),
            ("dark", Theme::dark()),
            ("midnight", Theme::midnight()),
        ] {
            let theme = components::settings_theme(base);
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
                    Section::Widget("control_center".into()),
                    Section::Islands,
                    Section::Layout,
                    Section::Titlebar,
                    Section::WorkingArea,
                    Section::Effects,
                    Section::Profile,
                ])
            {
                routes::navigate(view, &section);
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
                if routes::destination::parent(&section) != section {
                    assert!(
                        scene.window_drag_at(Point { x: 306.0, y: 28.0 }).is_none(),
                        "back button swallowed by drag: {title}"
                    );
                    scene
                        .hit_test(Point { x: 306.0, y: 28.0 })
                        .expect("compact back button")();
                    assert!(
                        routes::current(view) == routes::destination::parent(&section),
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

#[test]
fn bar_editor_scenes_and_transients() {
    use creamui_core::{Point, Renderer};
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    use creamui_theme::{Theme, ThemeProvider};
    creamui_reactive::with_context_scope(|| {
        let preview = Preview::new();
        let context = &preview.context;
        let config = &context.config;
        let bar_content = &context.bar_content;
        let dock_scroll = &context.dock_scroll;
        let view = &context.view;
        let root = |size| preview.render(size);
        let default_config = config.peek();
        for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
            let theme = components::settings_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            for (width, height) in [(1100, 760), (640, 480)] {
                let size = Size {
                    width: width as f32,
                    height: height as f32,
                };
                for state in [
                    "contents",
                    "long-name",
                    "empty",
                    "hidden",
                    "catalog",
                    "menu",
                    "filtered",
                    "no-results",
                    "error",
                    "position",
                    "appearance",
                ] {
                    config.set(default_config.clone());
                    bar_content.dismiss();
                    bar_content.editor.status.set(String::new());
                    dock_scroll.set(0.0);
                    routes::open(
                        view,
                        &format!(
                            "/desktop/statusbar?tab={}",
                            if matches!(state, "position" | "appearance") {
                                state
                            } else {
                                "contents"
                            }
                        ),
                    );
                    match state {
                        "long-name" => config.update(|config| config.statusbar.sections[0].islands.push(coconut_core::IslandEntry::with_id("An unavailable custom widget with a very long name that still needs to be editable"))),
                        "empty" => config.update(|config| config.statusbar.sections.clear()),
                        "hidden" => {
                            config.update(|config| config.statusbar.sections[0].enabled = false)
                        }
                        "catalog" | "filtered" | "no-results" => {
                            bar_content.preview_catalog();
                            let query = if state == "filtered" {
                                "clipboard"
                            } else if state == "no-results" {
                                "no widget matches this"
                            } else {
                                ""
                            };
                            for ch in query.chars() {
                                bar_content.handle_key(
                                    creamui_core::KeyInput {
                                        key: creamui_core::Key::Char(ch),
                                        modifiers: Default::default(),
                                    },
                                    true,
                                );
                            }
                        }
                        "menu" => {
                            let controller = bar_content.editor.menu.controller();
                            controller.open(Point { x: 320.0, y: 150.0 }, coconut_plugin_kit::bar_editor::content_menu(
                                coconut_core::BarKind::Statusbar, &config.peek().statusbar,
                                coconut_plugin_kit::bar_editor::Target::Group(0),
                                Rc::new(|_| {}), Rc::new(|_| {}),
                            ));

                        }
                        "error" => bar_content.editor.report_error(
                            "The configuration folder is read-only. Your bar has not changed."
                                .into(),
                        ),
                        _ => {}
                    }
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(width, height, 1.0, theme.colors.surface, theme.colors);
                    let scene = Renderer::new().render(root(size), size, &mut recorder);
                    let list = recorder.finish();
                    assert!(list.items.len() > 50, "empty bar editor: {state}");
                    let mut raster = Rasterizer::new(width, height);
                    raster.render(&list, &Damage::Full);
                    if let Ok(directory) = std::env::var("COCONUT_BAR_SNAPSHOTS") {
                        raster
                            .pixmap()
                            .save_png(
                                std::path::Path::new(&directory)
                                    .join(format!("{variant}-{width}-{state}.png")),
                            )
                            .unwrap();
                    }
                    if matches!(
                        state,
                        "catalog" | "filtered" | "no-results" | "error" | "menu"
                    ) {
                        assert!(
                            scene.window_drag_at(Point { x: 600.0, y: 28.0 }).is_none(),
                            "modal must shield titlebar"
                        );
                        bar_content.handle_key(
                            creamui_core::KeyInput {
                                key: creamui_core::Key::Escape,
                                modifiers: Default::default(),
                            },
                            true,
                        );
                        assert!(
                            bar_content.overlays(size).is_empty(),
                            "Escape dismisses {state}"
                        );
                    }
                }
            }
        }
        config.set(default_config);
    });
}

fn editor_scene(preview: &Preview, size: Size, snapshot: &str) -> creamui_core::Scene {
    use creamui_render::{Damage, Rasterizer, SceneRecorder};
    let theme = creamui_theme::use_theme();
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        theme.colors.surface,
        theme.colors,
    );
    let scene = creamui_core::Renderer::new().render(preview.render(size), size, &mut recorder);
    let list = recorder.finish();
    if let Ok(directory) = std::env::var("COCONUT_BAR_SNAPSHOTS") {
        let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
        raster.render(&list, &Damage::Full);
        raster
            .pixmap()
            .save_png(std::path::Path::new(&directory).join(format!("{snapshot}.png")))
            .unwrap();
    }
    scene
}

fn begin_editor_drag(scene: &creamui_core::Scene, point: creamui_core::Point) -> usize {
    let (rect, start) = scene
        .drag_start_at(point)
        .unwrap_or_else(|| panic!("no draggable handle at {point:?}"));
    start(
        creamui_core::Point {
            x: point.x - rect.x,
            y: point.y - rect.y,
        },
        rect,
    );
    scene.drag_hit_test(point).expect("drag hit region")
}

fn move_editor_drag(scene: &creamui_core::Scene, index: usize, point: creamui_core::Point) {
    let (rect, update) = scene.draggable_at(index).unwrap();
    update(
        creamui_core::Point {
            x: point.x - rect.x,
            y: point.y - rect.y,
        },
        rect,
    );
}

#[test]
fn bar_editor_drag_reorders_items_groups_and_moves_to_another_group() {
    use crate::views::desktop::bar_drag::Region;
    use creamui_core::Point;
    use creamui_theme::{Theme, ThemeProvider};
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (width, height) in [(1100, 760), (640, 480)] {
            creamui_reactive::with_context_scope(|| {
                let preview = Preview::new();
                creamui_reactive::provide_context(ThemeProvider::new(components::settings_theme(
                    base,
                )));
                let context = &preview.context;
                routes::open(&context.view, "/desktop/statusbar?tab=contents");
                context
                    .bar_content
                    .preview_edits(context.config.clone(), coconut_core::BarKind::Statusbar);
                let size = Size {
                    width: width as f32,
                    height: height as f32,
                };
                editor_scene(&preview, size, "initial-drag-fixture");
                if width == 640 {
                    context.dock_scroll.set(130.0);
                }
                let scene = editor_scene(&preview, size, "scrolled-drag-fixture");
                let first = context
                    .bar_content
                    .preview_rect(Region::Item { group: 0, item: 0 });
                let second = context
                    .bar_content
                    .preview_rect(Region::Item { group: 0, item: 1 });
                let source = Point {
                    x: second.x + 24.0,
                    y: second.y + second.height / 2.0,
                };
                let target = Point {
                    x: first.x + 100.0,
                    y: first.y + 4.0,
                };
                let before = context.config.peek();
                let index = begin_editor_drag(&scene, source);
                move_editor_drag(&scene, index, target);
                assert_eq!(context.config.peek(), before, "drag preview must not save");
                let during =
                    editor_scene(&preview, size, &format!("{variant}-{width}-drag-reorder"));
                during.drag_end_at(index).unwrap()();
                assert_eq!(
                    context.config.peek().statusbar.sections[0].islands[0].id,
                    "open_windows"
                );
                context.dock_scroll.set(0.0);
                let scene = editor_scene(&preview, size, "before-group-drop");
                let row = context
                    .bar_content
                    .preview_rect(Region::Item { group: 0, item: 0 });
                let group = context.bar_content.preview_rect(Region::Group(2));
                let index = begin_editor_drag(
                    &scene,
                    Point {
                        x: row.x + 24.0,
                        y: row.y + row.height / 2.0,
                    },
                );
                move_editor_drag(
                    &scene,
                    index,
                    Point {
                        x: group.x + group.width / 2.0,
                        y: group.y + group.height / 2.0,
                    },
                );
                let during =
                    editor_scene(&preview, size, &format!("{variant}-{width}-drag-transfer"));
                during.drag_end_at(index).unwrap()();
                assert_eq!(
                    context.config.peek().statusbar.sections[2]
                        .islands
                        .last()
                        .unwrap()
                        .id,
                    "open_windows"
                );
                assert_eq!(context.config.peek().statusbar.sections[0].islands.len(), 1);
                let scene = editor_scene(&preview, size, "before-group-reorder");
                let source = context.bar_content.preview_rect(Region::Group(2));
                let target = context.bar_content.preview_rect(Region::Group(0));
                let index = begin_editor_drag(
                    &scene,
                    Point {
                        x: source.x + source.width / 2.0,
                        y: source.y + 20.0,
                    },
                );
                move_editor_drag(
                    &scene,
                    index,
                    Point {
                        x: target.x + 2.0,
                        y: target.y + 20.0,
                    },
                );
                let during =
                    editor_scene(&preview, size, &format!("{variant}-{width}-drag-groups"));
                during.drag_end_at(index).unwrap()();
                assert_eq!(
                    context.config.peek().statusbar.sections[0].islands[0].id,
                    "control_center"
                );
            });
        }
    }
}

#[test]
fn bar_editor_drag_cancels_and_keyboard_moves_the_same_placements() {
    use crate::views::desktop::bar_drag::Region;
    use creamui_core::{Key, KeyInput, Point};
    creamui_reactive::with_context_scope(|| {
        let preview = Preview::new();
        let context = &preview.context;
        routes::open(&context.view, "/desktop/statusbar?tab=contents");
        context
            .bar_content
            .preview_edits(context.config.clone(), coconut_core::BarKind::Statusbar);
        let size = Size {
            width: 1100.0,
            height: 760.0,
        };
        for cancel in ["click", "escape", "outside", "focus"] {
            let scene = editor_scene(&preview, size, "cancellation-fixture");
            let row = context
                .bar_content
                .preview_rect(Region::Item { group: 0, item: 0 });
            let point = Point {
                x: row.x + 24.0,
                y: row.y + 20.0,
            };
            let before = context.config.peek();
            let index = begin_editor_drag(&scene, point);
            if cancel != "click" {
                move_editor_drag(
                    &scene,
                    index,
                    Point {
                        x: point.x + 20.0,
                        y: point.y + 100.0,
                    },
                );
            }
            match cancel {
                "escape" => {
                    context.bar_content.handle_key(
                        KeyInput {
                            key: Key::Escape,
                            modifiers: Default::default(),
                        },
                        true,
                    );
                }
                "outside" => move_editor_drag(&scene, index, Point { x: 10.0, y: 10.0 }),
                "focus" => context.bar_content.focus_lost(),
                _ => {}
            }
            scene.drag_end_at(index).unwrap()();
            assert_eq!(
                context.config.peek(),
                before,
                "canceled drag changed content: {cancel}"
            );
        }
        let scene = editor_scene(&preview, size, "keyboard-fixture");
        let row = context
            .bar_content
            .preview_rect(Region::Item { group: 0, item: 0 });
        let focused = scene
            .focus_hit_test(Point {
                x: row.x + 24.0,
                y: row.y + 20.0,
            })
            .unwrap();
        scene.on_key_at(focused).unwrap()(KeyInput {
            key: Key::Down,
            modifiers: creamui_core::Modifiers {
                alt: true,
                ..Default::default()
            },
        });
        assert_eq!(
            context.config.peek().statusbar.sections[0].islands[0].id,
            "open_windows"
        );
    });
}

#[test]
fn bar_editor_keyboard_focus_follows_the_reordered_item() {
    use crate::views::desktop::bar_drag::Region;
    use creamui_core::{Key, KeyInput, Point, Renderer};
    use creamui_render::SceneRecorder;
    creamui_reactive::with_context_scope(|| {
        let preview = Preview::new();
        let context = &preview.context;
        routes::open(&context.view, "/desktop/statusbar?tab=contents");
        context.config.update(|config| {
            config.statusbar.sections[0]
                .islands
                .push(coconut_core::IslandEntry::with_id("clock"))
        });
        context
            .bar_content
            .preview_edits(context.config.clone(), coconut_core::BarKind::Statusbar);
        let size = Size {
            width: 1100.0,
            height: 760.0,
        };
        let mut renderer = Renderer::new();
        let mut recorder = SceneRecorder::new();
        let theme = creamui_theme::use_theme();
        let mut render = || {
            recorder.begin(1100, 760, 1.0, theme.colors.surface, theme.colors);
            renderer.render(preview.render(size), size, &mut recorder)
        };
        let scene = render();
        let rect = context
            .bar_content
            .preview_rect(Region::Item { group: 0, item: 0 });
        let focus = scene
            .focus_hit_test(Point {
                x: rect.x + 24.0,
                y: rect.y + 20.0,
            })
            .unwrap();
        let id = scene.focus_id_at(focus).unwrap();
        let down = KeyInput {
            key: Key::Down,
            modifiers: creamui_core::Modifiers {
                alt: true,
                ..Default::default()
            },
        };
        scene.on_key_at(focus).unwrap()(down);
        let scene = render();
        let next_focus = scene.focus_index(id).expect("moved item keeps focus");
        scene.on_key_at(next_focus).unwrap()(down);
        assert_eq!(
            context.config.peek().statusbar.sections[0]
                .islands
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            ["open_windows", "clock", "workspaces"]
        );
    });
}

#[test]
fn widget_catalog_and_every_detail_page_render_in_both_sizes() {
    use coconut_api::settings::{Choice, Preference, Snapshot, Value};
    use creamui_core::Point;
    use creamui_theme::{Theme, ThemeProvider};
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (width, height) in [(1100, 760), (640, 480)] {
            creamui_reactive::with_context_scope(|| {
                let preview = Preview::new();
                let context = &preview.context;
                let theme = components::settings_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let mut native = Snapshot {
                    loaded: true,
                    ..Default::default()
                };
                native.preferences.insert(
                    "clipboard-enabled".into(),
                    Preference {
                        writable: true,
                        value: Some(Value::Bool(true)),
                        ..Default::default()
                    },
                );
                for (key, id, label) in [
                    ("clipboard-max-entries", "50", "50 entries"),
                    ("clipboard-max-age", "86400", "24 hours"),
                ] {
                    native.preferences.insert(
                        key.into(),
                        Preference {
                            writable: true,
                            value: Some(Value::Text(id.into())),
                            choices: vec![Choice {
                                id: id.into(),
                                label: label.into(),
                            }],
                            ..Default::default()
                        },
                    );
                }
                context.native.snapshot.set(native);
                let size = Size {
                    width: width as f32,
                    height: height as f32,
                };
                routes::open(&context.view, "/desktop/widgets");
                editor_scene(&preview, size, &format!("{variant}-{width}-widgets"));
                for (query, label) in [
                    ("sound", "widgets-search"),
                    ("no such widget here", "widgets-empty"),
                ] {
                    context.island_settings.search.set_value(query);
                    editor_scene(&preview, size, &format!("{variant}-{width}-{label}"));
                }
                context.island_settings.search.set_value("");
                for item in coconut_plugin_kit::bar_editor::BAR_ITEMS {
                    context.content_scroll.set(0.0);
                    routes::open(&context.view, item.settings_route);
                    let scene = editor_scene(
                        &preview,
                        size,
                        &format!("{variant}-{width}-widget-{}", item.id),
                    );
                    assert!(routes::current(&context.view) == Section::Widget(item.id.into()));
                    scene
                        .hit_test(Point { x: 306.0, y: 28.0 })
                        .expect("widget back control")();
                    assert!(
                        routes::current(&context.view) == Section::Islands,
                        "widget back goes to catalog"
                    );
                }
                routes::open(&context.view, "/desktop/tray");
                assert!(routes::current(&context.view) == Section::Widget("control_center".into()));
                editor_scene(&preview, size, &format!("{variant}-{width}-legacy-tray"));
            });
        }
    }
}

#[test]
fn global_search_opens_widget_configuration_and_preserves_query_focus() {
    use creamui_core::Point;
    use creamui_theme::{Theme, ThemeProvider};
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (width, height) in [(1100, 760), (640, 480)] {
            creamui_reactive::with_context_scope(|| {
                let preview = Preview::new();
                let context = &preview.context;
                creamui_reactive::provide_context(ThemeProvider::new(components::settings_theme(
                    base,
                )));
                let size = Size {
                    width: width as f32,
                    height: height as f32,
                };
                for query in ["clipboard", "clock", "sound", "no such setting"] {
                    context.settings_search.set_value(query);
                    context.content_scroll.set(0.0);
                    editor_scene(
                        &preview,
                        size,
                        &format!(
                            "{variant}-{width}-global-search-{}",
                            query.replace(' ', "-")
                        ),
                    );
                }
                context.settings_search.set_value("clipboard");
                let scene = editor_scene(&preview, size, "global-search-click");
                let point = (100..height)
                    .find_map(|y| {
                        let point = Point {
                            x: (width as f32 + 256.0) / 2.0,
                            y: y as f32,
                        };
                        scene.hit_test(point).is_some().then_some(point)
                    })
                    .expect("visible widget search result");
                scene.hit_test(point).unwrap()();
                assert!(context.settings_search.peek().is_empty());
                assert!(routes::current(&context.view) == Section::Widget("clipboard".into()));
            });
        }
    }
}
