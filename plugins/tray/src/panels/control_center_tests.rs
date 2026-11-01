use super::*;
use coconut_plugin_kit::{design, SharedState};
use creamui_core::{Key, Point, Renderer, Scene};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};

struct Network;

impl NetworkIntegration for Network {
    fn connected(&self) -> bool {
        true
    }
    fn network_name(&self) -> Option<String> {
        Some("Coconut · Oficina central · Red de invitados 日本語".into())
    }
    fn strength(&self) -> Option<u8> {
        Some(80)
    }
    fn enabled(&self) -> bool {
        true
    }
    fn set_enabled(&self, _: bool) {}
}

struct Battery;
impl BatteryIntegration for Battery {
    fn percentage(&self) -> Option<u8> {
        Some(78)
    }
    fn charging(&self) -> bool {
        true
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
        Some("Studio headphones".into())
    }
    fn set_powered(&self, _: bool) {}
}

fn fixture(size: Size) -> (ControlCenterPanel, PanelRenderContext) {
    let panel = ControlCenterPanel {
        tray: TrayConfig::default(),
        layout: Signal::new(ControlLayout::default()),
        app: None,
    };
    let ctx = PanelRenderContext {
        size,
        shared: SharedState::default(),
        scratch: SharedState::default(),
        open_panel: Rc::new(|_, _| {}),
        close: Rc::new(|| {}),
    };
    ctx.shared.insert(WifiEnabled(Signal::new(true)));
    ctx.shared
        .insert(Rc::new(Network) as Rc<dyn NetworkIntegration>);
    ctx.shared
        .insert(Rc::new(Battery) as Rc<dyn BatteryIntegration>);
    ctx.shared
        .insert(Rc::new(Bluetooth) as Rc<dyn BluetoothIntegration>);
    ctx.shared.insert(BrightnessLevel(Signal::new(0.65)));
    ctx.shared.insert(VolumeLevel(Signal::new(0.42)));
    (panel, ctx)
}

#[test]
fn cards_adapt_to_wide_tall_and_large_layouts() {
    use crate::ControlTile;
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            for width in [380.0, 320.0] {
                let (panel, ctx) = fixture(Size {
                    width,
                    height: 620.0,
                });
                panel.layout.set(ControlLayout {
                    tiles: [
                        (Control::Wifi, 0, 0, 2, 1),
                        (Control::Bluetooth, 2, 0, 1, 1),
                        (Control::Brightness, 0, 1, 1, 2),
                        (Control::Volume, 1, 1, 1, 2),
                        (Control::Battery, 2, 1, 1, 2),
                        (Control::KeepAwake, 0, 3, 3, 1),
                    ]
                    .into_iter()
                    .map(|(control, column, row, width, height)| ControlTile {
                        control,
                        column,
                        row,
                        width,
                        height,
                    })
                    .collect(),
                });
                let scene = render(&panel, &ctx, theme, &format!("{variant}-{width}-adaptive"));
                let slider = (280..440)
                    .find_map(|y| {
                        let point = Point {
                            x: if width == 380.0 { 55.0 } else { 48.0 },
                            y: y as f32,
                        };
                        scene
                            .drag_hit_test(point)
                            .map(|index| scene.draggable_at(index).unwrap())
                    })
                    .expect("the tall brightness slider stays visible and interactive");
                (slider.1)(
                    Point {
                        x: slider.0.width / 2.0,
                        y: slider.0.height * 0.2,
                    },
                    slider.0,
                );
                assert!(ctx.shared.get::<BrightnessLevel>().unwrap().0.peek() > 0.7);
            }
            for control in Control::ALL {
                let (panel, ctx) = fixture(Size {
                    width: 380.0,
                    height: 620.0,
                });
                panel.layout.set(ControlLayout {
                    tiles: vec![ControlTile {
                        control,
                        column: 0,
                        row: 0,
                        width: 3,
                        height: 3,
                    }],
                });
                let scene = render(
                    &panel,
                    &ctx,
                    theme,
                    &format!("{variant}-large-{}", control.label()),
                );
                assert!(
                    scene
                        .focus_hit_test(Point {
                            x: if control == Control::KeepAwake {
                                240.0
                            } else {
                                45.0
                            },
                            y: 80.0
                        })
                        .is_some(),
                    "the expanded card must expose its primary control"
                );
            }
        });
    }
}

