use super::*;
use crate::asset::Metadata;
use creamui_core::{
    ImageEffects, Painter, Renderer, Scene, TextAlign, TitlebarMode, TitlebarState, WindowControls,
    WindowControlsBackend,
};
use creamui_image::ImageData;
use creamui_render::{
    platform::{Key, KeyEvent, Modifiers},
    Damage, DisplayList, Rasterizer, SceneRecorder,
};
use creamui_theme::{Color, Theme, ThemeProvider};
use std::path::Path;

fn image() -> Asset {
    let width = 1200;
    let height = 800;
    let mut pixels = vec![255; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            pixels[i] = (x * 255 / width) as u8;
            pixels[i + 1] = (y * 255 / height) as u8;
            pixels[i + 2] = 128;
        }
    }
    Asset {
        path: PathBuf::from("/pictures/A very long photograph name — 東京 and ñ.png"),
        data: ImageData::from_rgba(width as u32, height as u32, pixels).unwrap(),
        metadata: Metadata {
            format: "PNG".into(),
            color: "RGBA8".into(),
            bytes: 194268,
            modified: None,
            original_dimensions: (width as u32, height as u32),
            orientation: "NoTransforms".into(),
            has_alpha: false,
            icc_bytes: 0,
            exif: vec![
                ("Model".into(), "Example camera".into()),
                ("ExposureTime".into(), "1/125 s".into()),
            ],
            warnings: vec![],
        },
    }
}
fn state() -> State {
    let state = State::base(None);
    let asset = image();
    state.model.borrow_mut().paths = Rc::new(vec![asset.path.clone()]);
    state.model.borrow_mut().asset = Some(Arc::new(asset));
    state
}
#[derive(Default)]
struct Probe {
    labels: Vec<(String, Rect)>,
    images: Vec<(Rect, u64, ImageEffects)>,
    strokes: Vec<(Rect, Color, f32)>,
}
impl Painter for Probe {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, rect: Rect, color: Color, width: f32, _: f32) {
        self.strokes.push((rect, color, width));
    }
    fn fill_text(&mut self, rect: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
        self.labels.push((text.into(), rect));
    }
    fn draw_image_effects(
        &mut self,
        rect: Rect,
        image: &creamui_core::RgbaImage,
        _: Option<Color>,
        _: f32,
        effects: ImageEffects,
    ) {
        self.images.push((rect, image.id(), effects));
    }
}
fn scene(state: &State, size: Size) -> (Scene, Probe) {
    let mut probe = Probe::default();
    let scene = Renderer::new().render(state.build(size), size, &mut probe);
    (scene, probe)
}
fn click(state: &State, size: Size, label: &str) {
    let (scene, probe) = scene(state, size);
    let bounds = probe
        .labels
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing {label}: {:?}", probe.labels))
        .1;
    scene
        .hit_test(Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        })
        .unwrap_or_else(|| panic!("Not clickable: {label} at {bounds:?}"))();
}
fn native(state: &State, key: Key, modifiers: Modifiers) -> bool {
    state.key(
        KeyEvent {
            key,
            pressed: true,
            synthetic: false,
        },
        modifiers,
    )
}
fn capture(state: &State, size: Size, theme: Theme, name: &str) -> DisplayList {
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        theme.colors.surface,
        theme.colors,
    );
    Renderer::new().render(state.build(size), size, &mut recorder);
    let list = recorder.finish();
    if let Ok(directory) = std::env::var("CVIEWER_PREVIEW_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        let mut raster = Rasterizer::new(list.width, list.height);
        raster.render(&list, &Damage::Full);
        let first = &raster.pixmap().data()[..4];
        assert!(raster
            .pixmap()
            .data()
            .chunks_exact(4)
            .any(|pixel| pixel != first));
        raster
            .pixmap()
            .save_png(format!("{directory}/{name}.png"))
            .unwrap();
    }
    list
}
const NORMAL: Size = Size {
    width: 960.0,
    height: 680.0,
};
#[test]
fn pointer_zoom_pan_and_minimap_navigate_the_actual_scene() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let state = state();
        let (rendered, _) = scene(&state, NORMAL);
        assert!(rendered.first_text_input().is_none());
        click(&state, NORMAL, "1:1");
        let (rendered, _) = scene(&state, NORMAL);
        let rect = state.canvas.get();
        let pointer = Point {
            x: rect.width / 2.0,
            y: rect.y + rect.height / 2.0,
        };
        state.pointer_moved(pointer);
        let index = rendered.scroll_hit_test(pointer).unwrap();
        rendered.on_scroll_at(index).unwrap()(-180.0);
        assert!(matches!(state.model.borrow().view.zoom,Zoom::Scale(value) if value>1.0));
        let (rendered, _) = scene(&state, NORMAL);
        let (_, start) = rendered.drag_start_at(pointer).unwrap();
        let local = Point {
            x: pointer.x - rect.x,
            y: pointer.y - rect.y,
        };
        start(local, rect);
        let index = rendered.drag_hit_test(pointer).unwrap();
        let (_, drag) = rendered.draggable_at(index).unwrap();
        drag(
            Point {
                x: local.x + 100.0,
                y: local.y + 50.0,
            },
            rect,
        );
        let (_, map_probe) = scene(&state, NORMAL);
        assert!(map_probe
            .strokes
            .iter()
            .any(|(_, color, width)| *color == Theme::light().colors.accent && *width == 2.0));
        let pan = state.model.borrow().view.pan;
        assert!(pan.x > 0.0 && pan.y > 0.0);
        rendered.drag_end_at(index).unwrap()();
        let asset = state.model.borrow().asset.clone().unwrap();
        let (map, _) = state
            .model
            .borrow()
            .view
            .minimap(state.viewport(), asset.size())
            .unwrap();
        state.drag_start(Point {
            x: map.x + 2.0,
            y: map.y + 2.0,
        });
        state.drag_to(Point {
            x: map.x + map.width - 2.0,
            y: map.y + map.height - 2.0,
        });
        assert!(state.model.borrow().view.pan.x < 0.0);
        assert!(state.model.borrow().view.pan.y < 0.0);
        click(&state, NORMAL, "Fit");
        assert_eq!(state.model.borrow().view.zoom, Zoom::Fit);
        assert!(state
            .model
            .borrow()
            .view
            .minimap(state.viewport(), asset.size())
            .is_none());
    });
}
#[test]
fn filters_and_rotation_are_render_properties_that_keep_the_same_pixels() {
    creamui_reactive::with_context_scope(|| {
        let theme = Theme::dark();
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let state = state();
        let asset = state.model.borrow().asset.clone().unwrap();
        let identity = asset.data.image().id();
        let pixels = asset.data.pixels();
        state.run(Action::Brightness(1.4));
        state.run(Action::Saturation(0.5));
        state.run(Action::Grayscale);
        state.run(Action::Invert);
        state.run(Action::Rotate(true));
        capture(&state, NORMAL, theme, "filters-and-rotation");
        let (_, probe) = scene(&state, NORMAL);
        let (_, image_id, effects) = probe
            .images
            .iter()
            .find(|(_, id, _)| *id == identity)
            .unwrap();
        assert_eq!(*image_id, identity);
        assert_eq!(effects.quarter_turns, 1);
        assert_ne!(effects.color_matrix, ImageEffects::default().color_matrix);
        assert_eq!(&*asset.data.pixels(), &*pixels);
        state.run(Action::ResetAdjustments);
        assert_eq!(
            state.model.borrow().view.adjustments,
            Adjustments::default()
        );
        assert_eq!(state.model.borrow().view.quarter_turns, 1);
        state.run(Action::ResetView);
        assert_eq!(state.model.borrow().view.quarter_turns, 0);
    });
}
#[test]
fn keyboard_routing_works_without_a_ui_context_and_releases_are_ignored() {
    let state = state();
    native(&state, Key::Character("r".into()), Modifiers::default());
    assert_eq!(state.model.borrow().view.quarter_turns, 1);
    native(
        &state,
        Key::Character("R".into()),
        Modifiers {
            shift: true,
            ..Default::default()
        },
    );
    assert_eq!(state.model.borrow().view.quarter_turns, 0);
    native(&state, Key::Character("i".into()), Modifiers::default());
    assert_eq!(state.panel.peek(), Panel::Information);
    native(&state, Key::Escape, Modifiers::default());
    assert_eq!(state.panel.peek(), Panel::None);
    assert!(!native(&state, Key::Tab, Modifiers::default()));
    assert!(!state.key(
        KeyEvent {
            key: Key::Character("r".into()),
            pressed: false,
            synthetic: false
        },
        Modifiers::default()
    ));
    native(&state, Key::F1, Modifiers::default());
    for c in "invert".chars() {
        native(&state, Key::Character(c.to_string()), Modifiers::default());
    }
    assert_eq!(state.palette.matching(&state.catalog()).len(), 1);
    native(&state, Key::Enter, Modifiers::default());
    assert!(state.model.borrow().view.adjustments.invert);
    assert!(!state.palette_open.peek());
}
#[test]
fn obsolete_loads_do_not_replace_the_current_image_and_only_the_latest_request_is_queued() {
    let state = state();
    let identity = state
        .model
        .borrow()
        .asset
        .as_ref()
        .unwrap()
        .data
        .image()
        .id();
    state.generation.set(2);
    state.accept(
        1,
        Ok(Loaded {
            asset: image(),
            playlist: None,
        }),
    );
    assert_eq!(
        state
            .model
            .borrow()
            .asset
            .as_ref()
            .unwrap()
            .data
            .image()
            .id(),
        identity
    );
    state.accept(1, Err("old error".into()));
    assert!(state.model.borrow().error.is_none());
    state.busy.set(true);
    state.open_paths(vec![PathBuf::from("first.png")]);
    state.open_paths(vec![PathBuf::from("latest.png")]);
    let request = state.pending.borrow().as_ref().unwrap().clone();
    assert_eq!(request.path, Path::new("latest.png"));
    assert_eq!(request.generation, state.generation.get());
    state.run(Action::Close);
    assert!(state.pending.borrow().is_none());
    state.accept(
        request.generation,
        Ok(Loaded {
            asset: image(),
            playlist: None,
        }),
    );
    assert!(state.model.borrow().asset.is_none());
    assert!(!state.loading.peek());
}
#[test]
fn image_information_adjustments_and_native_navigation_have_consistent_controls() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let state = state();
        click(&state, NORMAL, "Info");
        scene(&state, NORMAL);
        state.panel_scroll.set(10000.0);
        let (_, probe) = scene(&state, NORMAL);
        assert!(probe
            .labels
            .iter()
            .any(|(label, _)| label == "Example camera"));
        assert!(probe
            .labels
            .iter()
            .any(|(label, _)| label == "ExposureTime"));
        click(&state, NORMAL, "Close");
        assert_eq!(state.panel.peek(), Panel::None);
        click(&state, NORMAL, "Adjust");
        let (_, probe) = scene(&state, NORMAL);
        assert!(probe.labels.iter().any(|(label, _)| label == "Brightness"));
        assert!(probe
            .labels
            .iter()
            .any(|(label, _)| label == "Black and white"));
        native(&state, Key::Escape, Modifiers::default());
        assert_eq!(state.panel.peek(), Panel::None);
        state.busy.set(true);
        state.model.borrow_mut().paths =
            Rc::new(vec!["one.png".into(), "two.png".into(), "three.png".into()]);
        native(&state, Key::Right, Modifiers::default());
        assert_eq!(state.model.borrow().index, 1);
        native(&state, Key::End, Modifiers::default());
        assert_eq!(state.model.borrow().index, 2);
        state.run(Action::Next);
        assert_eq!(state.model.borrow().index, 2);
        native(&state, Key::Home, Modifiers::default());
        assert_eq!(state.model.borrow().index, 0);
    });
}
#[test]
fn light_dark_compact_and_normal_scenes_show_images_empty_loading_errors_and_inspectors() {
    creamui_reactive::with_context_scope(|| {
        for (appearance, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            for (width, height) in [(960, 680), (640, 480), (1100, 760)] {
                let size = Size {
                    width: width as f32,
                    height: height as f32,
                };
                let state = state();
                for mode in [
                    "fit", "zoom", "rotated", "adjust", "info", "error", "loading", "empty",
                    "commands",
                ] {
                    state.panel.set(Panel::None);
                    state.palette_open.set(false);
                    state.loading.set(false);
                    {
                        let mut model = state.model.borrow_mut();
                        model.error = None;
                        model.view = View::default();
                    }
                    match mode {
                        "zoom" => state.run(Action::ActualSize),
                        "rotated" => state.run(Action::Rotate(true)),
                        "adjust" => {
                            state.run(Action::Panel(Panel::Adjustments));
                            state.run(Action::Brightness(1.2));
                        }
                        "info" => state.run(Action::Panel(Panel::Information)),
                        "commands" => state.run(Action::Commands),
                        "error" | "loading" | "empty" => {
                            state.model.borrow_mut().asset = None;
                            if mode == "error" {
                                state.model.borrow_mut().error = Some(
                                    "The image data is invalid or the file could not be read."
                                        .into(),
                                );
                            }
                            if mode == "loading" {
                                state.loading.set(true);
                            }
                        }
                        _ => {}
                    }
                    capture(&state, size, theme, &format!("{appearance}-{mode}-{width}"));
                    if matches!(mode, "fit" | "zoom" | "rotated" | "adjust" | "info") {
                        let source = state.model.borrow().asset.clone().unwrap();
                        let (_, probe) = scene(&state, size);
                        assert!(probe
                            .images
                            .iter()
                            .any(|(bounds, id, _)| *id == source.data.image().id()
                                && bounds.width > 0.0
                                && bounds.height > 0.0));
                    }
                    let (_, probe) = scene(&state, size);
                    for label in ["Open", "Adjust", "Info", "Fit", "1:1", "Rotate"] {
                        let rect = probe
                            .labels
                            .iter()
                            .find(|(text, _)| text == label)
                            .unwrap_or_else(|| panic!("Missing {label} in {mode}"))
                            .1;
                        assert!(
                            rect.x >= 0.0
                                && rect.y >= 0.0
                                && rect.x + rect.width <= size.width + 1.0
                                && rect.y + rect.height <= size.height + 1.0,
                            "{label}: {rect:?}"
                        );
                    }
                    if state.model.borrow().asset.is_none() {
                        state.model.borrow_mut().asset = Some(Arc::new(image()));
                    }
                }
            }
        }
    });
}
struct Controls(Rc<Cell<TitlebarState>>);
impl WindowControlsBackend for Controls {
    fn state(&self) -> TitlebarState {
        self.0.get()
    }
    fn request_titlebar(&self) {}
    fn minimize(&self) {}
    fn toggle_maximize(&self) {}
    fn close(&self) {}
}
#[test]
fn caption_controls_on_either_side_keep_app_controls_clear() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let controls = Rc::new(Cell::new(TitlebarState::default()));
        creamui_reactive::provide_context(WindowControls::new(Controls(controls.clone())));
        let state = state();
        for (width, height) in [(640.0, 480.0), (960.0, 680.0)] {
            let size = Size { width, height };
            for x in [8.0, width - 120.0] {
                controls.set(TitlebarState {
                    mode: TitlebarMode::Overlay,
                    overlay: Rect {
                        x,
                        y: 7.0,
                        width: 112.0,
                        height: 28.0,
                    },
                    ..Default::default()
                });
                let (rendered, probe) = scene(&state, size);
                for label in ["Open", "Adjust", "Info"] {
                    let bounds = probe
                        .labels
                        .iter()
                        .find(|(text, _)| text == label)
                        .unwrap()
                        .1;
                    assert!(bounds.x + bounds.width <= x || bounds.x >= x + 112.0);
                }
                for offset in (0..112).step_by(8) {
                    assert!(rendered
                        .hit_test(Point {
                            x: x + offset as f32,
                            y: 20.0
                        })
                        .is_none());
                }
            }
            controls.set(TitlebarState {
                mode: TitlebarMode::Client,
                ..Default::default()
            });
            scene(&state, size);
        }
    });
}

