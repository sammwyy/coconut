use super::*;
use creamui_core::{
    Painter, Point, Rect, Renderer, Scene, TextAlign, TitlebarMode, TitlebarState, WindowControls,
    WindowControlsBackend,
};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, Theme, ThemeProvider};

#[derive(Default)]
struct Probe {
    labels: Vec<(String, Rect)>,
}
impl Painter for Probe {
    fn fill_rect(&mut self, _: Rect, _: Color, _: f32) {}
    fn stroke_rect(&mut self, _: Rect, _: Color, _: f32, _: f32) {}
    fn fill_text(&mut self, rect: Rect, text: &str, _: Color, _: f32, _: TextAlign) {
        self.labels.push((text.into(), rect));
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
        .filter(|(text, _)| text == label)
        .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
        .unwrap_or_else(|| panic!("Missing {label}: {:?}", probe.labels))
        .1;
    scene
        .hit_test(Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        })
        .unwrap_or_else(|| panic!("Not clickable: {label} {bounds:?}"))();
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
fn type_text(state: &State, text: &str) {
    for c in text.chars() {
        assert!(
            native(state, Key::Character(c.to_string()), Modifiers::default()),
            "{c}"
        );
    }
}
fn size(mode: Mode) -> Size {
    let (width, height) = mode.sizing().default;
    Size {
        width: width as f32,
        height: height as f32,
    }
}
fn save(state: &State, size: Size, theme: Theme, name: &str) {
    if let Ok(directory) = std::env::var("CCALC_PREVIEW_DIR") {
        let mut recorder = SceneRecorder::new();
        recorder.begin(
            size.width as u32,
            size.height as u32,
            1.0,
            theme.colors.surface,
            theme.colors,
        );
        Renderer::new().render(state.build(size), size, &mut recorder);
        let mut raster = Rasterizer::new(size.width as u32, size.height as u32);
        raster.render(&recorder.finish(), &Damage::Full);
        let pixels = raster.pixmap().data();
        assert!(pixels.chunks_exact(4).any(|pixel| pixel != &pixels[..4]));
        std::fs::create_dir_all(&directory).unwrap();
        raster
            .pixmap()
            .save_png(format!("{directory}/{name}.png"))
            .unwrap();
    }
}
#[test]
fn keyboard_entry_works_without_a_focusable_input_before_and_after_pointer_actions() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let state = State::default();
        let size = size(Mode::Standard);
        assert!(scene(&state, size).0.first_text_input().is_none());
        type_text(&state, "6*7");
        assert!(native(&state, Key::Enter, Modifiers::default()));
        assert_eq!(state.entry().text, "42");
        click(&state, size, "+");
        type_text(&state, "8");
        native(&state, Key::Enter, Modifiers::default());
        assert_eq!(state.entry().text, "50");
        type_text(&state, "3");
        assert_eq!(state.entry().text, "3");
        click(&state, size, "1/x");
        click(&state, size, "=");
        assert_eq!(state.entry().text, (1.0 / 3.0_f64).to_string());
        assert!(!native(&state, Key::Tab, Modifiers::default()));
        assert!(!state.key(
            KeyEvent {
                key: Key::Backspace,
                pressed: false,
                synthetic: false
            },
            Modifiers::default()
        ));
        state.set_expression("2×π");
        native(&state, Key::Backspace, Modifiers::default());
        assert_eq!(state.entry().text, "2×");
        native(&state, Key::Escape, Modifiers::default());
        assert_eq!(state.entry().text, "");
    });
}
#[test]
fn clear_all_resets_entry_memory_and_answer_but_preserves_history() {
    let state = State::default();
    state.set_expression("6*7");
    state.run(Action::Evaluate);
    state.run(Action::Memory(Memory::Store));
    assert_eq!(state.session.borrow().memory, Some(42.0));
    assert_eq!(state.session.borrow().context.answer, 42.0);
    state.set_expression("1+1");
    state.run(Action::ClearAll);
    assert_eq!(state.entry().text, "");
    assert_eq!(state.session.borrow().memory, None);
    assert_eq!(state.session.borrow().context.answer, 0.0);
    assert_eq!(state.session.borrow().context.memory, 0.0);
    assert_eq!(state.session.borrow().history.len(), 1);
    state.run(Action::Memory(Memory::Store));
    assert_eq!(state.session.borrow().memory, Some(0.0));
    state.run(Action::Clear);
    assert_eq!(state.session.borrow().memory, Some(0.0));
    assert!(native(
        &state,
        Key::Delete,
        Modifiers {
            shift: true,
            ..Default::default()
        },
    ));
    assert_eq!(state.session.borrow().memory, None);
}
#[test]
fn router_keeps_calculator_registers_and_each_converter_session() {
    let state = State::default();
    type_text(&state, "42");
    state.run(Action::Memory(Memory::Store));
    state.run(Action::Navigate(Mode::Scientific));
    assert_eq!(state.entry().text, "42");
    state.run(Action::Angle(Angle::Radians));
    state.run(Action::Navigate(Mode::Converter(Category::Length)));
    type_text(&state, "100");
    state.run(Action::Unit {
        source: false,
        index: 6,
    });
    let length = state.result().unwrap();
    state.run(Action::Navigate(Mode::Converter(Category::Temperature)));
    type_text(&state, "100");
    assert!((state.result().unwrap() - 212.0).abs() < 1e-10);
    state.run(Action::Evaluate);
    assert!(state
        .session
        .borrow()
        .history
        .front()
        .unwrap()
        .detail
        .contains("°C → °F"));
    state.run(Action::Navigate(Mode::Converter(Category::Length)));
    assert_eq!(state.entry().text, "100");
    assert_eq!(state.selected_units(Category::Length).1, 6);
    assert_eq!(state.result().unwrap(), length);
    state.run(Action::Navigate(Mode::Standard));
    assert_eq!(state.entry().text, "42");
    assert_eq!(state.session.borrow().memory, Some(42.0));
    assert_eq!(state.session.borrow().context.angle, Angle::Radians);
    assert_eq!(state.router.location().pathname, "/calculator/standard");
}
#[test]
fn conversions_swap_validate_and_share_history_without_corrupting_entry() {
    let state = State::default();
    state.run(Action::Navigate(Mode::Converter(Category::Temperature)));
    state.set_expression("100");
    state.run(Action::Evaluate);
    assert_eq!(state.entry().text, "100");
    assert!(state.entry().fresh);
    type_text(&state, "32");
    assert_eq!(state.entry().text, "32");
    state.run(Action::SwapUnits);
    assert!((state.result().unwrap() - 32.0).abs() < 1e-10);
    state.run(Action::Evaluate);
    assert_eq!(state.session.borrow().history.len(), 2);
    state.run(Action::Unit {
        source: true,
        index: 2,
    });
    state.set_expression("-1");
    state.run(Action::Evaluate);
    assert!(state.result().unwrap_err().contains("absolute zero"));
    assert_eq!(state.session.borrow().history.len(), 2);
    state.run(Action::Clear);
    type_text(&state, "273.15");
    assert!(state.message.peek().is_empty());
    assert!(state.result().is_ok());
    let (from, _) = &state.units[&Category::Temperature];
    from.set_open(true);
    assert!(!native(
        &state,
        Key::Character("9".into()),
        Modifiers::default()
    ));
    assert_eq!(state.entry().text, "273.15");
    from.set_open(false);
}
#[test]
fn command_keyboard_routing_stays_independent_of_theme_and_display_focus() {
    let state = State::default();
    type_text(&state, "42");
    native(&state, Key::F1, Modifiers::default());
    for c in "scientific".chars() {
        native(&state, Key::Character(c.to_string()), Modifiers::default());
    }
    assert_eq!(state.palette.matching(&state.catalog()).len(), 1);
    native(&state, Key::Enter, Modifiers::default());
    assert_eq!(state.mode(), Mode::Scientific);
    assert_eq!(state.entry().text, "42");
    native(
        &state,
        Key::Character("1".into()),
        Modifiers {
            alt: true,
            ..Default::default()
        },
    );
    assert_eq!(state.mode(), Mode::Standard);
    native(
        &state,
        Key::Character("b".into()),
        Modifiers {
            ctrl: true,
            ..Default::default()
        },
    );
    assert!(state.drawer_open.peek());
    native(&state, Key::F1, Modifiers::default());
    native(&state, Key::Escape, Modifiers::default());
    assert!(!state.palette_open.peek());
}
#[test]
fn drawer_navigation_and_history_reuse_work_with_pointer_and_no_text_input() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::dark()));
        let state = State::default();
        let size = size(Mode::Standard);
        assert!(!state.drawer_open.peek());
        state.run(Action::Drawer);
        click(&state, size, "Scientific");
        assert_eq!(state.mode(), Mode::Scientific);
        assert_eq!(state.router.location().pathname, "/calculator/scientific");
        state.run(Action::Drawer);
        click(&state, size, "Length");
        assert_eq!(state.mode(), Mode::Converter(Category::Length));
        state.set_expression("1");
        state.run(Action::Evaluate);
        state.run(Action::Navigate(Mode::Standard));
        state.set_expression("6*7");
        state.run(Action::Evaluate);
        click(&state, size, "History");
        click(&state, size, "= 42");
        assert_eq!(state.entry().text, "42");
        click(&state, size, "Clear history");
        assert!(state.session.borrow().history.is_empty());
        native(&state, Key::Escape, Modifiers::default());
        assert!(!state.history_open.peek());
        assert!(scene(&state, size).0.first_text_input().is_none());
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
fn every_route_renders_in_both_themes_with_a_shared_hud_and_visible_keypad() {
    creamui_reactive::with_context_scope(|| {
        let state = State::default();
        for (appearance, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            for mode in [Mode::Standard, Mode::Scientific]
                .into_iter()
                .chain(Category::ALL.into_iter().map(Mode::Converter))
            {
                state.run(Action::Navigate(mode));
                state.set_expression(if mode == Mode::Scientific {
                    "sin(30)^2+cos(30)^2"
                } else {
                    "100"
                });
                let default = size(mode);
                let minimum = mode.sizing().minimum;
                for (name, size) in [
                    ("default", default),
                    (
                        "compact",
                        Size {
                            width: minimum.0 as f32,
                            height: minimum.1 as f32,
                        },
                    ),
                ] {
                    let (rendered, probe) = scene(&state, size);
                    assert!(rendered.first_text_input().is_none());
                    assert!(!probe
                        .labels
                        .iter()
                        .any(|(label, _)| label == "CALCULATOR" || label == "CONVERTER"));
                    assert!(!probe.labels.iter().any(|(label, _)| label == "Menu"));
                    for label in ["7", "=", "Copy", "History"] {
                        let bounds = probe
                            .labels
                            .iter()
                            .filter(|(text, _)| text == label)
                            .max_by(|a, b| a.1.y.total_cmp(&b.1.y))
                            .unwrap_or_else(|| panic!("Missing {label} in {mode:?}"))
                            .1;
                        assert!(
                            bounds.x >= 0.0
                                && bounds.y > 42.0
                                && bounds.x + bounds.width <= size.width + 0.5
                                && bounds.y + bounds.height <= size.height + 0.5,
                            "{mode:?} {name} {label}: {bounds:?}"
                        );
                    }
                    save(
                        &state,
                        size,
                        theme,
                        &format!("{appearance}-{}-{name}", mode.path().replace('/', "-")),
                    );
                }
            }
        }
    });
}
#[test]
fn titlebar_respects_compositor_controls_and_drawer_trigger_keeps_its_bounds() {
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(Theme::light()));
        let controls = Rc::new(Cell::new(TitlebarState::default()));
        creamui_reactive::provide_context(WindowControls::new(Controls(controls.clone())));
        let state = State::default();
        for (width, height) in [
            (320.0, 480.0),
            (360.0, 520.0),
            (720.0, 560.0),
            (960.0, 640.0),
        ] {
            let size = Size { width, height };
            for mode in [TitlebarMode::Client, TitlebarMode::Overlay] {
                for x in [8.0, size.width - 120.0] {
                    controls.set(TitlebarState {
                        mode,
                        overlay: Rect {
                            x,
                            y: 7.0,
                            width: 112.0,
                            height: 28.0,
                        },
                        ..Default::default()
                    });
                    for page in [
                        Mode::Standard,
                        Mode::Scientific,
                        Mode::Converter(Category::Temperature),
                    ] {
                        state.run(Action::Navigate(page));
                        let (scene, probe) = scene(&state, size);
                        let label = format!("{}  ▾", page.name());
                        let rect = probe
                            .labels
                            .iter()
                            .find(|(text, _)| text == &label)
                            .unwrap()
                            .1;
                        if mode == TitlebarMode::Overlay {
                            assert!(rect.x + rect.width <= x || rect.x >= x + 112.0);
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
                }
            }
        }
    });
}
#[test]
fn shared_history_errors_palette_and_scaled_ui_render_and_dismiss() {
    creamui_reactive::with_context_scope(|| {
        let state = State::default();
        let size = Size {
            width: 960.0,
            height: 680.0,
        };
        for (appearance, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            creamui_reactive::provide_context(ThemeProvider::new(theme));
            state.run(Action::Navigate(Mode::Scientific));
            state.set_expression("sqrt(-1)");
            save(&state, size, theme, &format!("{appearance}-error"));
            state.set_expression("6*7");
            state.run(Action::Evaluate);
            state.run(Action::History);
            save(&state, size, theme, &format!("{appearance}-history"));
            state.run(Action::Commands);
            save(&state, size, theme, &format!("{appearance}-commands"));
            state.palette.query().set_value("no-such-command");
            let (_, probe) = scene(&state, size);
            assert!(probe
                .labels
                .iter()
                .any(|(label, _)| label == "No matching commands"));
            save(&state, size, theme, &format!("{appearance}-empty-commands"));
            native(&state, Key::Escape, Modifiers::default());
            assert!(!state.palette_open.peek());
            assert!(state.history_open.peek());
            native(&state, Key::Escape, Modifiers::default());
            assert!(!state.history_open.peek());
            type_text(&state, "8");
            assert_eq!(state.entry().text, "8");
        }
        let theme = Theme {
            text_scale: 1.5,
            ..Theme::light()
        };
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        scene(&state, size);
        state.scroll.set(10000.0);
        click(&state, size, "=");
        assert_eq!(state.entry().text, "8");
        save(&state, size, theme, "scaled-scientific");
    });
}

