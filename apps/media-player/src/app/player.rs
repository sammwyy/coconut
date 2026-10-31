use crate::config::Config;
use crate::state::PlayerState;
#[cfg(target_os = "linux")]
use creamui_image::{ImageData, SvgSize};
use creamui_render::{AppBuilder, WindowOptions};
#[cfg(target_os = "linux")]
use creamui_render::{TrayBuilder, TrayIcon};
use creamui_theme::Color;
use creamui_theme_watch::ThemeListener;
#[cfg(target_os = "linux")]
use std::cell::RefCell;
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn run(paths: Vec<PathBuf>) {
    crate::backends::init_env();
    #[cfg(unix)]
    let listener = match super::ipc::start(&paths) {
        Ok(Some(listener)) => listener,
        Ok(None) => return,
        Err(error) => {
            eprintln!("cplayer: player IPC unavailable: {error}");
            return;
        }
    };
    #[cfg(unix)]
    let cleanup = listener.clone();
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    let background = Config::load().interface.background_playback;
    let builder = AppBuilder::new();
    #[cfg(target_os = "linux")]
    let builder = if background {
        builder.keep_running()
    } else {
        builder
    };
    #[cfg(not(target_os = "linux"))]
    let _ = background;
    #[cfg(target_os = "linux")]
    let tray_state = Rc::new(RefCell::new(None));
    #[cfg(target_os = "linux")]
    let builder = if background {
        builder.tray(tray(tray_state.clone()))
    } else {
        builder
    };
    builder
        .on_started(move |app| {
            let state = PlayerState::new(app.clone());
            #[cfg(target_os = "linux")]
            {
                *tray_state.borrow_mut() = Some(state.clone());
            }
            state.enqueue(paths);
            open_window(&state);
            state.start_pump();
            state.start_video_pump();
            state.start_ticker();
            super::watch_theme_reloads(app.clone(), theme_events, state.window.clone());
            #[cfg(unix)]
            super::ipc::listen(app, state, listener);
        })
        .run();
    #[cfg(unix)]
    cleanup.cleanup();
}

#[cfg(target_os = "linux")]
fn tray_icon() -> TrayIcon {
    let svg = include_bytes!("../../../../assets/icons/apps/app-music.svg");
    let image = ImageData::from_svg(svg, SvgSize::Exact(32, 32)).expect("bundled tray icon");
    TrayIcon::from_rgba(image.pixels().to_vec(), 32, 32).expect("tray icon dimensions")
}

#[cfg(target_os = "linux")]
fn tray(state: Rc<RefCell<Option<PlayerState>>>) -> TrayBuilder {
    let show = state.clone();
    let toggle = state.clone();
    let next = state.clone();
    TrayBuilder::new(tray_icon())
        .tooltip("Cplayer")
        .item("show", "Show player", move |_| {
            if let Some(state) = show.borrow().as_ref() {
                open_window(state);
            }
        })
        .item("play-pause", "Play / Pause", move |_| {
            if let Some(state) = toggle.borrow().as_ref() {
                state.toggle();
            }
        })
        .item("next", "Next", move |_| {
            if let Some(state) = next.borrow().as_ref() {
                state.advance(true);
            }
        })
        .quit_item("quit", "Quit Cplayer")
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn bundled_tray_icon_rasterizes() {
        let _ = super::tray_icon();
    }
}

pub(crate) fn open_window(state: &PlayerState) {
    if let Some(window) = state.window.borrow().as_ref() {
        if window.is_open() {
            window.show();
            return;
        }
    }
    if state.window_pending.replace(true) {
        return;
    }
    let window = state.window.clone();
    let pending = state.window_pending.clone();
    let keys = state.clone();
    let view = state.clone();
    let options = WindowOptions {
        title: "Media Player".into(),
        width: 960,
        height: 640,
        transparent: false,
        decorations: true,
        blur: None,
        backend: Config::load().interface.renderer.backend(),
        theme: super::theme_for(state.media_accent()),
        ..Default::default()
    };
    state.app.append_window(
        options,
        Color::rgb(0, 0, 0),
        move |handle| {
            let activity = keys.clone();
            handle.on_pointer_moved(move |p| activity.poke_pointer((p.x, p.y)));
            let presence = keys.clone();
            handle.on_pointer_presence(move |inside| {
                if inside {
                    presence.poke();
                } else {
                    presence.finish_subtitle_drag();
                }
            });
            let dropped = keys.clone();
            handle.on_files_dropped(move |paths| dropped.drop_files(paths));
            if keys.media_session.borrow().is_none() {
                keys.start_media_controls();
            }
            handle.on_keyboard_input(move |event, modifiers| {
                let pressed = keys.key(event, modifiers);
                if pressed {
                    keys.poke();
                }
                pressed
            });
            *window.borrow_mut() = Some(handle);
            pending.set(false);
        },
        move |size| crate::views::build(&view, size),
    );
}
