use crate::config::Config;
use creamui_render::platform::Key;
use creamui_render::{AppBuilder, WindowOptions};
use creamui_theme::Color;
use creamui_theme_watch::ThemeListener;
use std::sync::{Arc, Mutex};

/// Opens the player preferences in their own window (`cplayer --settings`).
pub fn show_settings() {
    crate::backends::init_env();
    let settings = crate::settings::State::load();
    let window = settings.window.clone();
    let options = WindowOptions {
        title: "Media Player Settings".into(),
        width: 780,
        height: 600,
        transparent: false,
        decorations: true,
        blur: None,
        backend: Config::load().interface.renderer.backend(),
        theme: super::load_theme(),
        ..Default::default()
    };
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    AppBuilder::new()
        .on_started(move |app| {
            let ready_window = window.clone();
            settings.detect_backends(app.clone());
            app.append_window(
                options,
                Color::rgb(0, 0, 0),
                move |handle| {
                    let closer = handle.clone();
                    handle.on_keyboard_input(move |event, _| {
                        let escape = event.pressed && event.key == Key::Escape;
                        if escape {
                            closer.close();
                        }
                        escape
                    });
                    *ready_window.borrow_mut() = Some(handle);
                },
                move |size| crate::settings::build(&settings, size),
            );
            super::watch_theme_reloads(app, theme_events, window);
        })
        .run();
}

/// Spawns the settings window as a separate instance of this binary.
pub fn open_settings() {
    match std::env::current_exe()
        .and_then(|exe| std::process::Command::new(exe).arg("--settings").spawn())
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("cplayer: couldn't open settings: {error}"),
    }
}
