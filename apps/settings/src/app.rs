use crate::{components, routes};
use creamui_router::RouterProvider;

use crate::icons::SettingsIcons;
use coconut_core::{BackgroundConfig, ShellConfig};
use creamui_core::Size;
use creamui_reactive::Signal;
use creamui_render::{
    platform::WindowRole, AppBuilder, BlurRegion, CompositorIntegrationRequest, RenderBackend,
    WindowDecorationMode, WindowHandle, WindowOptions,
};
use creamui_widgets::{ColorPickerController, ScrollController, TextController};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn run() {
    components::load_settings_fonts();
    let config = Signal::new(ShellConfig::load());
    let background = Signal::new(BackgroundConfig::load());
    let render_backend = if config.peek().rendering.gpu_acceleration {
        RenderBackend::Gpu
    } else {
        RenderBackend::Cpu
    };
    let integrations = Rc::new(coconut_registry::detect());
    let appearance = Signal::new(load_system_appearance());
    let view = routes::create(&routes::initial_route(std::env::args().nth(1).as_deref()))
        .unwrap_or_else(|error| {
            eprintln!("settings: invalid initial route: {error}");
            routes::create(routes::DEFAULT_ROUTE).expect("valid settings routes")
        });
    let wallpaper_pickers = crate::views::personalization::wallpaper::WallpaperPickers::new();
    let desktop_icons_color_picker = ColorPickerController::new();
    let appearance_accent_picker = ColorPickerController::new();
    let appearance_custom_accent = Signal::new(false);
    let account_list = crate::services::accounts::list_accounts();
    let profile = crate::views::users::profile::ProfileControllers::load(&account_list);
    let users = Signal::new(account_list);
    let icons = SettingsIcons::load();
    let wallpaper_gallery = crate::views::personalization::wallpaper::GalleryState::new();
    let connectivity = crate::views::connectivity::State::new(view.clone());
    let native = crate::services::settings::State::new(integrations.settings.clone());
    let window_settings = Rc::new(crate::views::windows::WindowState::load());
    let shortcut_settings = Rc::new(crate::views::windows::ShortcutState::load());
    let content_scroll = ScrollController::new(0.0);
    let sidebar_scroll = ScrollController::new(0.0);
    let dock_scroll = ScrollController::new(0.0);
    let island_settings = crate::views::desktop::island_settings::State::default();
    let settings_search = TextController::new("");
    let search_scroll = content_scroll.clone();
    settings_search.on_change(move |_, next| {
        search_scroll.set(0.0);
        Some(next.to_owned())
    });
    let maximized = Signal::new(false);
    let last_location = RefCell::new(view.url());
    let reported_decorations = Cell::new(WindowDecorationMode::Pending);
    crate::polkit::ensure_kde_agent();
    let window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
    let initial_theme = coconut_plugin_kit::design::coconut_appearance(&appearance.peek());
    let network_changes = integrations.network.changes();
    let bluetooth_changes = integrations.bluetooth.changes();
    let connectivity_revision = Signal::new(());

    let state = crate::views::context::ViewContext {
        size: Size {
            width: 0.0,
            height: 0.0,
        },
        config,
        background,
        integrations,
        appearance,
        view,
        window,
        wallpaper_pickers,
        desktop_icons_color_picker,
        appearance_accent_picker,
        appearance_custom_accent,
        users,
        profile,
        icons,
        wallpaper_gallery,
        window_settings,
        shortcut_settings,
        content_scroll,
        sidebar_scroll,
        dock_scroll,
        bar_content: crate::views::desktop::bar_content::State::default(),
        island_settings,
        settings_search,
        connectivity,
        maximized,
        native,
    };

    AppBuilder::new()
        .on_started(move |app| {
            let appearance = state.appearance.clone();
            let window = state.window.clone();
            state.native.on_appearance_change(move |next| {
                if let Some(handle) = window.borrow().as_ref() {
                    handle.set_theme(coconut_plugin_kit::design::coconut_appearance(&next));
                }
                appearance.set(next);
            });
            state.native.start(app.clone(), state.users.clone());
            watch_shell_config(
                app.clone(),
                state.config.clone(),
                state.bar_content.clone(),
                std::sync::Arc::new(std::sync::Mutex::new(
                    coconut_core::ipc::listen_for_runtime_events(),
                )),
            );
            state.window_settings.start(app.clone());
            state.shortcut_settings.start(app.clone());
            watch_connectivity(app.clone(), network_changes, connectivity_revision.clone());
            watch_connectivity(
                app.clone(),
                bluetooth_changes,
                connectivity_revision.clone(),
            );
            app.append_window(
                WindowOptions {
                    title: "Settings".into(),
                    // Same maximum floating window as the reference shell.
                    width: 1120,
                    height: 760,
                    decorations: true,
                    resizable: true,
                    transparent: true,
                    blur: Some(BlurRegion::Window),
                    role: WindowRole::Normal,
                    theme: initial_theme,
                    backend: render_backend,
                    ..Default::default()
                },
                creamui_theme::Color::rgba(0, 0, 0, 0),
                {
                    let window = state.window.clone();
                    let shortcuts = state.shortcut_settings.clone();
                    let bar_content = state.bar_content.clone();
                    let search = state.settings_search.clone();
                    move |handle| {
                        shortcuts.attach_window(handle.clone());
                        let keys = bar_content.clone();
                        let shortcuts_keys = shortcuts.clone();
                        handle.on_keyboard_input(move |event, modifiers| {
                            if event.pressed
                                && event.key == creamui_render::platform::Key::Escape
                                && !search.peek().is_empty()
                            {
                                search.set_value("");
                                return true;
                            }
                            coconut_plugin_kit::bar_editor::key_input(&event.key, modifiers)
                                .is_some_and(|input| keys.handle_key(input, event.pressed))
                                || shortcuts_keys.record_native(event, modifiers)
                        });
                        let blur = bar_content.clone();
                        handle.on_focus_lost(move || {
                            blur.focus_lost();
                            shortcuts.cancel_capture();
                        });
                        handle
                            .set_compositor_integration(Some(CompositorIntegrationRequest::Hybrid));
                        *window.borrow_mut() = Some(handle);
                    }
                },
                move |size| {
                    RouterProvider::new(state.view.clone()).render(|| {
                        connectivity_revision.get();
                        let location = state.view.url();
                        if *last_location.borrow() != location {
                            state.settings_search.set_value("");
                            state.bar_content.dismiss();
                            state.shortcut_settings.cancel_capture();
                            state.content_scroll.set(0.0);
                            state.dock_scroll.set(0.0);
                            *last_location.borrow_mut() = location;
                        }
                        let decorations = creamui_render::use_window_decorations();
                        if decorations.mode != WindowDecorationMode::Pending
                            && reported_decorations.replace(decorations.mode) != decorations.mode
                        {
                            eprintln!(
                                "settings: CreamUI negotiated window decorations: {:?}",
                                decorations.mode
                            );
                        }
                        let mut context = state.clone();
                        context.size = size;
                        crate::views::layout::build(&context)
                    })
                },
            );
        })
        .run();
}