#[test]
fn compact_controls_keep_their_toggle_slider_and_navigation_actions() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let (panel, ctx) = fixture(Size {
            width: 380.0,
            height: 620.0,
        });
        panel.layout.update(|layout| {
            for tile in &mut layout.tiles {
                tile.width = 1;
            }
            layout.normalize();
        });
        let scene = render(&panel, &ctx, theme, "compact-controls");
        scene
            .hit_test(Point { x: 68.0, y: 78.0 })
            .expect("Wi-Fi toggle is visible")();
        assert!(!ctx.shared.get::<WifiEnabled>().unwrap().0.peek());
        let slider = (230..420)
            .find_map(|y| {
                let point = Point {
                    x: 140.0,
                    y: y as f32,
                };
                scene
                    .drag_hit_test(point)
                    .map(|index| scene.draggable_at(index).unwrap())
            })
            .expect("the single-column volume slider remains draggable");
        (slider.1)(
            Point {
                x: slider.0.width / 2.0,
                y: slider.0.height * 0.2,
            },
            slider.0,
        );
        assert!(ctx.shared.get::<VolumeLevel>().unwrap().0.peek() > 0.7);
        scene
            .hit_test(Point { x: 30.0, y: 135.0 })
            .expect("Wi-Fi card opens its details")();
        assert!(matches!(
            ctx.scratch.get::<Signal<PanelView>>().unwrap().peek(),
            PanelView::Network
        ));
    });
}

#[test]
fn small_level_dials_support_keys_and_wheel_without_a_center_click_jump() {
    use crate::ControlTile;
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let (panel, ctx) = fixture(Size {
            width: 380.0,
            height: 620.0,
        });
        panel.layout.set(ControlLayout {
            tiles: [Control::Brightness, Control::Volume, Control::Battery]
                .into_iter()
                .enumerate()
                .map(|(column, control)| ControlTile {
                    control,
                    column: column as u8,
                    row: 0,
                    width: 1,
                    height: 1,
                })
                .collect(),
        });
        let scene = render(&panel, &ctx, theme, "light-mini-rings");
        let point = Point { x: 55.0, y: 110.0 };
        let index = scene
            .drag_hit_test(point)
            .expect("the circular brightness control is interactive");
        let (rect, drag) = scene.draggable_at(index).unwrap();
        drag(
            Point {
                x: rect.width / 2.0,
                y: rect.height / 2.0,
            },
            rect,
        );
        assert_eq!(ctx.shared.get::<BrightnessLevel>().unwrap().0.peek(), 0.65);
        let scroll = scene.scroll_hit_test(point).unwrap();
        scene.on_scroll_at(scroll).unwrap()(-50.0);
        assert!(ctx.shared.get::<BrightnessLevel>().unwrap().0.peek() > 0.7);
        let focus = scene.focus_hit_test(point).unwrap();
        scene.on_key_at(focus).unwrap()(creamui_core::KeyInput {
            key: Key::End,
            modifiers: Default::default(),
        });
        assert_eq!(ctx.shared.get::<BrightnessLevel>().unwrap().0.peek(), 1.0);
    });
}

fn render(panel: &ControlCenterPanel, ctx: &PanelRenderContext, theme: Theme, name: &str) -> Scene {
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        ctx.size.width as u32,
        ctx.size.height as u32,
        1.0,
        Color::rgba(0, 0, 0, 0),
        theme.colors,
    );
    let scene = Renderer::new().render(panel.build(ctx), ctx.size, &mut recorder);
    let mut raster = Rasterizer::new(ctx.size.width as u32, ctx.size.height as u32);
    raster.render(&recorder.finish(), &Damage::Full);
    assert!(raster.pixmap().pixel(0, 0).unwrap().alpha() > 0);
    if let Ok(prefix) = std::env::var("COCONUT_TRAY_SNAPSHOT") {
        raster
            .pixmap()
            .save_png(format!("{prefix}-{name}.png"))
            .unwrap();
    }
    scene
}

fn edit_point(scene: &Scene) -> Point {
    (110..240)
        .find_map(|y| {
            let point = Point {
                x: 35.0,
                y: y as f32,
            };
            scene.drag_hit_test(point).map(|_| point)
        })
        .expect("the first editable tile must have a visible drag target")
}

