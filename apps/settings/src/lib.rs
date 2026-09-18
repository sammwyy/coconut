mod common;
mod icons;
mod polkit;
mod sections;
mod users;

use coconut_core::ShellConfig;
use creamui_core::layout::{Dimension, FlexDirection, LengthPercentage, Style};
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_render::{platform::WindowRole, AppBuilder, WindowHandle, WindowOptions};
use creamui_widgets::{
    nested_sidebar, ColorPickerController, IconSource, ScrollController, SidebarNavController,
    SidebarNode, Surface, SurfaceRole, Symbol,
};
use icons::SettingsIcons;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum Section {
    Appearance,
    Theme,
    IconPack,
    Sound,
    Desktop,
    Wallpaper,
    DesktopIcons,
    Taskbar,
    Dock(usize),
    AddDock,
    Tray,
    Widgets,
    WindowBehaviour,
    Layout,
    Titlebar,
    Compositor,
    WorkingArea,
    Effects,
    Input,
    Focus,
    Shortcuts,
    Users,
    Profile,
    User(String),
    CreateUser,
}

/// The page shown when a sidebar category is opened. Keeping this decision in
/// the app lets `nested_sidebar` remain generic while ensuring its visible
/// selection always corresponds to the content panel.
fn category_default(section: Section) -> Section {
    match section {
        Section::Appearance => Section::Theme,
        Section::Desktop => Section::Wallpaper,
        Section::Taskbar => Section::Dock(0),
        Section::WindowBehaviour => Section::Compositor,
        Section::Users => Section::Profile,
        leaf => leaf,
    }
}

fn tree(
    accounts: &[users::Account],
    icons: &SettingsIcons,
    docks: &[coconut_core::DockConfig],
) -> Vec<SidebarNode<Section>> {
    let profile_icon = std::env::var("USER")
        .ok()
        .and_then(|username| accounts.iter().find(|account| account.username == username))
        .map(users::account_icon)
        .unwrap_or(IconSource::Symbol(Symbol::Controls));
    vec![
        SidebarNode::parent(
            Section::Appearance,
            icons.paintbrush.clone(),
            "Appearance",
            vec![
                SidebarNode::leaf(Section::Theme, icons.paintbrush.clone(), "Theme"),
                SidebarNode::leaf(
                    Section::IconPack,
                    IconSource::Symbol(Symbol::Grid),
                    "Icon Pack",
                ),
                SidebarNode::leaf(Section::Sound, IconSource::Symbol(Symbol::Sliders), "Sound"),
                SidebarNode::group(
                    Section::Desktop,
                    "Desktop (Coconut)",
                    vec![
                        SidebarNode::leaf(Section::Wallpaper, icons.wallpaper.clone(), "Wallpaper"),
                        SidebarNode::leaf(
                            Section::DesktopIcons,
                            IconSource::Symbol(Symbol::Grid),
                            "Desktop icons",
                        ),
                    ],
                ),
                SidebarNode::group(
                    Section::Taskbar,
                    "Docks (Coconut)",
                    docks
                        .iter()
                        .enumerate()
                        .map(|(index, dock)| {
                            SidebarNode::leaf(
                                Section::Dock(index),
                                icons.position.clone(),
                                sections::general::dock_label(index, dock),
                            )
                        })
                        .chain(std::iter::once(SidebarNode::leaf(
                            Section::AddDock,
                            icons.position.clone(),
                            "+ Add dock",
                        )))
                        .chain([
                            SidebarNode::leaf(Section::Tray, icons.status.clone(), "Status area"),
                            SidebarNode::leaf(Section::Widgets, icons.widgets.clone(), "Widgets"),
                        ])
                        .collect(),
                ),
            ],
        ),
        SidebarNode::parent(
            Section::WindowBehaviour,
            IconSource::Symbol(Symbol::Controls),
            "Window behaviour",
            vec![
                SidebarNode::leaf(
                    Section::Compositor,
                    IconSource::Symbol(Symbol::Controls),
                    "Compositor",
                ),
                SidebarNode::leaf(Section::Layout, IconSource::Symbol(Symbol::Grid), "Layout"),
                SidebarNode::leaf(
                    Section::Titlebar,
                    IconSource::Symbol(Symbol::Grid),
                    "Titlebar",
                ),
                SidebarNode::leaf(
                    Section::WorkingArea,
                    IconSource::Symbol(Symbol::Grid),
                    "Workspaces",
                ),
                SidebarNode::leaf(
                    Section::Effects,
                    IconSource::Symbol(Symbol::Controls),
                    "Effects",
                ),
                SidebarNode::leaf(
                    Section::Input,
                    IconSource::Symbol(Symbol::Controls),
                    "Input",
                ),
                SidebarNode::leaf(
                    Section::Focus,
                    IconSource::Symbol(Symbol::Controls),
                    "Focus",
                ),
                SidebarNode::leaf(
                    Section::Shortcuts,
                    IconSource::Symbol(Symbol::Keyboard),
                    "Shortcuts",
                ),
            ],
        ),
        SidebarNode::parent(
            Section::Users,
            icons.users.clone(),
            "Users",
            vec![
                SidebarNode::leaf(Section::Profile, profile_icon, "My profile"),
                SidebarNode::group(
                    Section::Users,
                    "Accounts",
                    accounts
                        .iter()
                        .map(|account| {
                            SidebarNode::leaf(
                                Section::User(account.username.clone()),
                                users::account_icon(account),
                                if account.real_name.is_empty() {
                                    account.username.clone()
                                } else {
                                    account.real_name.clone()
                                },
                            )
                        })
                        .collect(),
                ),
                SidebarNode::leaf(Section::CreateUser, icons.users.clone(), "Create user"),
            ],
        ),
    ]
}

