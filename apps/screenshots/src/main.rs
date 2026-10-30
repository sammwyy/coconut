use coconut_screenshots::{app::State, appearance, capture, paths};
use creamui_render::{AppBuilder, RenderBackend, WindowOptions};
use creamui_theme_watch::ThemeListener;
use std::sync::{Arc, Mutex};

const HELP: &str = "Coconut Screenshots

Usage: coconut-screenshots [--screen] [--save-as]

  (no args)   Capture the screen and open the selection editor
  --screen    Capture the full screen without opening the editor
  --save-as   Ask where to save instead of the Screenshots folder
  -h, --help     Show help
  -V, --version  Show version

In the editor: drag to select, drag edges to resize, right-drag to move,
Ctrl+C copies, Ctrl+S saves, Ctrl+Shift+S saves as, Ctrl+A selects all,
Escape quits.";

fn main() -> Result<(), String> {
    let mut screen = false;
    let mut save_as = false;
    for argument in std::env::args_os().skip(1) {
        match argument.to_str() {
            Some("--screen") => screen = true,
            Some("--save-as") => save_as = true,
            Some("--help" | "-h") => {
                println!("{HELP}");
                return Ok(());
            }
            Some("--version" | "-V") => {
                println!("Coconut Screenshots {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ => return Err(format!("Unknown argument: {}", argument.to_string_lossy())),
        }
    }
    let shot = source()?;
    if screen || save_as {
        return headless(shot, save_as);
    }
    gui(shot)
}

/// `COCONUT_SCREENSHOTS_SOURCE` swaps the live capture for a PNG on disk —
/// lets the editor run where no capture backend exists (foreign compositors,
/// nested sessions, CI).
fn source() -> Result<coconut_screenshots::image::Shot, String> {
    let Some(path) = std::env::var_os("COCONUT_SCREENSHOTS_SOURCE") else {
        return capture::screen();
    };
    let decoded = image::open(&path)
        .map_err(|error| format!("could not open {}: {error}", path.to_string_lossy()))?
        .to_rgba8();
    let (width, height) = decoded.dimensions();
    coconut_screenshots::image::Shot::new(width, height, decoded.into_raw())
}

fn headless(shot: coconut_screenshots::image::Shot, save_as: bool) -> Result<(), String> {
    let target = if save_as {
        let suggested = paths::timestamped_target(&paths::screenshots_dir());
        let Some(path) = rfd::FileDialog::new()
            .set_title("Save screenshot")
            .set_directory(paths::screenshots_dir())
            .set_file_name(
                suggested
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )
            .save_file()
        else {
            return Ok(());
        };
        if path.extension().is_none() {
            path.with_extension("png")
        } else {
            path
        }
    } else {
        paths::timestamped_target(&paths::screenshots_dir())
    };
    shot.write_png(&target)?;
    println!("{}", target.display());
    Ok(())
}

fn gui(shot: coconut_screenshots::image::Shot) -> Result<(), String> {
    let options = WindowOptions {
        title: "Coconut Screenshots".into(),
        width: 960,
        height: 640,
        decorations: false,
        resizable: false,
        theme: appearance::load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    AppBuilder::new()
        .on_started(move |app| {
            let state = State::new(app.clone(), shot);
            let ready = state.clone();
            let build = state.clone();
            app.append_window(
                options,
                creamui_theme::Color::rgba(0, 0, 0, 255),
                move |window| {
                    *ready.window.borrow_mut() = Some(window.clone());
                    window.maximize();
                    let keys = ready.clone();
                    window.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    let pointer = ready.clone();
                    window.on_pointer_moved(move |point| pointer.pointer_moved(point));
                    let buttons = ready.clone();
                    window.on_pointer_button(move |button, pressed, point| {
                        buttons.pointer_button(button, pressed, point)
                    });
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
