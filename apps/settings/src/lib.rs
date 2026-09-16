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
    nested_sidebar, ColorPickerController, IconSource, SidebarNavController, SidebarNode, Surface,
    SurfaceRole, Symbol,
};
use icons::SettingsIcons;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum Section {
    Appearance,
    Theme,
    Wallpaper,
    DesktopIcons,
    Taskbar,
    General,
    Tray,
    Widgets,
    Users,
    Profile,
    User(String),
    CreateUser,
}

fn tree(accounts: &[users::Account], icons: &SettingsIcons) -> Vec<SidebarNode<Section>> {
    let profile_icon = std::env::var("USER")
        .ok()
        .and_then(|username| accounts.iter().find(|account| account.username == username))
        .map(users::account_icon)
        .unwrap_or(IconSource::Symbol(Symbol::Controls));
    vec![
        SidebarNode::parent(
            Section::Appearance,
            icons.appearance.clone(),
            "Appearance",
            vec![
                SidebarNode::leaf(Section::Theme, icons.paintbrush.clone(), "Theme"),
                SidebarNode::leaf(Section::Wallpaper, icons.wallpaper.clone(), "Wallpaper"),
                SidebarNode::leaf(
                    Section::DesktopIcons,
                    IconSource::Symbol(Symbol::Grid),
                    "Desktop icons",
                ),
                SidebarNode::group(
                    Section::Taskbar,
                    "Taskbar",
                    vec![
                        SidebarNode::leaf(Section::General, icons.position.clone(), "Position"),
                        SidebarNode::leaf(Section::Tray, icons.status.clone(), "Status area"),
                        SidebarNode::leaf(Section::Widgets, icons.widgets.clone(), "Widgets"),
                    ],
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
    let active_theme_id = Signal::new(creamui_theme::active_theme_id());
    let view = Signal::new(Section::Theme);
    let nav = SidebarNavController::new();
    let wallpaper_color_picker = ColorPickerController::new();
    let desktop_icons_color_picker = ColorPickerController::new();
    let account_list = users::list_accounts();
    let profile = users::ProfileControllers::load(&account_list);
    let users = Signal::new(account_list);
    let icons = SettingsIcons::load();
    let wallpaper_gallery = sections::wallpaper::GalleryState::new();
    polkit::ensure_kde_agent();
    let window: Rc<RefCell<Option<WindowHandle>>> = Rc::new(RefCell::new(None));
    let initial_theme = creamui_theme::active_theme();

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
                        &active_theme_id,
                        &view,
                        &nav,
                        &window,
                        &wallpaper_color_picker,
                        &desktop_icons_color_picker,
                        &users,
                        &profile,
                        &icons,
                        &wallpaper_gallery,
                    )
                },
            );
        })
        .run();
}

fn build(
    size: Size,
    config: &Signal<ShellConfig>,
    active_theme_id: &Signal<String>,
    view: &Signal<Section>,
    nav: &SidebarNavController<Section>,
    window: &Rc<RefCell<Option<WindowHandle>>>,
    wallpaper_color_picker: &ColorPickerController,
    desktop_icons_color_picker: &ColorPickerController,
    users: &Signal<Vec<users::Account>>,
    profile: &users::ProfileControllers,
    icons: &SettingsIcons,
    wallpaper_gallery: &sections::wallpaper::GalleryState,
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
    let navigation = tree(&account_list, icons);
    let select = view.clone();
    let sidebar = nested_sidebar(
        sidebar_style,
        &navigation,
        nav,
        Some(&current),
        move |section| {
            select.set(section);
        },
    );

    let content = match current {
        Section::Theme => sections::appearance::build(size, active_theme_id, window),
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
        Section::General => sections::general::build(size, config),
        Section::Tray => sections::tray::build(size, config),
        Section::Widgets => sections::widgets::build(size, config),
        Section::Profile => users::build(size, profile),
        Section::User(username) => users::account_view(size, &account_list, &username),
        Section::CreateUser => users::create_user_view(size),
        Section::Appearance | Section::Taskbar | Section::Users => {
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
