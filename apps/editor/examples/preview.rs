const SAMPLE: &str = concat!(
    "# Coconut editor\n",
    "\n",
    "A place for **clear thinking** and `good code`.\n",
    "\n",
    "- [x] Fast editing\n",
    "- [x] Keyboard first\n",
    "- [ ] Write something new\n",
    "\n",
    "> Start with the words.\n",
    "\n",
    "## Everything within reach\n",
    "\n",
    "Find, replace, jump to a line, or switch to HEX.\n",
    "",
);

use coconut_editor::{
    app,
    commands::Command,
    document::{Language, ViewMode},
};
use creamui_core::{
    Point, Rect, Renderer, Scene, Size, TitlebarMode, TitlebarState, WindowControls,
    WindowControlsBackend,
};
use creamui_render::{
    platform::{Key, KeyEvent, Modifiers},
    AppBuilder, Damage, Rasterizer, SceneRecorder,
};
use creamui_theme::{Theme, ThemeProvider};
use std::{cell::Cell, path::Path, rc::Rc};

struct PreviewControls(Rc<Cell<TitlebarState>>);
impl WindowControlsBackend for PreviewControls {
    fn state(&self) -> TitlebarState {
        self.0.get()
    }
    fn request_titlebar(&self) {}
    fn minimize(&self) {}
    fn toggle_maximize(&self) {}
    fn close(&self) {}
}
fn capture(state: &app::State, size: Size, theme: Theme, path: &Path) -> Scene {
    let mut recorder = SceneRecorder::new();
    recorder.begin(
        size.width as u32,
        size.height as u32,
        1.0,
        theme.colors.surface,
        theme.colors,
    );
    let scene = Renderer::new().render(state.build(size), size, &mut recorder);
    let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
    raster.render(&recorder.finish(), &Damage::Full);
    raster.pixmap().save_png(path).unwrap();
    scene
}
fn escape(state: &app::State) {
    state.key(
        KeyEvent {
            key: Key::Escape,
            pressed: true,
            synthetic: false,
        },
        Modifiers::default(),
    );
}
#[derive(Default)]
struct Probe {
    labels: Vec<(String, Rect)>,
    fills: Vec<(Rect, creamui_theme::Color)>,
}
impl creamui_core::Painter for Probe {
    fn fill_rect(&mut self, rect: Rect, color: creamui_theme::Color, _: f32) {
        self.fills.push((rect, color));
    }
    fn stroke_rect(&mut self, _: Rect, _: creamui_theme::Color, _: f32, _: f32) {}
    fn fill_text(
        &mut self,
        rect: Rect,
        text: &str,
        _: creamui_theme::Color,
        _: f32,
        _: creamui_core::TextAlign,
    ) {
        self.labels.push((text.into(), rect));
    }
}
fn probe(state: &app::State, size: Size) -> Probe {
    let mut painter = Probe::default();
    Renderer::new().render(state.build(size), size, &mut painter);
    painter
}
fn center(painter: &Probe, label: &str) -> f32 {
    let rect = painter
        .labels
        .iter()
        .find(|(text, _)| text == label)
        .unwrap_or_else(|| panic!("Missing label {label}"))
        .1;
    rect.x + rect.width / 2.0
}
fn control_bounds(state: &app::State, size: Size, label: &str) -> (usize, usize) {
    let mut painter = Probe::default();
    let scene = Renderer::new().render(state.build(size), size, &mut painter);
    let x = center(&painter, label);
    let index = scene
        .focus_hit_test(Point {
            x,
            y: size.height - 16.0,
        })
        .unwrap();
    let points: Vec<_> = (0..size.width as usize)
        .filter(|x| {
            scene.focus_hit_test(Point {
                x: *x as f32 + 0.5,
                y: size.height - 16.0,
            }) == Some(index)
        })
        .collect();
    (*points.first().unwrap(), *points.last().unwrap())
}
fn verify_footer(state: &app::State, size: Size) {
    escape(state);
    state.run(Command::PlainText);
    let plain = probe(state, size);
    let kind_bounds = control_bounds(state, size, "Plain ▾");
    let encoding = center(&plain, "ASCII ▾");
    let encoding_bounds = control_bounds(state, size, "ASCII ▾");
    state.run(Command::Markdown);
    let markdown = probe(state, size);
    assert_eq!(control_bounds(state, size, "Markdown ▾"), kind_bounds);
    assert_eq!(control_bounds(state, size, "ASCII ▾"), encoding_bounds);
    let counter = markdown
        .labels
        .iter()
        .find(|(text, _)| text.starts_with("Ln "))
        .unwrap()
        .1;
    assert!(counter.x <= 12.0);
    let mut recorder = Probe::default();
    let scene = Renderer::new().render(state.build(size), size, &mut recorder);
    scene
        .hit_test(Point {
            x: encoding,
            y: size.height - 16.0,
        })
        .unwrap()();
    let menu = probe(state, size);
    assert!(!menu
        .labels
        .iter()
        .any(|(text, _)| text == "HEX" || text == "Binary"));
    escape(state);
    state.edit(|editor| editor.document.set_view(ViewMode::Hex));
    let bytes = probe(state, size);
    assert_eq!(control_bounds(state, size, "Bytes ▾"), kind_bounds);
    assert!(!bytes.labels.iter().any(|(text, _)| text == "ASCII ▾"));
    state.edit(|editor| editor.document.set_view(ViewMode::Ascii));
    state.run(Command::Hybrid);
}

