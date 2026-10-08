mod components;
mod icons;
mod polkit;
mod routes;
mod users;
mod views;
use creamui_router::{Router, RouterOutlet, RouterProvider};
use routes::destination::{page_description, page_title, section_presentation, Section};
#[cfg(test)]
mod visual_tests;

use coconut_core::ShellConfig;
use creamui_core::layout::{
    Dimension, FlexDirection, LengthPercentage, LengthPercentageAuto, Position, Style,
};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::{
    platform::WindowRole, AppBuilder, BlurRegion, CompositorIntegrationRequest,
    WindowDecorationMode, WindowHandle, WindowOptions,
};
use creamui_widgets::{
    Avatar, CUIWindowDragArea, ColorPickerController, Heading, Icon, IconSource, RawButton,
    RawView, ScrollController, Text, TextController, TextInput, TextSize,
};
use icons::SettingsIcons;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use components::pages::{category_pages, unavailable_page};

use components::navigation::{distribution_name, settings_sidebar, window_controls};

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
    let account_list = users::list_accounts();
    let profile = users::ProfileControllers::load(&account_list);
    let users = Signal::new(account_list);
    let icons = SettingsIcons::load();
    let wallpaper_gallery = crate::views::personalization::wallpaper::GalleryState::new();
    let connectivity = crate::views::connectivity::State::new(view.clone());
    let native = crate::views::native::State::new(integrations.settings.clone());
    let window_settings = Rc::new(crate::views::windows::WindowState::load());
    let shortcut_settings = Rc::new(crate::views::windows::ShortcutState::load());
    let content_scroll = ScrollController::new(0.0);
    let sidebar_scroll = ScrollController::new(0.0);
    let dock_scroll = ScrollController::new(0.0);
    let island_settings = crate::views::desktop::island_settings::State::default();
    let settings_search = TextController::new("");
    let maximized = Signal::new(false);
    let last_view = RefCell::new(Section::Connectivity);
    let reported_decorations = Cell::new(WindowDecorationMode::Pending);
    polkit::ensure_kde_agent();
    let window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
    let initial_theme = coconut_plugin_kit::design::coconut_appearance(&appearance.peek());
    let network_changes = integrations.network.changes();
    let bluetooth_changes = integrations.bluetooth.changes();
    let connectivity_revision = Signal::new(());

    AppBuilder::new()
        .on_started(move |app| {
            native.start(app.clone(), users.clone());
            window_settings.start(app.clone());
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
                    let window = window.clone();
                    move |handle| {
                        handle
                            .set_compositor_integration(Some(CompositorIntegrationRequest::Hybrid));
                        *window.borrow_mut() = Some(handle);
                    }
                },
                move |size| {
                    RouterProvider::new(view.clone()).render(|| {
                        connectivity_revision.get();
                        let current = routes::current(&view);
                        if *last_view.borrow() != current {
                            content_scroll.set(0.0);
                            dock_scroll.set(0.0);
                            *last_view.borrow_mut() = current;
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
                        build(
                            size,
                            &config,
                            &integrations,
                            &appearance,
                            &view,
                            &window,
                            &wallpaper_color_picker,
                            &desktop_icons_color_picker,
                            &appearance_accent_picker,
                            &appearance_custom_accent,
                            &users,
                            &profile,
                            &icons,
                            &wallpaper_gallery,
                            &window_settings,
                            &shortcut_settings,
                            &content_scroll,
                            &sidebar_scroll,
                            &dock_scroll,
                            &island_settings,
                            &settings_search,
                            &connectivity,
                            &maximized,
                            &native,
                        )
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

fn build(
    size: Size,
    config: &Signal<ShellConfig>,
    integrations: &Rc<coconut_api::Registry>,
    appearance: &Signal<creamui_theme::ResolvedAppearance>,
    view: &Router,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    wallpaper_color_picker: &ColorPickerController,
    desktop_icons_color_picker: &ColorPickerController,
    appearance_accent_picker: &ColorPickerController,
    appearance_custom_accent: &Signal<bool>,
    users: &Signal<Vec<users::Account>>,
    profile: &users::ProfileControllers,
    icons: &SettingsIcons,
    wallpaper_gallery: &crate::views::personalization::wallpaper::GalleryState,
    window_settings: &Rc<crate::views::windows::WindowState>,
    shortcut_settings: &Rc<crate::views::windows::ShortcutState>,
    content_scroll: &ScrollController,
    sidebar_scroll: &ScrollController,
    dock_scroll: &ScrollController,
    island_settings: &crate::views::desktop::island_settings::State,
    settings_search: &TextController,
    connectivity: &crate::views::connectivity::State,
    maximized: &Signal<bool>,
    native: &crate::views::native::State,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();

    let sidebar_style = Style {
        flex_direction: FlexDirection::Column,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        flex_grow: 1.0,
        flex_shrink: 0.0,
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(16.0),
            right: LengthPercentage::Length(12.0),
            top: LengthPercentage::Length(20.0),
            bottom: LengthPercentage::Length(20.0),
        },
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(2.0),
            height: LengthPercentage::Length(2.0),
        },
        ..Default::default()
    };
    let current = routes::current(view);
    let connectivity_detail = current == Section::Connectivity && connectivity.showing_detail();
    let native_detail = crate::views::native::parent(&current) != current;
    let decorations = creamui_render::use_window_decorations();
    let show_page_header = !connectivity_detail && !native_detail;
    let account_list = users.get();
    let title = page_title(&current);
    let description = page_description(&current);
    let (category_icon, category_color) = section_presentation(&current, icons);
    creamui_reactive::provide_context(components::PageStyle {
        color: category_color,
        title: title.clone(),
    });
    let _ = &sidebar_style;
    let sidebar = settings_sidebar(&current, view, icons);

    let content_has_own_scroll = matches!(current, Section::Statusbar | Section::Dockbar);
    creamui_reactive::provide_context(crate::views::context::ViewContext {
        size: size,
        config: config.clone(),
        integrations: integrations.clone(),
        appearance: appearance.clone(),
        view: view.clone(),
        window: window.clone(),
        wallpaper_color_picker: wallpaper_color_picker.clone(),
        desktop_icons_color_picker: desktop_icons_color_picker.clone(),
        appearance_accent_picker: appearance_accent_picker.clone(),
        appearance_custom_accent: appearance_custom_accent.clone(),
        users: users.clone(),
        profile: profile.clone(),
        icons: icons.clone(),
        wallpaper_gallery: wallpaper_gallery.clone(),
        window_settings: window_settings.clone(),
        shortcut_settings: shortcut_settings.clone(),
        content_scroll: content_scroll.clone(),
        sidebar_scroll: sidebar_scroll.clone(),
        dock_scroll: dock_scroll.clone(),
        island_settings: island_settings.clone(),
        settings_search: settings_search.clone(),
        connectivity: connectivity.clone(),
        maximized: maximized.clone(),
        native: native.clone(),
    });
    let content = RouterOutlet::render();

    let sidebar_shell_style = Style {
        flex_direction: FlexDirection::Column,
        size: creamui_core::layout::Size {
            width: Dimension::Length(256.0),
            height: Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        border: creamui_core::layout::Rect {
            right: LengthPercentage::Length(1.0),
            left: LengthPercentage::Length(0.0),
            top: LengthPercentage::Length(0.0),
            bottom: LengthPercentage::Length(0.0),
        },
        ..Default::default()
    };
    // The concept does not have a separate application toolbar: the page
    // heading belongs to the scrollable document, with a generous inset and
    // no divider beneath it.  Keeping it in the document also prevents large
    // headings from being squeezed into a fixed-height strip.
    let page_header_style = Style {
        flex_direction: FlexDirection::Row,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(0.0),
            right: LengthPercentage::Length(0.0),
            top: LengthPercentage::Length(48.0),
            bottom: LengthPercentage::Length(0.0),
        },
        ..Default::default()
    };
    let content_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        flex_shrink: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Percent(1.0),
            height: Dimension::Auto,
        },
        min_size: creamui_core::layout::Size {
            width: Dimension::Length(0.0),
            height: Dimension::Length(0.0),
        },
        ..Default::default()
    };
    let page_header: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} style={page_header_style} gap={16.0} align={creamui_widgets::layout::Align::Center}>
            <Flex size={(56.0, 56.0)} align={creamui_widgets::layout::Align::Center} justify={creamui_widgets::layout::Justify::Center} background={creamui_core::LinearGradient::new(180.0, category_color.mix(creamui_theme::Color::rgb(255, 255, 255), 0.22), category_color)} corner_radius={16.0}>
                {Box::new(Icon::new(category_icon, creamui_theme::Color::rgb(255, 255, 255)).size(28.0)) as BoxedWidget}
            </Flex>
            <Flex direction={FlexDirection::Column} gap={4.0} justify={creamui_widgets::layout::Justify::Center}>
                {Box::new(Heading::lg(title).font_size(24.0)) as BoxedWidget}
                {Box::new(Text::secondary(description).font_size(14.0)) as BoxedWidget}
            </Flex>
        </Flex>
    });
    let panel_content: BoxedWidget = if content_has_own_scroll {
        let header = if native_detail {
            crate::views::native::detail_header(&current, view, size.width)
        } else {
            page_header
        };
        Box::new(jsx! {
            <Flex direction={FlexDirection::Column} grow={1.0} gap={if native_detail { 0.0 } else { 32.0 }}>
                {header}
                {content}
            </Flex>
        })
    } else {
        let scroll: BoxedWidget = Box::new(
            creamui_widgets::RawScrollView::controlled(content_style, content_scroll.clone())
                .scrollbar_width(2.0)
                .scrollbar_margin(0.0)
                .scrollbar_color(creamui_theme::Color::rgba(
                    theme.colors.text_secondary.r,
                    theme.colors.text_secondary.g,
                    theme.colors.text_secondary.b,
                    38,
                ))
                .child(Box::new(
                    RawView::new(Style {
                        flex_direction: FlexDirection::Column,
                        flex_shrink: 0.0,
                        // Let the form use the available width at the normal
                        // window size; retain a readable cap on large screens.
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Auto,
                        },
                        max_size: creamui_core::layout::Size {
                            width: Dimension::Length(864.0),
                            height: Dimension::Auto,
                        },
                        align_self: Some(creamui_core::layout::AlignSelf::Center),
                        gap: creamui_core::layout::Size {
                            width: LengthPercentage::Length(0.0),
                            height: LengthPercentage::Length(32.0),
                        },
                        padding: creamui_core::layout::Rect {
                            left: LengthPercentage::Length(if size.width < 1000.0 {
                                24.0
                            } else {
                                40.0
                            }),
                            right: LengthPercentage::Length(if size.width < 1000.0 {
                                24.0
                            } else {
                                40.0
                            }),
                            top: LengthPercentage::Length(if show_page_header {
                                0.0
                            } else if connectivity_detail
                                && matches!(
                                    connectivity.current(),
                                    crate::views::connectivity::View::KnownNetworks
                                        | crate::views::connectivity::View::NearbyNetworks
                                )
                            {
                                16.0
                            } else if connectivity_detail || native_detail {
                                28.0
                            } else {
                                11.0
                            }),
                            bottom: LengthPercentage::Length(64.0),
                        },
                        ..Default::default()
                    })
                    .with_children(if show_page_header {
                        vec![page_header, content]
                    } else {
                        vec![content]
                    }),
                )),
        );
        if connectivity_detail || native_detail {
            let header = if connectivity_detail {
                crate::views::connectivity::detail_header(connectivity, size.width)
            } else {
                crate::views::native::detail_header(&current, view, size.width)
            };
            Box::new(jsx! {
                <Flex direction={FlexDirection::Column} grow={1.0} gap={0.0}>
                    {header}
                    {scroll}
                </Flex>
            })
        } else {
            scroll
        }
    };
    let main_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        ..Default::default()
    };
    let profile_name = std::env::var("USER").unwrap_or_else(|_| "User".into());
    let account = account_list
        .iter()
        .find(|account| account.username == profile_name);
    let display_name = account
        .map(|account| account.real_name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(&profile_name);
    let profile_initial = display_name
        .chars()
        .next()
        .unwrap_or('U')
        .to_uppercase()
        .to_string();
    let mut avatar = Avatar::new(40.0)
        .fallback_color(theme.colors.accent)
        .initials(profile_initial)
        .initials_color(theme.colors.selection_text);
    if let Some(IconSource::Image(image)) = account.map(users::account_icon) {
        avatar = avatar.image(Box::new(
            Icon::new(IconSource::Image(image), theme.colors.text_primary).size(40.0),
        ));
    }
    let search: BoxedWidget = Box::new(
        TextInput::controlled_with_style(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(1.0),
                    height: Dimension::Length(32.0),
                },
                padding: creamui_core::layout::Rect {
                    left: LengthPercentage::Length(32.0),
                    right: LengthPercentage::Length(12.0),
                    top: LengthPercentage::Length(0.0),
                    bottom: LengthPercentage::Length(0.0),
                },
                ..Default::default()
            },
            settings_search,
        )
        .placeholder("Search settings")
        .font_size(12.0)
        .background(
            theme
                .colors
                .surface_elevated
                .mix(theme.colors.surface, 0.45),
        ),
    );
    let sidebar_header: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={16.0} style={Style { flex_direction: FlexDirection::Column, flex_shrink: 0.0, padding: creamui_core::layout::Rect { left: LengthPercentage::Length(16.0), right: LengthPercentage::Length(16.0), top: LengthPercentage::Length(20.0), bottom: LengthPercentage::Length(12.0) }, ..Default::default() }}>
            <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} padding_xy={(4.0, 0.0)}>
                {Box::new(avatar) as BoxedWidget}
                <Flex direction={FlexDirection::Column} gap={2.0}>
                    {Box::new(Text::new(display_name).font_size(13.0).bold(true)) as BoxedWidget}
                    {Box::new(Text::secondary(distribution_name()).size(TextSize::Xs)) as BoxedWidget}
                </Flex>
            </Flex>
            <RawView style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(32.0) }, ..Default::default() }}>
                {search}
                {Box::new(RawView::new(Style { position: Position::Absolute, inset: creamui_core::layout::Rect { left: LengthPercentageAuto::Length(10.0), top: LengthPercentageAuto::Length(8.0), right: LengthPercentageAuto::Auto, bottom: LengthPercentageAuto::Auto }, ..Default::default() }).child(Box::new(Icon::new(icons.search.clone(), theme.colors.text_secondary).size(16.0)))) as BoxedWidget}
            </RawView>
        </Flex>
    });
    // Declare background regions, not invisible buttons. CreamUI gives the
    // compact header's back button and any future controls input priority.
    let drag_area: BoxedWidget = Box::new(CUIWindowDragArea::new().layout(Style {
        position: Position::Absolute,
        inset: creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(256.0),
            right: LengthPercentageAuto::Length(0.0),
            top: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Auto,
        },
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Length(if connectivity_detail || native_detail {
                56.0
            } else {
                48.0
            }),
        },
        ..Default::default()
    }));
    // Only the empty strip above the account, not the avatar/search/sidebar.
    let sidebar_drag_area: BoxedWidget = Box::new(CUIWindowDragArea::new().layout(Style {
        position: Position::Absolute,
        inset: creamui_core::layout::Rect {
            left: LengthPercentageAuto::Length(0.0),
            right: LengthPercentageAuto::Auto,
            top: LengthPercentageAuto::Length(0.0),
            bottom: LengthPercentageAuto::Auto,
        },
        size: creamui_core::layout::Size {
            width: Dimension::Length(256.0),
            height: Dimension::Length(16.0),
        },
        ..Default::default()
    }));
    let controls = if decorations.mode == WindowDecorationMode::Client {
        window_controls(window, icons, maximized)
    } else {
        Box::new(RawView::new(Style::default())) as BoxedWidget
    };

    let window_surface = theme.colors.surface;
    let sidebar_surface = window_surface.mix(theme.colors.accent, 0.035);
    let page_surface = creamui_core::LinearGradient::new(
        150.0,
        window_surface,
        window_surface.mix(theme.colors.accent, 0.055),
    );
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(size.width, size.height)} background={window_surface}>
            <RawView style={sidebar_shell_style} background={sidebar_surface}>
                {sidebar_header}
                {Box::new(creamui_widgets::RawScrollView::controlled(Style { flex_grow: 1.0, min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Length(0.0) }, ..Default::default() }, sidebar_scroll.clone()).scrollbar(false).child(sidebar)) as BoxedWidget}
                {Box::new(RawView::new(Style { position: Position::Absolute, inset: creamui_core::layout::Rect { left: LengthPercentageAuto::Auto, right: LengthPercentageAuto::Length(0.0), top: LengthPercentageAuto::Length(0.0), bottom: LengthPercentageAuto::Length(0.0) }, size: creamui_core::layout::Size { width: Dimension::Length(1.0), height: Dimension::Percent(1.0) }, ..Default::default() }).background(theme.colors.border)) as BoxedWidget}
            </RawView>
            <RawView style={main_style} background={page_surface}>
                {panel_content}
            </RawView>
            {drag_area}
            {sidebar_drag_area}
            {controls}
        </Flex>
    })
}
