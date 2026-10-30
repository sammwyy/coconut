use coconut_calculator::{
    app::{Action, State},
    appearance,
    navigation::Mode,
};
use creamui_render::{AppBuilder, RenderBackend, WindowOptions};
use creamui_theme_watch::ThemeListener;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), String> {
    let mut scientific = false;
    for argument in std::env::args().skip(1) {
        match argument.as_str() {
            "--help" | "-h" => {
                println!("Calculator\n\nUsage: ccalc [--scientific]\n\n  --scientific   Start in scientific mode\n  -h, --help     Show help\n  -V, --version  Show version\n\nF1 or Ctrl+Shift+P opens commands. Alt+1/2 changes calculator mode.\nCtrl+B opens the mode selector and unit converters.");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("Calculator {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--scientific" => scientific = true,
            _ => return Err(format!("Unknown option: {argument}")),
        }
    }
    let mode = if scientific {
        Mode::Scientific
    } else {
        Mode::Standard
    };
    let sizing = mode.sizing();
    let options = WindowOptions {
        title: "Calculator".into(),
        width: sizing.default.0,
        height: sizing.default.1,
        theme: appearance::load_system_theme(),
        decorations: true,
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    AppBuilder::new()
        .on_started(move |app| {
            let state = State::default();
            state.run(Action::Navigate(mode));
            let ready = state.clone();
            let build = state.clone();
            app.append_window(
                options,
                creamui_theme::Color::rgba(0, 0, 0, 0),
                move |window| {
                    *ready.window.borrow_mut() = Some(window.clone());
                    let keys = ready.clone();
                    window.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    ready.apply_window_size();
                },
                move |size| build.build(size),
            );
            appearance::watch_theme_reloads(
                app,
                Arc::new(Mutex::new(ThemeListener::start())),
                state.window.clone(),
            );
        })
        .run();
    Ok(())
}
