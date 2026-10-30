use creamui_theme::Theme;

pub fn load_system_theme() -> Theme {
    match coconut_integration_cream::load_system_appearance() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance.theme
        }
        Err(error) => {
            eprintln!("coconut-permission-portal: failed to load CreamUI system theme: {error}");
            Theme::light()
        }
    }
}

pub fn watch_theme_reloads(
    app: creamui_render::AppHandle,
    listener: std::sync::Arc<std::sync::Mutex<creamui_theme_watch::ThemeListener>>,
    window: std::rc::Rc<std::cell::RefCell<Option<creamui_render::WindowHandle>>>,
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
