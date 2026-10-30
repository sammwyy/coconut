use crate::config::Config;
use creamui_render::{AppHandle, WindowHandle};
use creamui_theme::{Color, Theme};
use creamui_theme_watch::ThemeListener;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[cfg(unix)]
mod ipc;
pub(crate) mod player;
mod settings;

pub use player::run;
pub use settings::{open_settings, show_settings};

#[cfg(unix)]
pub fn quit() -> Result<(), String> {
    ipc::send(ipc::Request::Quit).map_err(|error| format!("player is not running: {error}"))
}

#[cfg(not(unix))]
pub fn quit() -> Result<(), String> {
    Err("--quit is not supported on this platform".into())
}

/// The system theme with the user's accent preference applied.
pub(crate) fn load_theme() -> Theme {
    theme_for(None)
}

/// Like [`load_theme`], with `media` as the automatic accent.
pub(crate) fn theme_for(media: Option<Color>) -> Theme {
    let mut theme = Config::load()
        .appearance
        .accent
        .apply(load_system_theme(), media);
    let colors = &mut theme.colors;
    colors.surface.a = 255;
    let surface = colors.surface;
    for color in [
        &mut colors.surface_elevated,
        &mut colors.surface_hover,
        &mut colors.accent,
        &mut colors.accent_hover,
        &mut colors.accent_pressed,
        &mut colors.selection_background,
        &mut colors.selection_text,
        &mut colors.text_primary,
        &mut colors.text_secondary,
        &mut colors.text_disabled,
        &mut colors.border,
        &mut colors.border_strong,
        &mut colors.danger,
        &mut colors.warning,
        &mut colors.success,
    ] {
        let a = color.a as f32 / 255.0;
        *color = Color::rgb(
            (color.r as f32 * a + surface.r as f32 * (1.0 - a)).round() as u8,
            (color.g as f32 * a + surface.g as f32 * (1.0 - a)).round() as u8,
            (color.b as f32 * a + surface.b as f32 * (1.0 - a)).round() as u8,
        );
    }
    theme
}

pub(crate) fn load_system_theme() -> Theme {
    match coconut_integration_cream::load_system_appearance() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance.theme
        }
        Err(error) => {
            eprintln!("cplayer: failed to load CreamUI system theme: {error}");
            Theme::light()
        }
    }
}

fn watch_theme_reloads(
    app: AppHandle,
    listener: Arc<Mutex<ThemeListener>>,
    window: Rc<RefCell<Option<WindowHandle>>>,
) {
    let next_app = app.clone();
    let next_listener = listener.clone();
    let next_window = window.clone();
    app.spawn_background(
        move || listener.lock().is_ok_and(|listener| listener.recv()),
        move |changed| {
            if changed {
                if let Some(window) = window.borrow().as_ref() {
                    window.set_theme(load_theme());
                }
                watch_theme_reloads(next_app, next_listener, next_window);
            }
        },
    );
}
