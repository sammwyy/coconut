//! Renders the editor overlay through the real widget pipeline so selection,
//! HUD layout and annotation compositing get checked without a compositor.
#![cfg(test)]

use super::{hud::Tool, ResizeHandle, State};
use crate::image::Shot;
use creamui_core::{Point, Rect, Renderer, Size};
use creamui_render::{Damage, DisplayList, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};

fn shot() -> Shot {
    let (width, height) = (160u32, 120u32);
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize * 4;
            pixels[index] = ((x * 255) / width) as u8;
            pixels[index + 1] = ((y * 255) / height) as u8;
            pixels[index + 2] = 96;
            pixels[index + 3] = 255;
        }
    }
    Shot::new(width, height, pixels).unwrap()
}

fn frame(renderer: &mut Renderer, state: &State, theme: Theme, size: Size) -> DisplayList {
    creamui_reactive::provide_context(ThemeProvider::new(theme));
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        Color::rgba(0, 0, 0, 255),
        theme.colors,
    );
    renderer.render(state.build(size), size, &mut recorder);
    let list = recorder.finish();
    save("frame", &list);
    list
}

fn save(name: &str, list: &DisplayList) {
    if let Ok(directory) = std::env::var("SCREENSHOTS_PREVIEW_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        let mut raster = Rasterizer::new(list.width, list.height);
        raster.render(list, &Damage::Full);
        raster
            .pixmap()
            .save_png(format!("{directory}/{name}.png"))
            .unwrap();
    }
}

fn size(width: f32, height: f32) -> Size {
    Size { width, height }
}

fn drag(state: &State, from: (f32, f32), to: (f32, f32)) {
    state.drag_start_at(Point {
        x: from.0,
        y: from.1,
    });
    state.drag_to_at(Point { x: to.0, y: to.1 });
    state.drag_finished();
}

#[test]
fn overlay_renders_at_every_size_and_theme() {
    creamui_reactive::with_context_scope(|| {
        let state = State::base(None, shot());
        let mut renderer = Renderer::new();
        for (theme, size) in [
            (Theme::dark(), size(1280.0, 800.0)),
            (Theme::light(), size(1280.0, 800.0)),
            (Theme::dark(), size(640.0, 480.0)),
        ] {
            let list = frame(&mut renderer, &state, theme, size);
            assert!(list.width > 0 && !list.items.is_empty());
        }
    });
}

#[test]
fn left_drag_creates_and_resizes_a_selection() {
    creamui_reactive::with_context_scope(|| {
        let state = State::base(None, shot());
        let mut renderer = Renderer::new();
        let size = size(640.0, 480.0);
        frame(&mut renderer, &state, Theme::dark(), size);
        drag(&state, (160.0, 120.0), (480.0, 360.0));
        {
            let model = state.model();
            let selection = model.selection.expect("a drag leaves a selection");
            assert!((selection.width - 80.0).abs() < 1.0);
            assert!((selection.height - 60.0).abs() < 1.0);
        }
        frame(&mut renderer, &state, Theme::dark(), size);
        let model = state.model();
        let view = state.selection_view().unwrap();
        let corner = ResizeHandle::Se.anchor(view);
        drop(model);
        drag(
            &state,
            (corner.x, corner.y),
            (corner.x + 40.0, corner.y - 20.0),
        );
        let model = state.model();
        let selection = model.selection.unwrap();
        assert!((selection.width - 90.0).abs() < 1.0);
        assert!((selection.height - 55.0).abs() < 1.0);
    });
}

#[test]
fn right_drag_moves_the_selection_and_annotations_stay_erasable() {
    creamui_reactive::with_context_scope(|| {
        let state = State::base(None, shot());
        let mut renderer = Renderer::new();
        let size = size(640.0, 480.0);
        frame(&mut renderer, &state, Theme::dark(), size);
        drag(&state, (160.0, 120.0), (480.0, 360.0));
        let before = state.model().selection.unwrap();
        state.pointer_button(
            creamui_render::platform::MouseButton::Right,
            true,
            Point { x: 300.0, y: 200.0 },
        );
        state.pointer_moved(Point { x: 340.0, y: 240.0 });
        state.pointer_button(
            creamui_render::platform::MouseButton::Right,
            false,
            Point { x: 340.0, y: 240.0 },
        );
        let after = state.model().selection.unwrap();
        assert!((after.x - before.x - 10.0).abs() < 1.0);
        assert!((after.y - before.y - 10.0).abs() < 1.0);

        {
            let mut model = state.model_mut();
            model.tool = Tool::Brush;
            drop(model);
        }
        drag(&state, (200.0, 150.0), (280.0, 200.0));
        assert!(!state.model().annotations.is_empty());
        assert!(state.model().annotation_image().is_some());
        frame(&mut renderer, &state, Theme::dark(), size);

        {
            let mut model = state.model_mut();
            model.tool = Tool::Eraser;
        }
        drag(&state, (240.0, 175.0), (241.0, 175.0));
        let model = state.model();
        let flat = model
            .shot
            .flattened(
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 160.0,
                    height: 120.0,
                },
                Some(&model.annotations),
            )
            .unwrap();
        assert_eq!(flat.pixels.len(), (160 * 120 * 4) as usize);
    });
}

#[test]
fn hud_hides_handles_and_positions_around_the_selection() {
    creamui_reactive::with_context_scope(|| {
        let state = State::base(None, shot());
        let mut renderer = Renderer::new();
        let size = size(640.0, 480.0);
        frame(&mut renderer, &state, Theme::dark(), size);
        state.select_all();
        frame(&mut renderer, &state, Theme::dark(), size);
        let selection = state.model().selection.unwrap();
        assert_eq!(selection.width, 160.0);
        assert_eq!(selection.height, 120.0);
    });
}