#[test]
fn popup_height_tracks_content_editing_errors_and_device_navigation() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            let (panel, mut ctx) = fixture(Size {
                width: 380.0,
                height: 620.0,
            });
            ctx.size = panel.preferred_size(&ctx.scratch);
            assert_eq!(ctx.size.height, 398.0);
            render(&panel, &ctx, theme, &format!("{variant}-fitted"));
            let editor = ctx.scratch.get::<Editor>().unwrap();
            editor.editing.set(true);
            ctx.size = panel.preferred_size(&ctx.scratch);
            assert_eq!(ctx.size.height, 464.0);
            render(&panel, &ctx, theme, &format!("{variant}-fitted-edit"));
            editor.layout.set(ControlLayout { tiles: vec![] });
            ctx.size = panel.preferred_size(&ctx.scratch);
            assert_eq!(ctx.size.height, 244.0);
            render(&panel, &ctx, theme, &format!("{variant}-fitted-empty"));
            editor.error.set(Some("Could not save layout".into()));
            assert_eq!(panel.preferred_size(&ctx.scratch).height, 276.0);
            let view = ctx.scratch.get::<Signal<PanelView>>().unwrap();
            view.set(PanelView::Network);
            assert_eq!(panel.preferred_size(&ctx.scratch).height, 620.0);
            view.set(PanelView::Main);
            editor.cancel(&panel.layout);
            assert_eq!(panel.preferred_size(&ctx.scratch).height, 398.0);
        });
    }
}

#[test]
fn renders_normal_edit_empty_and_scrolled_grids_in_both_themes() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        for (width, height) in [(380.0, 620.0), (320.0, 480.0)] {
            creamui_reactive::with_context_scope(|| {
                design::load_fonts();
                let theme = design::coconut_theme(base);
                creamui_reactive::provide_context(ThemeProvider::new(theme));
                let (panel, ctx) = fixture(Size { width, height });
                let normal = render(&panel, &ctx, theme, &format!("{variant}-{width}-normal"));
                let editor = ctx.scratch.get::<Editor>().unwrap();
                normal
                    .hit_test(Point {
                        x: width - 38.0,
                        y: 33.0,
                    })
                    .expect("Edit is clickable beside the panel title")();
                assert!(editor.editing.peek());
                let scene = render(&panel, &ctx, theme, &format!("{variant}-{width}-editing"));
                edit_point(&scene);
                editor.layout.update(|layout| {
                    for (index, tile) in layout.tiles.iter_mut().enumerate() {
                        tile.width = 3;
                        tile.height = 3;
                        tile.column = 0;
                        tile.row = index as u16 * 3;
                    }
                });
                render(&panel, &ctx, theme, &format!("{variant}-{width}-tall"));
                editor.scroll.set(600.0);
                let scene = render(&panel, &ctx, theme, &format!("{variant}-{width}-scrolled"));
                assert!(
                    scene.drag_hit_test(Point { x: 30.0, y: 30.0 }).is_none(),
                    "scrolling tiles must stay clipped below the header"
                );
                editor.layout.set(ControlLayout { tiles: vec![] });
                render(&panel, &ctx, theme, &format!("{variant}-{width}-empty"));
                assert_eq!(
                    editor.scroll.peek(),
                    0.0,
                    "removing cards must not leave the empty state scrolled out of view"
                );
                editor
                    .error
                    .set(Some("Could not save layout: permission denied".into()));
                render(&panel, &ctx, theme, &format!("{variant}-{width}-error"));
            });
        }
    }
}

#[test]
fn context_menu_resizes_removes_adds_and_cancels_without_changing_system_controls() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let (panel, ctx) = fixture(Size {
            width: 380.0,
            height: 620.0,
        });
        let scene = render(&panel, &ctx, theme, "context-before");
        let point = Point { x: 35.0, y: 160.0 };
        scene.context_menu_at(point).unwrap()(point);
        let editor = ctx.scratch.get::<Editor>().unwrap();
        assert!(editor.editing.peek());
        assert!(editor.menu.is_open());
        assert!(ctx.shared.get::<WifiEnabled>().unwrap().0.peek());
        render(&panel, &ctx, theme, "context-menu");
        for key in [Key::Home, Key::Right, Key::End, Key::Enter] {
            editor.menu.handle_key(&key);
        }
        let wifi = editor
            .layout
            .peek()
            .tiles
            .into_iter()
            .find(|tile| tile.control == Control::Wifi)
            .unwrap();
        assert_eq!(wifi.width, COLUMNS);
        editor.open_tile(point, wifi);
        for key in [Key::End, Key::Enter] {
            editor.menu.handle_key(&key);
        }
        assert!(!editor
            .layout
            .peek()
            .tiles
            .iter()
            .any(|tile| tile.control == Control::Wifi));
        editor.open_add(point, &Control::ALL);
        for key in [Key::Home, Key::Enter] {
            editor.menu.handle_key(&key);
        }
        assert!(editor
            .layout
            .peek()
            .tiles
            .iter()
            .any(|tile| tile.control == Control::Wifi));
        let scene = render(&panel, &ctx, theme, "keyboard-ready");
        let focus = scene.focus_hit_test(edit_point(&scene)).unwrap();
        scene.on_key_at(focus).unwrap()(creamui_core::KeyInput {
            key: Key::Right,
            modifiers: Default::default(),
        });
        assert_eq!(editor.layout.peek().tiles[0].column, 1);
        let scene = render(&panel, &ctx, theme, "keyboard-moved");
        let focus = scene.focus_hit_test(Point { x: 150.0, y: 140.0 }).unwrap();
        scene.on_key_at(focus).unwrap()(creamui_core::KeyInput {
            key: Key::Down,
            modifiers: creamui_core::Modifiers {
                shift: true,
                ..Default::default()
            },
        });
        assert_eq!(editor.layout.peek().tiles[0].height, 2);
        scene.on_key_at(focus).unwrap()(creamui_core::KeyInput {
            key: Key::Delete,
            modifiers: Default::default(),
        });
        assert!(!editor
            .layout
            .peek()
            .tiles
            .iter()
            .any(|tile| tile.control == Control::Wifi));
        editor.cancel(&panel.layout);
        assert_eq!(editor.layout.peek(), ControlLayout::default());
        assert!(!editor.editing.peek());
    });
}

