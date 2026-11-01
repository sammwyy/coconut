use super::*;
use coconut_core::PinnedApplication;
use coconut_plugin_kit::{design, SharedState};
use creamui_core::{Painter, Rect, Renderer, Size, TextAlign};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};
use std::cell::Cell;

#[derive(Default)]
struct BoundsPainter {
    images: Vec<Rect>,
}
impl Painter for BoundsPainter {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, _: Rect, _: &str, _: Color, _: f32, _: TextAlign) {}
    fn draw_image(&mut self, rect: Rect, _: &creamui_core::RgbaImage, _: Option<Color>) {
        self.images.push(rect);
    }
}

fn application(index: usize) -> AppEntry {
    AppEntry {
        desktop_file: Some(format!("/tmp/application-{index}.desktop").into()),
        name: if index == 0 {
            "An application with a very long Unicode name — Diseño 日本語".into()
        } else {
            format!("Application {index:03}")
        },
        exec: vec!["true".into()],
        terminal: false,
        category: if index.is_multiple_of(2) {
            "Graphics".into()
        } else {
            "Accessories".into()
        },
        icon_name: None,
        icon: Some(
            creamui_image::ImageData::from_svg(
                include_bytes!("../../../assets/icons/apps/app-files.svg"),
                creamui_image::SvgSize::Max(48),
            )
            .unwrap(),
        ),
    }
}

#[test]
fn picker_renders_grouped_filtered_and_empty_scenes_in_compact_sizes() {
    for (variant, base) in [("light", Theme::light()), ("dark", Theme::dark())] {
        creamui_reactive::with_context_scope(|| {
            design::load_fonts();
            let theme = design::coconut_theme(base);
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            for (width, height) in [(360.0, 480.0), (300.0, 320.0)] {
                for (name, query) in [
                    ("grouped", ""),
                    ("filtered", "Diseño"),
                    ("empty", "missing-app"),
                ] {
                    let shared = SharedState::default();
                    shared.insert(AppCatalog::from_entries(
                        (0..120).map(application).collect(),
                    ));
                    let scratch = SharedState::default();
                    let state = PickerState::default();
                    state.query.set_value(query);
                    scratch.insert(state);
                    let closed = Rc::new(Cell::new(false));
                    let close = closed.clone();
                    let ctx = PanelRenderContext {
                        size: Size { width, height },
                        shared,
                        scratch,
                        open_panel: Rc::new(|_, _| {}),
                        close: Rc::new(move || close.set(true)),
                    };
                    let mut renderer = Renderer::new();
                    let mut recorder = SceneRecorder::new();
                    recorder.begin(
                        width as u32,
                        height as u32,
                        1.0,
                        Color::rgba(0, 0, 0, 0),
                        theme.colors,
                    );
                    renderer.render(picker(&ctx), ctx.size, &mut recorder);
                    recorder.finish();
                    recorder.begin(
                        width as u32,
                        height as u32,
                        1.0,
                        Color::rgba(0, 0, 0, 0),
                        theme.colors,
                    );
                    let scene = renderer.render(picker(&ctx), ctx.size, &mut recorder);
                    assert!(
                        scene.focus_accepts_text_input(
                            scene
                                .next_focus(None, false)
                                .and_then(|first| scene.next_focus(Some(first), false))
                                .unwrap()
                        ),
                        "search remains in the keyboard focus order"
                    );
                    if name == "grouped" {
                        assert!(scene
                            .scroll_hit_test(Point { x: 120.0, y: 150.0 })
                            .is_some());
                    }
                    let display = recorder.finish();
                    let mut bounds = BoundsPainter::default();
                    renderer.render(picker(&ctx), ctx.size, &mut bounds);
                    let images = bounds.images.len();
                    assert!(images < 20, "only visible application rows are built");
                    if name != "empty" {
                        assert!(images > 0, "application icons must be visible");
                    }
                    let mut raster = Rasterizer::new(width as u32, height as u32);
                    raster.render(&display, &Damage::Full);
                    if let Ok(prefix) = std::env::var("COCONUT_PINNED_SNAPSHOT") {
                        raster
                            .pixmap()
                            .save_png(format!("{prefix}-{variant}-{width}-{name}.png"))
                            .unwrap();
                    }
                    let done = Point {
                        x: width - 40.0,
                        y: 27.0,
                    };
                    scene.hit_test(done).expect("Done stays visible")();
                    assert!(closed.get());
                }
            }
        });
    }
}

