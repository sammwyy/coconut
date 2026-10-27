use creamui_render::{AppHandle, WindowHandle};
use creamui_theme::Theme;
use creamui_theme_watch::ThemeListener;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

pub fn load_system_theme() -> Theme {
    match creamui_theme_loader::SystemThemeLoader::new().load() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance.theme
        }
        Err(error) => {
            eprintln!("terminal: failed to load CreamUI system theme: {error}");
            Theme::light()
        }
    }
}

pub fn watch_theme_reloads(
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
                    window.set_theme(load_system_theme());
                }
                watch_theme_reloads(next_app, next_listener, next_window);
            }
        },
    );
}