pub fn run() {
    let config = Signal::new(ShellConfig::load());
    let appearance = Signal::new(load_system_appearance());
    let view = Signal::new(Section::Theme);
    let nav = SidebarNavController::new();
    let wallpaper_color_picker = ColorPickerController::new();
    let desktop_icons_color_picker = ColorPickerController::new();
    let appearance_accent_picker = ColorPickerController::new();
    let appearance_custom_accent = Signal::new(false);
    let account_list = users::list_accounts();
    let profile = users::ProfileControllers::load(&account_list);
    let users = Signal::new(account_list);
    let icons = SettingsIcons::load();
    let wallpaper_gallery = sections::wallpaper::GalleryState::new();
    let window_settings = sections::window::WindowState::load();
    let shortcut_settings = sections::window::ShortcutState::load();
    let dock_scroll = ScrollController::new(0.0);
    let dock_tab = Signal::new(sections::general::DockTab::Display);
    polkit::ensure_kde_agent();
    let window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
    let initial_theme = appearance.peek().theme;

    AppBuilder::new()
        .on_started(move |app| {
            app.append_window(
                WindowOptions {
                    title: "Settings".into(),
                    width: 960,
                    height: 640,
                    decorations: true,
                    resizable: true,
                    transparent: false,
                    role: WindowRole::Normal,
                    theme: initial_theme,
                    ..Default::default()
                },
                initial_theme.colors.surface,
                {
                    let window = window.clone();
                    move |handle| *window.borrow_mut() = Some(handle)
                },
                move |size| {
                    build(
                        size,
                        &config,
                        &appearance,
                        &view,
                        &nav,
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
                        &dock_scroll,
                        &dock_tab,
                    )
                },
            );
        })
        .run();
}

fn load_system_appearance() -> creamui_theme::ResolvedAppearance {
    match creamui_theme_loader::SystemThemeLoader::new().load() {
        Ok(appearance) => appearance,
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
            }
        }
    }
}