#[test]
fn edit_scene_drags_and_resizes_tiles_and_done_keeps_layout_across_opens() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::dark());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let (panel, ctx) = fixture(Size {
            width: 380.0,
            height: 620.0,
        });
        render(&panel, &ctx, theme, "drag-before");
        let editor = ctx.scratch.get::<Editor>().unwrap();
        editor.editing.set(true);
        let scene = render(&panel, &ctx, theme, "drag-ready");
        let point = edit_point(&scene);
        let index = scene.drag_hit_test(point).unwrap();
        let (rect, start) = scene.drag_start_at(point).unwrap();
        let (_, drag) = scene.draggable_at(index).unwrap();
        let column_step = (rect.width + GAP) / 2.0;
        start(
            Point {
                x: point.x - rect.x,
                y: point.y - rect.y,
            },
            rect,
        );
        drag(
            Point {
                x: point.x - rect.x + column_step,
                y: point.y - rect.y + 110.0,
            },
            rect,
        );
        assert_eq!(
            editor.layout.peek(),
            ControlLayout::default(),
            "tiles stay in place until the drop"
        );
        let preview_scene = render(&panel, &ctx, theme, "drag-preview");
        let (current_rect, current_drag) = preview_scene.draggable_at(index).unwrap();
        assert_eq!(
            current_rect, rect,
            "the native drag target must stay stable across rebuilds"
        );
        current_drag(
            Point {
                x: point.x - current_rect.x + column_step,
                y: point.y - current_rect.y + 110.0,
            },
            current_rect,
        );
        preview_scene.drag_end_at(index).unwrap()();
        let tile = editor.layout.peek().tiles[0];
        assert_eq!(tile.control, Control::Wifi);
        assert_eq!((tile.column, tile.row), (1, 1));
        let scene = render(&panel, &ctx, theme, "drag-moved");
        let moved_point = Point {
            x: point.x + column_step,
            y: point.y + 110.0,
        };
        let moved = scene
            .draggable_at(scene.drag_hit_test(moved_point).unwrap())
            .unwrap()
            .0;
        let resize_point = Point {
            x: moved.x + moved.width - 18.0,
            y: moved.y + moved.height - 18.0,
        };
        assert_eq!(
            scene.cursor_hit_test(resize_point),
            Some(creamui_core::CursorIcon::ResizeNwse)
        );
        let index = scene
            .drag_hit_test(resize_point)
            .expect("resize handle is visible");
        let (handle, start) = scene.drag_start_at(resize_point).unwrap();
        assert!(
            handle.width < rect.width,
            "the resize handle must win over the move target"
        );
        start(
            Point {
                x: resize_point.x - handle.x,
                y: resize_point.y - handle.y,
            },
            handle,
        );
        scene.draggable_at(index).unwrap().1(
            Point {
                x: resize_point.x - handle.x + column_step,
                y: resize_point.y - handle.y + 110.0,
            },
            handle,
        );
        scene.drag_end_at(index).unwrap()();
        assert_eq!(
            (
                editor.layout.peek().tiles[0].width,
                editor.layout.peek().tiles[0].height
            ),
            (3, 2)
        );
        let scene = render(&panel, &ctx, theme, "drag-resized");
        scene
            .hit_test(Point { x: 330.0, y: 78.0 })
            .expect("Done remains clickable")();
        assert!(!editor.editing.peek());
        assert_eq!(panel.layout.peek(), editor.layout.peek());
        let reopened = PanelRenderContext {
            scratch: SharedState::default(),
            ..ctx
        };
        render(&panel, &reopened, theme, "reopened");
        assert_eq!(
            reopened.scratch.get::<Editor>().unwrap().layout.peek(),
            panel.layout.peek()
        );
    });
}