fn watch_connectivity(
    app: creamui_render::AppHandle,
    listener: Option<coconut_api::ChangeListener>,
    revision: Signal<()>,
) {
    let Some(listener) = listener else {
        return;
    };
    let next_app = app.clone();
    let next_listener = listener.clone();
    app.spawn_background(
        move || listener.wait(),
        move |changed| {
            if changed {
                revision.set(());
                watch_connectivity(next_app, Some(next_listener), revision);
            }
        },
    );
}

fn load_system_appearance() -> creamui_theme::ResolvedAppearance {
    match coconut_integration_cream::load_system_appearance() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance
        }
        Err(error) => {
            eprintln!("settings: failed to load CreamUI system appearance: {error}");
            let theme = coconut_integration_cream::builtin_theme();
            let variant_id = theme.default_variant.clone();
            let resolved = theme.default_theme();
            creamui_theme::ResolvedAppearance {
                theme_id: theme.id,
                variant_id,
                accent: resolved.colors.accent,
                theme: resolved,
                font_family: None,
                corners: Default::default(),
            }
        }
    }
}

fn watch_shell_config(
    app: creamui_render::AppHandle,
    config: Signal<ShellConfig>,
    editor: crate::views::desktop::bar_content::State,
    events: std::sync::Arc<
        std::sync::Mutex<std::sync::mpsc::Receiver<coconut_core::ipc::RuntimeEvent>>,
    >,
) {
    let next_app = app.clone();
    let next_events = events.clone();
    app.spawn_background(
        move || match events.lock() {
            Ok(events) => match events.recv() {
                Ok(event) => Some(event),
                Err(error) => {
                    eprintln!("settings: shell configuration listener stopped: {error}");
                    None
                }
            },
            Err(error) => {
                eprintln!("settings: shell configuration listener lock failed: {error}");
                None
            }
        },
        move |event| {
            let Some(event) = event else {
                return;
            };
            if let coconut_core::ipc::RuntimeEvent::ShellConfig(next) = event {
                if *next != config.peek() {
                    editor.dismiss();
                    config.set(*next);
                }
            }
            watch_shell_config(next_app, config, editor, next_events);
        },
    );
}