#[test]
fn navigation_overlay_starts_hidden_preserves_layout_and_dismisses_without_calculating() {
    creamui_reactive::with_context_scope(|| {
        let theme = Theme::light();
        creamui_reactive::provide_context(ThemeProvider::new(theme));
        let state = State::default();
        let size = size(Mode::Standard);
        assert!(!state.drawer_open.peek());
        let (_, before) = scene(&state, size);
        let numeric = before
            .labels
            .iter()
            .find(|(text, _)| text == "7")
            .unwrap()
            .1;
        state.run(Action::Drawer);
        let (opened, probe) = scene(&state, size);
        assert!(probe.labels.iter().any(|(text, _)| text == "CALCULATOR"));
        assert!(probe.labels.iter().any(|(text, _)| text == "CONVERTER"));
        assert_eq!(
            numeric,
            probe.labels.iter().find(|(text, _)| text == "7").unwrap().1
        );
        save(&state, size, theme, "light-navigation-overlay");
        opened
            .hit_test(Point {
                x: size.width - 10.0,
                y: size.height - 10.0,
            })
            .unwrap()();
        assert!(!state.drawer_open.peek());
        assert!(state.session.borrow().history.is_empty());
        state.run(Action::Drawer);
        type_text(&state, "8");
        assert!(!state.drawer_open.peek());
        assert_eq!(state.entry().text, "8");
        state.run(Action::Drawer);
        native(&state, Key::Escape, Modifiers::default());
        assert!(!state.drawer_open.peek());
        assert_eq!(state.entry().text, "8");
    });
}
