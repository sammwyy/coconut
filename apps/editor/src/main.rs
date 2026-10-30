use coconut_editor::{app, appearance};
use creamui_render::{AppBuilder, BlurRegion, RenderBackend, WindowOptions};
use creamui_theme_watch::ThemeListener;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
fn main() -> Result<(), String> {
    let mut paths = Vec::new();
    let mut positional = false;
    for argument in std::env::args_os().skip(1) {
        if !positional {
            match argument.to_str() {
                Some("--help" | "-h") => {
                    println!("CEdit — Coconut text, code, Markdown and byte editor\n\nUsage: cedit [--] [FILE ...]\n\n  -h, --help     Show help\n  -V, --version  Show version\n\nPress Ctrl+Shift+P for commands or F1 for keyboard shortcuts.");
                    return Ok(());
                }
                Some("--version" | "-V") => {
                    println!("CEdit {}", env!("CARGO_PKG_VERSION"));
                    return Ok(());
                }
                Some("--") => {
                    positional = true;
                    continue;
                }
                Some(value) if value.starts_with('-') => {
                    return Err(format!("Unknown option: {value}"))
                }
                _ => {}
            }
        }
        paths.push(PathBuf::from(argument));
    }
    let options = WindowOptions {
        title: "Untitled - CEdit".into(),
        width: 1100,
        height: 760,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: appearance::load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    AppBuilder::new()
        .on_started(move |app| {
            let state = app::State::new(app.clone());
            let ready = state.clone();
            let build = state.clone();
            app.append_window(
                options,
                creamui_theme::Color::rgba(0, 0, 0, 0),
                move |window| {
                    *ready.window.borrow_mut() = Some(window.clone());
                    let keys = ready.clone();
                    window.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    let close = ready.clone();
                    window.on_close_requested(move || close.request_quit());
                    let files = ready.clone();
                    window.on_files_dropped(move |paths| {
                        for path in paths {
                            files.open(path);
                        }
                    });
                    window.focus_text_input();
                    for path in paths {
                        ready.open(path);
                    }
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
