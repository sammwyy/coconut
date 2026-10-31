use crate::{browser::BrowserState, window::WindowRequest};
use creamui_render::{
    AppBuilder, AppHandle, BlurRegion, RenderBackend, WindowHandle, WindowOptions,
};
use creamui_theme::{Color, Theme};
use creamui_theme_watch::ThemeListener;
use std::{
    cell::RefCell,
    fs,
    rc::Rc,
    sync::{Arc, Mutex},
};

/// The manager and the portal render the same native browser tree.
pub fn show_window(request: WindowRequest) -> Option<crate::portal::PortalSelection> {
    ensure_wayland_display();
    let browser = BrowserState::new_deferred(request);
    let result = browser.result.clone();
    let window = browser.window.clone();
    let options = WindowOptions {
        title: browser.context.request.title.clone(),
        width: 1120,
        height: 760,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    AppBuilder::new()
        .on_started(move |app| {
            let ready_window = window.clone();
            let keys = browser.clone();
            let blur = browser.clone();
            let pending = browser.file_menu.clone();
            let loading = browser.clone();
            app.append_window(
                options,
                Color::rgba(0, 0, 0, 0),
                move |handle| {
                    handle.on_pointer_button(move |button, pressed, _| {
                        if pressed && button == creamui_render::platform::MouseButton::Left {
                            pending.cancel_pending();
                        }
                        false
                    });
                    handle.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    handle.on_focus_lost(move || {
                        blur.file_menu.parent_focus_lost();
                        blur.close_search_if_empty();
                    });
                    *ready_window.borrow_mut() = Some(handle);
                    loading.load_directories();
                },
                move |size| crate::views::build(&browser, size),
            );
            watch_theme_reloads(app, theme_events, window);
        })
        .run();
    let selected = result.lock().expect("browser result").take();
    selected
}

/// Opens the File Explorer preferences instead of a filesystem browser.
pub fn show_settings() {
    ensure_wayland_display();
    let settings = crate::settings::State::load();
    let window = settings.window.clone();
    let options = WindowOptions {
        title: "File Explorer Settings".into(),
        width: 760,
        height: 560,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    };
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    AppBuilder::new()
        .on_started(move |app| {
            let ready_window = window.clone();
            let keys = settings.clone();
            app.append_window(
                options,
                Color::rgba(0, 0, 0, 0),
                move |handle| {
                    handle.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    *ready_window.borrow_mut() = Some(handle);
                },
                move |size| crate::settings::build(&settings, size),
            );
            watch_theme_reloads(app, theme_events, window);
        })
        .run();
}

fn load_system_theme() -> Theme {
    match coconut_integration_cream::load_system_appearance() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance.theme
        }
        Err(error) => {
            eprintln!("files: failed to load CreamUI system theme: {error}");
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
                    window.set_theme(load_system_theme());
                }
                watch_theme_reloads(next_app, next_listener, next_window);
            }
        },
    );
}
fn ensure_wayland_display() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return;
    }
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return;
    };
    use std::os::unix::fs::FileTypeExt;
    let mut sockets = fs::read_dir(runtime)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_socket()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("wayland-"))
        .collect::<Vec<_>>();
    sockets.sort();
    if let Some(socket) = sockets.first() {
        std::env::set_var("WAYLAND_DISPLAY", socket);
    }
}