#[test]
fn pinned_icons_are_centered_in_both_bar_orientations_and_empty_space_has_a_context_menu() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::light());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        for vertical in [false, true] {
            for scale in [0.8, 1.0, 1.7] {
                let entries: Vec<_> = (0..3)
                    .map(|index| PinnedApp {
                        pin: PinnedApplication {
                            source: format!("/tmp/{index}.desktop").into(),
                            desktop_file: format!("/tmp/copy-{index}.desktop").into(),
                        },
                        application: Some(application(index)),
                    })
                    .collect();
                let size = if vertical {
                    Size {
                        width: 64.0,
                        height: 260.0,
                    }
                } else {
                    Size {
                        width: 260.0,
                        height: 64.0,
                    }
                };
                let mut recorder = SceneRecorder::new();
                let mut bounds = BoundsPainter::default();
                let scene = Renderer::new().render(
                    build_island(
                        entries.clone(),
                        None,
                        None,
                        vertical,
                        TaskIconStyle::new(&Default::default(), scale, false),
                        Rc::new(|_, _| {}),
                    ),
                    size,
                    &mut bounds,
                );
                if let Ok(prefix) = std::env::var("COCONUT_PINNED_SNAPSHOT") {
                    recorder.begin(
                        size.width as u32,
                        size.height as u32,
                        1.0,
                        theme.colors.surface,
                        theme.colors,
                    );
                    Renderer::new().render(
                        build_island(
                            entries,
                            None,
                            None,
                            vertical,
                            TaskIconStyle::new(&Default::default(), scale, false),
                            Rc::new(|_, _| {}),
                        ),
                        size,
                        &mut recorder,
                    );
                    let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                    raster.render(&recorder.finish(), &Damage::Full);
                    raster
                        .pixmap()
                        .save_png(format!("{prefix}-island-{vertical}-{scale}.png"))
                        .unwrap();
                }
                let images = bounds.images;
                assert_eq!(images.len(), 3);
                for bounds in &images {
                    let center = Point {
                        x: bounds.x + bounds.width / 2.0,
                        y: bounds.y + bounds.height / 2.0,
                    };
                    let hit = scene
                        .hit_test(center)
                        .expect("the icon stays inside its button");
                    for (length, horizontal) in [(size.width, true), (size.height, false)] {
                        let pixels: Vec<_> = (0..length as u32)
                            .filter(|value| {
                                let at = if horizontal {
                                    Point {
                                        x: *value as f32 + 0.5,
                                        y: center.y,
                                    }
                                } else {
                                    Point {
                                        x: center.x,
                                        y: *value as f32 + 0.5,
                                    }
                                };
                                scene
                                    .hit_test(at)
                                    .is_some_and(|candidate| Rc::ptr_eq(candidate, hit))
                            })
                            .collect();
                        let button_center =
                            (*pixels.first().unwrap() + *pixels.last().unwrap() + 1) as f32 / 2.0;
                        let icon_center = if horizontal { center.x } else { center.y };
                        assert!(
                            (button_center - icon_center).abs() <= 1.0,
                            "the icon is centered in its actual hit bounds"
                        );
                    }
                    assert!(scene.context_menu_at(center).is_some());
                }
                let opened = Rc::new(Cell::new(None));
                let target = opened.clone();
                recorder.begin(
                    size.width as u32,
                    size.height as u32,
                    1.0,
                    Color::rgba(0, 0, 0, 0),
                    theme.colors,
                );
                let empty = Renderer::new().render(
                    build_island(
                        Vec::new(),
                        None,
                        None,
                        vertical,
                        TaskIconStyle::new(&Default::default(), scale, false),
                        Rc::new(move |id, at| target.set(Some((id, at)))),
                    ),
                    size,
                    &mut recorder,
                );
                if let Ok(prefix) = std::env::var("COCONUT_PINNED_SNAPSHOT") {
                    let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
                    raster.render(&recorder.finish(), &Damage::Full);
                    raster
                        .pixmap()
                        .save_png(format!("{prefix}-empty-{vertical}-{scale}.png"))
                        .unwrap();
                }
                let at = Point {
                    x: 16.0 * scale,
                    y: 16.0 * scale,
                };
                assert!(empty.hit_test(at).is_none());
                assert!(empty.context_menu_at(at).is_some());
                let menu = pin_menu(Rc::new(move |id, at| opened.set(Some((id, at)))), at, None);
                let controller = creamui_widgets::ContextMenuController::new();
                controller.open(at, menu);
                controller.handle_key(&creamui_core::Key::Down);
                controller.handle_key(&creamui_core::Key::Enter);
            }
        }
    });
}

#[test]
fn many_pins_keep_every_application_accessible_through_the_overflow_menu() {
    creamui_reactive::with_context_scope(|| {
        design::load_fonts();
        let theme = design::coconut_theme(Theme::dark());
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let entries: Vec<_> = (0..20)
            .map(|index| PinnedApp {
                pin: PinnedApplication {
                    source: format!("/tmp/{index}.desktop").into(),
                    desktop_file: format!("/tmp/copy-{index}.desktop").into(),
                },
                application: Some(application(index)),
            })
            .collect();
        let menu = overflow_menu(&entries[6..], None, Rc::new(|_, _| {}), Point::default());
        for (index, entry) in entries[6..].iter().enumerate() {
            assert_eq!(menu[index].label, pinned_name(entry));
            assert!(menu[index].children.is_empty());
            assert!(menu[index].enabled);
        }
        let size = Size {
            width: 280.0,
            height: 44.0,
        };
        let mut bounds = BoundsPainter::default();
        let scene = Renderer::new().render(
            build_island(
                entries,
                None,
                None,
                false,
                TaskIconStyle::new(&Default::default(), 1.0, false),
                Rc::new(|_, _| {}),
            ),
            size,
            &mut bounds,
        );
        assert_eq!(bounds.images.len(), 6);
        assert!(scene.hit_test(Point { x: 251.0, y: 16.0 }).is_some());
    });
}
