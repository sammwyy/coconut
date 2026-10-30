use coconut_image_viewer::{app::State, appearance};
use creamui_render::{AppBuilder, WindowOptions};
use creamui_theme_watch::ThemeListener;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
fn main() -> Result<(), String> {
    let mut paths = vec![];
    let mut positional = false;
    for argument in std::env::args_os().skip(1) {
        if !positional {
            match argument.to_str() {
                Some("--help" | "-h") => {
                    println!("Image Viewer\n\nUsage: cviewer [--] [IMAGE ...]\n\n  -h, --help     Show help\n  -V, --version  Show version\n\nOpen or drop images. Scroll to zoom, drag to pan.\nR rotates, E adjusts, I shows information. F1 shows commands.");
                    return Ok(());
                }
                Some("--version" | "-V") => {
                    println!("Image Viewer {}", env!("CARGO_PKG_VERSION"));
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
        title: "Image Viewer".into(),
        width: 960,
        height: 680,
        theme: appearance::load_system_theme(),
        decorations: true,
        ..Default::default()
    };
    AppBuilder::new()
        .on_started(move |app| {
            let state = State::new(app.clone());
            let ready = state.clone();
            let build = state.clone();
            app.append_window(
                options,
                creamui_theme::Color::rgba(0, 0, 0, 0),
                move |window| {
                    *ready.window.borrow_mut() = Some(window.clone());
                    window.set_min_size(Some((640, 480)));
                    let keys = ready.clone();
                    window.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    let pointer = ready.clone();
                    window.on_pointer_moved(move |point| pointer.pointer_moved(point));
                    let files = ready.clone();
                    window.on_files_dropped(move |paths| files.open_paths(paths));
                    ready.open_paths(paths);
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
