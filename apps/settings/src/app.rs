use crate::{components, routes};
use creamui_router::RouterProvider;

use crate::icons::SettingsIcons;
use coconut_core::ShellConfig;
use creamui_core::Size;
use creamui_reactive::Signal;
use creamui_render::{
    platform::WindowRole, AppBuilder, BlurRegion, CompositorIntegrationRequest,
    WindowDecorationMode, WindowHandle, WindowOptions,
};
use creamui_widgets::{ColorPickerController, ScrollController, TextController};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn run() {
    components::load_settings_fonts();
    let config = Signal::new(ShellConfig::load());
    let integrations = Rc::new(coconut_registry::detect());
    let appearance = Signal::new(load_system_appearance());
    let view = routes::create(&routes::initial_route(std::env::args().nth(1).as_deref()))
        .unwrap_or_else(|error| {
            eprintln!("settings: invalid initial route: {error}");
            routes::create(routes::DEFAULT_ROUTE).expect("valid settings routes")
        });
    let wallpaper_color_picker = ColorPickerController::new();
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
        integrations,
        appearance,
        view,
        window,
        wallpaper_color_picker,
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
        island_settings,
        settings_search,
        connectivity,
        maximized,
        native,
    };

    AppBuilder::new()
        .on_started(move |app| {
            state.native.start(app.clone(), state.users.clone());
            state.window_settings.start(app.clone());
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
                    ..Default::default()
                },
                creamui_theme::Color::rgba(0, 0, 0, 0),
                {
                    let window = state.window.clone();
                    move |handle| {
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
    match creamui_theme_loader::SystemThemeLoader::new().load() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance
        }
        Err(error) => {
            eprintln!("settings: failed to load CreamUI system appearance: {error}");
            let theme = creamui_theme_loader::builtin_theme();
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
