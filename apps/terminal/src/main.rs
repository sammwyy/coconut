mod app;
mod appearance;
mod backends;
mod cli;
mod config;
mod input;
mod island;
mod screen;
mod search;
mod session;
mod settings;
mod shortcuts;
mod terminal;
mod terminal_graphics;
mod ui;

use creamui_render::{AppBuilder, BlurRegion, RenderBackend, WindowOptions};
use creamui_theme_watch::ThemeListener;
use std::sync::{mpsc, Arc, Mutex};

fn main() -> Result<(), String> {
    let launch = cli::parse(std::env::args_os().skip(1))?;
    match launch {
        cli::Launch::Help => println!("{}", cli::HELP),
        cli::Launch::Version => println!("Terminal {}", env!("CARGO_PKG_VERSION")),
        cli::Launch::Settings => run_settings(),
        cli::Launch::ExportConfig(path) => export_config(path)?,
        cli::Launch::ImportConfig(path) => import_config(path)?,
        cli::Launch::Terminal { cwd, command } => run_terminal(cwd, command),
    }
    Ok(())
}
fn export_config(path: std::path::PathBuf) -> Result<(), String> {
    config::Config::load()?.save_to(&path)?;
    println!("Exported settings to {}", path.display());
    Ok(())
}
fn import_config(path: std::path::PathBuf) -> Result<(), String> {
    let config = config::Config::import_from(&path)?;
    config.save_to(&config::path())?;
    println!("Imported settings from {}", path.display());
    Ok(())
}
fn options(title: &str, width: u32, height: u32) -> WindowOptions {
    WindowOptions {
        title: title.into(),
        width,
        height,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: appearance::load_system_theme(),
        backend: RenderBackend::Cpu,
        ..Default::default()
    }
}
fn run_terminal(cwd: std::path::PathBuf, command: Vec<String>) {
    backends::prepare_display();
    let configuration = config::Config::load();
    let (sender, receiver) = mpsc::sync_channel(128);
    let state = app::State::new(cwd, command, sender.clone());
    if let Ok(config) = &configuration {
        state.configure(config.clone());
    }
    state.enable_session_restore();
    if !state.restore_session() {
        state.add();
    }
    if let Err(error) = configuration {
        state.show_error(error);
    }
    app::watch_configuration(sender);
    let cleanup = state.clone();
    let window = state.window.clone();
    let ready = window.clone();
    let events = Arc::new(Mutex::new(receiver));
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    let opts = options("Terminal", 1000, 680);
    AppBuilder::new()
        .on_started(move |app| {
            let keys = state.clone();
            let build = state.clone();
            let focus = state.clone();
            app.append_window(
                opts,
                ui::CLEAR,
                move |handle| {
                    handle.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    handle.on_focus_lost(move || focus.set_window_focused(false));
                    *ready.borrow_mut() = Some(handle);
                },
                move |size| build.build(size),
            );
            app::watch(app.clone(), events, state);
            appearance::watch_theme_reloads(app, theme_events, window);
        })
        .run();
    cleanup.cleanup();
}
fn run_settings() {
    backends::prepare_display();
    let state = settings::State::new();
    let window = state.window.clone();
    let ready = window.clone();
    let keys = state.clone();
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    let opts = options("Terminal Settings", 720, 640);
    AppBuilder::new()
        .on_started(move |app| {
            app.append_window(
                opts,
                ui::CLEAR,
                move |handle| {
                    handle.on_keyboard_input(move |event, modifiers| keys.key(event, modifiers));
                    *ready.borrow_mut() = Some(handle);
                },
                move |size| state.build(size),
            );
            appearance::watch_theme_reloads(app, theme_events, window);
        })
        .run();
}