fn palette_height(state: &app::State, size: Size, theme: Theme) -> f32 {
    probe(state, size)
        .fills
        .iter()
        .filter(|(rect, color)| {
            color.r == theme.colors.surface_elevated.r
                && color.g == theme.colors.surface_elevated.g
                && color.b == theme.colors.surface_elevated.b
                && rect.width >= 400.0
        })
        .map(|(rect, _)| rect.height)
        .reduce(f32::max)
        .unwrap()
}
fn verify_palette_size(state: &app::State, size: Size, theme: Theme) {
    let before = palette_height(state, size, theme);
    let mut painter = Probe::default();
    let scene = Renderer::new().render(state.build(size), size, &mut painter);
    let field = scene.first_text_input().unwrap();
    let handler = scene.on_key_at(field).unwrap();
    for character in "ascii".chars() {
        handler(creamui_core::KeyInput {
            key: creamui_core::Key::Char(character),
            modifiers: creamui_core::Modifiers::default(),
        });
    }
    let after = palette_height(state, size, theme);
    assert!(
        after < before,
        "palette heights {before} -> {after}; labels {:?}",
        probe(state, size)
            .labels
            .iter()
            .map(|(text, _)| text)
            .collect::<Vec<_>>()
    );
}
fn preview_theme(
    app: &creamui_render::AppHandle,
    directory: &Path,
    appearance: &str,
    theme: Theme,
) {
    creamui_reactive::provide_context(ThemeProvider::new(theme));
    let controls = Rc::new(Cell::new(TitlebarState::default()));
    creamui_reactive::provide_context(WindowControls::new(PreviewControls(controls.clone())));
    let state = app::State::new(app.clone());
    state.edit(|editor| {
        editor.insert(SAMPLE)?;
        editor.document.path = Some("Welcome.md".into());
        editor.document.language = Language::Markdown;
        editor.document.markdown_index = std::sync::Arc::new(
            coconut_editor::markdown::BlockIndex::build(&editor.document.text),
        );
        editor.document.markdown_revision = Some(editor.document.revision);
        editor.move_to(0, false);
        Ok(())
    });
    for command in [
        Command::Raw,
        Command::Hybrid,
        Command::Preview,
        Command::Find,
        Command::Palette,
        Command::Help,
        Command::GoTo,
        Command::Menu,
        Command::Close,
    ] {
        escape(&state);
        state.run(command);
        for (width, height) in [(1100, 760), (640, 480)] {
            capture(
                &state,
                Size {
                    width: width as f32,
                    height: height as f32,
                },
                theme,
                &directory.join(format!("{appearance}-{command:?}-{width}.png")),
            );
        }
    }
    escape(&state);
    state.run(Command::Palette);
    let size = Size {
        width: 1100.0,
        height: 760.0,
    };
    verify_palette_size(&state, size, theme);
    capture(
        &state,
        size,
        theme,
        &directory.join(format!("{appearance}-Palette-filtered.png")),
    );
    escape(&state);
    state.run(Command::Hybrid);
    state.edit(|editor| editor.document.set_view(ViewMode::Ascii));
    assert_eq!(
        state.current().1.borrow().document.language,
        Language::Markdown
    );
    for (width, height) in [(1100, 760), (640, 480)] {
        let size = Size {
            width: width as f32,
            height: height as f32,
        };
        let right = if width >= 900 { 76.0 } else { 0.0 };
        let encoding_x = width as f32 - 104.0 - right;
        let mode_x = encoding_x - 95.0;
        let render_x = mode_x - 90.0;
        for (name, x) in [
            ("render", render_x),
            ("mode", mode_x),
            ("encoding", encoding_x),
        ] {
            escape(&state);
            let scene = capture(
                &state,
                size,
                theme,
                &directory.join(format!("{appearance}-Markdown-ASCII-{width}.png")),
            );
            if let Some(click) = scene.hit_test(Point {
                x,
                y: height as f32 - 16.0,
            }) {
                click();
            }
            capture(
                &state,
                size,
                theme,
                &directory.join(format!("{appearance}-{name}-menu-{width}.png")),
            );
        }
    }
    verify_footer(
        &state,
        Size {
            width: 1100.0,
            height: 760.0,
        },
    );
    verify_footer(
        &state,
        Size {
            width: 640.0,
            height: 480.0,
        },
    );
    escape(&state);
    state.run(Command::New);
    for (width, height) in [(1100, 760), (640, 480)] {
        for (side, x) in [("left", 8.0), ("right", width as f32 - 120.0)] {
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
            let scene = capture(
                &state,
                Size {
                    width: width as f32,
                    height: height as f32,
                },
                theme,
                &directory.join(format!("{appearance}-tabs-{side}-{width}.png")),
            );
            for offset in (0..112).step_by(8) {
                assert!(scene
                    .hit_test(Point {
                        x: x + offset as f32,
                        y: 20.0
                    })
                    .is_none());
            }
        }
    }
    state.edit(|editor| {
        editor.document.set_view(ViewMode::Hex)?;
        Ok(())
    });
    controls.set(TitlebarState::default());
    state.editor_focused.set(true);
    state.key(
        KeyEvent {
            key: Key::Character("41".into()),
            pressed: true,
            synthetic: false,
        },
        Modifiers::default(),
    );
    assert_eq!(state.current().1.borrow().document.text.to_string(), "A");
    capture(
        &state,
        Size {
            width: 1100.0,
            height: 760.0,
        },
        theme,
        &directory.join(format!("{appearance}-paired-HEX.png")),
    );
    state.edit(|editor| editor.document.set_view(ViewMode::Binary));
    state.key(
        KeyEvent {
            key: Key::Character("01000010".into()),
            pressed: true,
            synthetic: false,
        },
        Modifiers::default(),
    );
    assert_eq!(state.current().1.borrow().document.text.to_string(), "AB");
}
fn main() {
    let directory = std::env::args_os()
        .nth(1)
        .map(std::path::PathBuf::from)
        .expect("Usage: preview OUTPUT_DIRECTORY");
    std::fs::create_dir_all(&directory).unwrap();
    AppBuilder::new()
        .on_started(move |app| {
            for (appearance, theme) in [("dark", Theme::dark()), ("light", Theme::light())] {
                creamui_reactive::with_context_scope(|| {
                    preview_theme(&app, &directory, appearance, theme)
                });
            }
            app.exit();
        })
        .run();
}