fn build(
    size: Size,
    config: &Signal<ShellConfig>,
    appearance: &Signal<creamui_theme::ResolvedAppearance>,
    view: &Signal<Section>,
    nav: &SidebarNavController<Section>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    wallpaper_color_picker: &ColorPickerController,
    desktop_icons_color_picker: &ColorPickerController,
    appearance_accent_picker: &ColorPickerController,
    appearance_custom_accent: &Signal<bool>,
    users: &Signal<Vec<users::Account>>,
    profile: &users::ProfileControllers,
    icons: &SettingsIcons,
    wallpaper_gallery: &sections::wallpaper::GalleryState,
    window_settings: &sections::window::WindowState,
    shortcut_settings: &sections::window::ShortcutState,
    dock_scroll: &ScrollController,
    dock_tab: &Signal<sections::general::DockTab>,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();

    let sidebar_style = Style {
        flex_direction: FlexDirection::Column,
        size: creamui_core::layout::Size {
            width: Dimension::Length(196.0),
            height: Dimension::Percent(1.0),
        },
        flex_shrink: 0.0,
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(14.0),
            right: LengthPercentage::Length(10.0),
            top: LengthPercentage::Length(20.0),
            bottom: LengthPercentage::Length(20.0),
        },
        gap: creamui_core::layout::Size {
            width: LengthPercentage::Length(2.0),
            height: LengthPercentage::Length(2.0),
        },
        ..Default::default()
    };
    let current = view.get();
    let account_list = users.get();
    let docks = config.get().docks;
    let navigation = tree(&account_list, icons, &docks);
    let select = view.clone();
    let enter_category = view.clone();
    let add_dock_config = config.clone();
    let sidebar = nested_sidebar(
        sidebar_style,
        &navigation,
        nav,
        Some(&current),
        move |section| {
            if section == Section::AddDock {
                let mut new_index = 0;
                common::update_config(&add_dock_config, |c| {
                    c.docks.push(coconut_core::DockConfig {
                        name: None,
                        ..coconut_core::DockConfig::default()
                    });
                    new_index = c.docks.len() - 1;
                });
                select.set(Section::Dock(new_index));
            } else {
                select.set(section);
            }
        },
        move |section| {
            enter_category.set(category_default(section));
        },
    );

    let content = match current {
        Section::Theme => sections::appearance::build(
            size,
            appearance,
            window,
            appearance_accent_picker,
            appearance_custom_accent,
        ),
        Section::IconPack => sections::asset_packs::icon_packs(size, config),
        Section::Sound => sections::asset_packs::sound_themes(size, config),
        Section::Wallpaper => sections::wallpaper::build(
            size,
            config,
            wallpaper_color_picker,
            wallpaper_gallery,
            window,
        ),
        Section::DesktopIcons => {
            sections::desktop_icons::build(size, config, desktop_icons_color_picker)
        }
        Section::Dock(index) => {
            sections::general::build_dock_page(size, config, &dock_scroll, dock_tab, index)
        }
        Section::AddDock => {
            sections::general::build_dock_page(size, config, &dock_scroll, dock_tab, 0)
        }
        Section::Tray => sections::tray::build(size, config),
        Section::Widgets => sections::widgets::build(size, config),
        Section::Layout => sections::window::build_layout(size, window_settings),
        Section::Titlebar => sections::window::build_titlebar(size, window_settings),
        Section::Compositor => sections::window::build_general(size, window_settings),
        Section::WorkingArea => sections::window::build_working_area(size, window_settings),
        Section::Effects => sections::window::build_effects(size, window_settings),
        Section::Input => sections::window::build_input(size, window_settings),
        Section::Focus => sections::window::build_focus(size, window_settings),
        Section::Shortcuts => sections::window::build_shortcuts(size, shortcut_settings),
        Section::Profile => users::build(size, profile),
        Section::User(username) => users::account_view(size, &account_list, &username),
        Section::CreateUser => users::create_user_view(size),
        Section::Appearance
        | Section::Desktop
        | Section::Taskbar
        | Section::WindowBehaviour
        | Section::Users => {
            unreachable!("sidebar parents are not selectable")
        }
    };

    let panel_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(28.0),
            right: LengthPercentage::Length(28.0),
            top: LengthPercentage::Length(24.0),
            bottom: LengthPercentage::Length(24.0),
        },
        ..Default::default()
    };
    let panel: BoxedWidget = Box::new(Surface::new(SurfaceRole::Panel, panel_style).child(content));
    let panel_outer_style = Style {
        flex_direction: FlexDirection::Column,
        flex_grow: 1.0,
        size: creamui_core::layout::Size {
            width: Dimension::Auto,
            height: Dimension::Percent(1.0),
        },
        padding: creamui_core::layout::Rect {
            left: LengthPercentage::Length(0.0),
            right: LengthPercentage::Length(20.0),
            top: LengthPercentage::Length(20.0),
            bottom: LengthPercentage::Length(20.0),
        },
        ..Default::default()
    };

    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(size.width, size.height)} background={theme.surface}>
            {sidebar}
            <RawView style={panel_outer_style}>
                {panel}
            </RawView>
        </Flex>
    })
}