#[test]
fn modal_failures_shield_view_shortcuts_and_preserve_the_open_inspector() {
    let state = state();
    state.run(Action::Panel(Panel::Information));
    state.fail("Cannot open selection".into());
    assert!(!native(
        &state,
        Key::Character("r".into()),
        Modifiers::default()
    ));
    assert_eq!(state.model.borrow().view.quarter_turns, 0);
    native(&state, Key::Escape, Modifiers::default());
    assert!(state.model.borrow().error.is_none());
    assert_eq!(state.panel.peek(), Panel::Information);
}
#[test]
fn text_scaling_keeps_canvas_and_primary_controls_visible() {
    creamui_reactive::with_context_scope(|| {
        let theme = Theme {
            text_scale: 1.5,
            ..Theme::light()
        };
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let state = state();
        let size = Size {
            width: 640.0,
            height: 480.0,
        };
        state.run(Action::Panel(Panel::Adjustments));
        let (_, probe) = scene(&state, size);
        let bounds = state.canvas.get();
        assert!(bounds.width > 200.0 && bounds.height > 250.0);
        for label in ["Open", "Fit", "1:1", "Rotate"] {
            let bounds = probe
                .labels
                .iter()
                .find(|(text, _)| text == label)
                .unwrap()
                .1;
            assert!(
                bounds.x >= 0.0
                    && bounds.x + bounds.width <= size.width
                    && bounds.y + bounds.height <= size.height
            );
        }
        capture(&state, size, theme, "light-scaled-640");
    });
}
