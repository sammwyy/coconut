mod common;
mod icons;
mod polkit;
mod sections;
mod users;
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
    RawView, ScrollController, SidebarNavController, SidebarNode, Symbol, Text, TextController,
    TextInput, TextSize,
};
use icons::SettingsIcons;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
enum Section {
    GroupNetwork,
    GroupLookAndFeel,
    GroupWorkflow,
    GroupPeople,
    GroupSystem,
    Connectivity,
    Hardware,
    Personalization,
    IconPack,
    Sound,
    Desktop,
    Wallpaper,
    DesktopIcons,
    Statusbar,
    Dockbar,
    Tray,
    Islands,
    Windows,
    System,
    Layout,
    Titlebar,
    Compositor,
    WorkingArea,
    Effects,
    Keyboard,
    Mouse,
    CursorTheme,
    Touchpad,
    Focus,
    ShortcutsCategory,
    Shortcuts,
    Applications,
    Users,
    Profile,
    User(String),
    CreateUser,
    Privacy,
    Accessibility,
    About,
    Detail(sections::native::Page),
    Unavailable(String),
}

/// The page shown when a sidebar category is opened. Keeping this decision in
/// the app lets `nested_sidebar` remain generic while ensuring its visible
/// selection always corresponds to the content panel.
fn category_default(section: Section) -> Section {
    match section {
        Section::Connectivity => Section::Unavailable("Connectivity".into()),
        Section::Hardware => Section::Keyboard,
        Section::Desktop => Section::Wallpaper,
        Section::Windows => Section::Layout,
        Section::ShortcutsCategory => Section::Shortcuts,
        Section::Applications => Section::Unavailable("Applications".into()),
        Section::Users => Section::Profile,
        Section::Privacy => Section::Unavailable("Privacy & Security".into()),
        Section::Accessibility => Section::Unavailable("Accessibility".into()),
        Section::System => Section::Unavailable("System".into()),
        Section::About => Section::Unavailable("About".into()),
        leaf => leaf,
    }
}

fn page_title(section: &Section) -> String {
    match section {
        Section::Detail(page) => page.title().into(),
        Section::IconPack => "Icons".into(),
        Section::Sound => "Sound".into(),
        Section::Wallpaper => "Wallpaper".into(),
        Section::DesktopIcons => "Desktop icons".into(),
        Section::Statusbar => "Statusbar".into(),
        Section::Dockbar => "Dockbar".into(),
        Section::Tray => "Status icons".into(),
        Section::Islands => "Islands".into(),
        Section::Layout => "Window layout".into(),
        Section::Titlebar => "Titlebar".into(),
        Section::Compositor => "Advanced".into(),
        Section::WorkingArea => "Workspaces".into(),
        Section::Effects => "Motion".into(),
        Section::Keyboard => "Keyboard".into(),
        Section::Mouse => "Mouse & touchpad".into(),
        Section::CursorTheme => "Cursor".into(),
        Section::Touchpad => "Touchpad".into(),
        Section::Focus => "Window focus".into(),
        Section::Shortcuts => "Shortcuts".into(),
        Section::Profile => "My profile".into(),
        Section::User(username) => username.clone(),
        Section::CreateUser => "Create user".into(),
        Section::Unavailable(title) => title.clone(),
        Section::Connectivity => "Connectivity".into(),
        Section::Hardware => "Hardware".into(),
        Section::Personalization => "Personalization".into(),
        Section::Desktop => "Desktop".into(),
        Section::Windows => "Windows".into(),
        Section::ShortcutsCategory => "Shortcuts".into(),
        Section::Applications => "Applications".into(),
        Section::Users => "Users & Accounts".into(),
        Section::Privacy => "Privacy & Security".into(),
        Section::Accessibility => "Accessibility".into(),
        Section::System => "System".into(),
        Section::About => "About".into(),
        Section::GroupNetwork
        | Section::GroupLookAndFeel
        | Section::GroupWorkflow
        | Section::GroupPeople
        | Section::GroupSystem => "Settings".into(),
    }
}

/// The mock uses a small, human description below each page title.  Keep it
/// here with the navigation metadata so the native pages get that context
/// without every individual settings section needing to duplicate chrome.
fn page_description(section: &Section) -> &'static str {
    match section {
        Section::Connectivity => "Wireless, wired and virtual network connections.",
        Section::Hardware => "Displays, audio, input and connected peripherals.",
        Section::Personalization => "Make the desktop feel like yours.",
        Section::Desktop => "Panels, widgets, status controls and desktop icons.",
        Section::Windows => "How windows open, arrange and behave.",
        Section::ShortcutsCategory => "Keyboard shortcuts for the system and apps.",
        Section::Applications => "Installed apps, defaults and startup items.",
        Section::Users => "People who use this device and their accounts.",
        Section::Privacy => "Control what apps can access and how you sign in.",
        Section::Accessibility => "Adjust the system for vision, hearing and motor needs.",
        Section::System => "Updates, storage, date, language and more.",
        Section::About => "Information about this device.",
        Section::IconPack => "Choose the icons used throughout the desktop.",
        Section::Sound => "Select the sound theme for desktop events.",
        Section::CursorTheme => "Choose the pointer style used by applications.",
        Section::Wallpaper | Section::DesktopIcons => "Personalize how your desktop looks.",
        Section::Statusbar | Section::Dockbar | Section::Tray | Section::Islands => {
            "Configure the desktop shell and its status controls."
        }
        Section::Layout | Section::Titlebar | Section::WorkingArea | Section::Focus => {
            "Control how application windows behave."
        }
        Section::Compositor | Section::Effects => "Fine-tune desktop rendering and motion.",
        Section::Keyboard | Section::Mouse | Section::Touchpad => {
            "Adjust your input devices and interactions."
        }
        Section::Shortcuts => "Set the keyboard shortcuts used by the desktop.",
        Section::Profile | Section::User(_) | Section::CreateUser => {
            "Manage the people and accounts on this computer."
        }
        Section::Unavailable(_) => "This area is planned but is not supported by Coconut yet.",
        _ => "Customize your Coconut desktop.",
    }
}

#[allow(unreachable_code, unused_variables)]
fn tree(accounts: &[users::Account], icons: &SettingsIcons) -> Vec<SidebarNode<Section>> {
    // This is intentionally a flat navigation model: the mock's categories
    // are destinations, not folders.  A click changes the main view while
    // keeping the complete category list visible in the sidebar.
    return vec![
        SidebarNode::group(
            Section::GroupNetwork,
            "Network",
            vec![
                SidebarNode::leaf(Section::Connectivity, icons.status.clone(), "Connectivity"),
                SidebarNode::leaf(Section::Hardware, icons.devices.clone(), "Hardware"),
            ],
        ),
        SidebarNode::group(
            Section::GroupLookAndFeel,
            "Look & Feel",
            vec![
                SidebarNode::leaf(
                    Section::Personalization,
                    icons.paintbrush.clone(),
                    "Personalization",
                ),
                SidebarNode::leaf(Section::Desktop, icons.wallpaper.clone(), "Desktop"),
                SidebarNode::leaf(Section::Windows, icons.windows.clone(), "Windows"),
            ],
        ),
        SidebarNode::group(
            Section::GroupWorkflow,
            "Workflow",
            vec![
                SidebarNode::leaf(
                    Section::ShortcutsCategory,
                    icons.shortcuts.clone(),
                    "Shortcuts",
                ),
                SidebarNode::leaf(
                    Section::Applications,
                    IconSource::Symbol(Symbol::Grid),
                    "Applications",
                ),
            ],
        ),
        SidebarNode::group(
            Section::GroupPeople,
            "People",
            vec![
                SidebarNode::leaf(Section::Users, icons.users.clone(), "Users & Accounts"),
                SidebarNode::leaf(
                    Section::Privacy,
                    IconSource::Symbol(Symbol::Controls),
                    "Privacy & Security",
                ),
                SidebarNode::leaf(
                    Section::Accessibility,
                    IconSource::Symbol(Symbol::Appearance),
                    "Accessibility",
                ),
            ],
        ),
        SidebarNode::group(
            Section::GroupSystem,
            "System",
            vec![
                SidebarNode::leaf(Section::System, icons.system.clone(), "System"),
                SidebarNode::leaf(Section::About, IconSource::Symbol(Symbol::Display), "About"),
            ],
        ),
    ];

    let profile_icon = std::env::var("USER")
        .ok()
        .and_then(|username| accounts.iter().find(|account| account.username == username))
        .map(users::account_icon)
        .unwrap_or(IconSource::Symbol(Symbol::Controls));
    let personalization = SidebarNode::parent(
        Section::Personalization,
        icons.paintbrush.clone(),
        "Personalization",
        vec![
            SidebarNode::leaf(Section::Wallpaper, icons.wallpaper.clone(), "Wallpaper"),
            SidebarNode::leaf(
                Section::IconPack,
                IconSource::Symbol(Symbol::Grid),
                "Icon pack",
            ),
            SidebarNode::leaf(Section::CursorTheme, icons.cursor.clone(), "Cursor"),
            SidebarNode::leaf(Section::Sound, icons.music_note.clone(), "Sound pack"),
        ],
    );
    let desktop = SidebarNode::parent(
        Section::Desktop,
        icons.wallpaper.clone(),
        "Desktop",
        vec![
            SidebarNode::leaf(Section::Statusbar, icons::blank(), "Top panel"),
            SidebarNode::leaf(Section::Dockbar, icons::blank(), "Dock"),
            SidebarNode::leaf(Section::Islands, icons.islands.clone(), "Widgets"),
            SidebarNode::leaf(Section::Tray, icons.status.clone(), "Status icons"),
            SidebarNode::leaf(
                Section::DesktopIcons,
                IconSource::Symbol(Symbol::Grid),
                "Desktop icons",
            ),
        ],
    );
    let windows = SidebarNode::parent(
        Section::Windows,
        icons.windows.clone(),
        "Windows",
        vec![
            SidebarNode::leaf(
                Section::Layout,
                IconSource::Symbol(Symbol::Grid),
                "Default layout",
            ),
            SidebarNode::leaf(
                Section::Focus,
                IconSource::Symbol(Symbol::Controls),
                "Focus follows mouse",
            ),
            SidebarNode::leaf(Section::WorkingArea, icons.workspaces.clone(), "Workspaces"),
            SidebarNode::leaf(Section::Titlebar, icons.titlebar.clone(), "Decorations"),
            SidebarNode::leaf(Section::Effects, icons.eye.clone(), "Effects"),
            SidebarNode::leaf(
                Section::Unavailable("Window rules".into()),
                IconSource::Symbol(Symbol::Sliders),
                "Window rules",
            ),
        ],
    );
    let users_node = SidebarNode::parent(
        Section::Users,
        icons.users.clone(),
        "Users & Accounts",
        vec![
            SidebarNode::leaf(Section::Profile, profile_icon, "My profile"),
            SidebarNode::group(
                Section::Users,
                "Other accounts",
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
            SidebarNode::leaf(Section::CreateUser, icons.users.clone(), "Add user"),
        ],
    );
    vec![
        SidebarNode::group(
            Section::GroupNetwork,
            "Network",
            vec![
                SidebarNode::parent(
                    Section::Connectivity,
                    icons.status.clone(),
                    "Connectivity",
                    vec![SidebarNode::leaf(
                        Section::Unavailable("Connectivity".into()),
                        icons.status.clone(),
                        "Not available yet",
                    )],
                ),
                SidebarNode::parent(
                    Section::Hardware,
                    icons.devices.clone(),
                    "Hardware",
                    vec![
                        SidebarNode::leaf(Section::Keyboard, icons.keyboard.clone(), "Keyboard"),
                        SidebarNode::leaf(Section::Mouse, icons.mouse.clone(), "Mouse & touchpad"),
                        SidebarNode::leaf(
                            Section::Touchpad,
                            IconSource::Symbol(Symbol::Controls),
                            "Touchpad",
                        ),
                        SidebarNode::leaf(
                            Section::Unavailable("Displays & sound".into()),
                            IconSource::Symbol(Symbol::Display),
                            "Displays & sound",
                        ),
                    ],
                ),
            ],
        ),
        SidebarNode::group(
            Section::GroupLookAndFeel,
            "Look & Feel",
            vec![personalization, desktop, windows],
        ),
        SidebarNode::group(
            Section::GroupWorkflow,
            "Workflow",
            vec![
                SidebarNode::parent(
                    Section::ShortcutsCategory,
                    icons.shortcuts.clone(),
                    "Shortcuts",
                    vec![SidebarNode::leaf(
                        Section::Shortcuts,
                        icons.shortcuts.clone(),
                        "Keyboard shortcuts",
                    )],
                ),
                SidebarNode::parent(
                    Section::Applications,
                    IconSource::Symbol(Symbol::Grid),
                    "Applications",
                    vec![SidebarNode::leaf(
                        Section::Unavailable("Applications".into()),
                        IconSource::Symbol(Symbol::Grid),
                        "Not available yet",
                    )],
                ),
            ],
        ),
        SidebarNode::group(
            Section::GroupPeople,
            "People",
            vec![
                users_node,
                SidebarNode::parent(
                    Section::Privacy,
                    IconSource::Symbol(Symbol::Controls),
                    "Privacy & Security",
                    vec![SidebarNode::leaf(
                        Section::Unavailable("Privacy & Security".into()),
                        IconSource::Symbol(Symbol::Controls),
                        "Not available yet",
                    )],
                ),
                SidebarNode::parent(
                    Section::Accessibility,
                    IconSource::Symbol(Symbol::Appearance),
                    "Accessibility",
                    vec![SidebarNode::leaf(
                        Section::Unavailable("Accessibility".into()),
                        IconSource::Symbol(Symbol::Appearance),
                        "Not available yet",
                    )],
                ),
            ],
        ),
        SidebarNode::group(
            Section::GroupSystem,
            "System",
            vec![
                SidebarNode::parent(
                    Section::System,
                    icons.system.clone(),
                    "System",
                    vec![SidebarNode::leaf(
                        Section::Unavailable("System".into()),
                        icons.system.clone(),
                        "Not available yet",
                    )],
                ),
                SidebarNode::parent(
                    Section::About,
                    IconSource::Symbol(Symbol::Display),
                    "About",
                    vec![SidebarNode::leaf(
                        Section::Unavailable("About".into()),
                        IconSource::Symbol(Symbol::Display),
                        "Not available yet",
                    )],
                ),
            ],
        ),
    ]
}

fn unavailable_page(name: &str) -> BoxedWidget {
    common::section(
        "",
        "",
        vec![common::group(vec![common::row(
            "Not available yet",
            Box::new(
                Text::secondary(format!(
                    "{name} is part of the Settings layout, but Coconut does not support it yet."
                ))
                .size(TextSize::Sm),
            ),
        )])],
    )
}

fn category_pages(pages: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={32.0} children={pages} />
    })
}

fn sidebar_item(
    current: &Section,
    section: Section,
    label: &str,
    icon: IconSource,
    color: creamui_theme::Color,
    view: &Signal<Section>,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let active = sections::native::parent(current) == section;
    let select = view.clone();
    let item = RawButton::new(
        Style {
            size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(36.0) },
            ..Default::default()
        },
        move || select.set(section.clone()),
    )
    .background(if active { theme.colors.selection_background } else { creamui_theme::Color::rgba(0, 0, 0, 0) })
    .hover_style(creamui_core::StateStyle::new().background(if active { theme.colors.selection_background } else { theme.colors.surface_hover }))
    .corner_radius(12.0)
    .child(Box::new(jsx! {
        <RawView style={Style {
            size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Percent(1.0) },
            align_items: Some(creamui_core::layout::AlignItems::Center),
            gap: creamui_core::layout::Size { width: LengthPercentage::Length(10.0), height: LengthPercentage::Length(0.0) },
            padding: creamui_core::layout::Rect { left: LengthPercentage::Length(8.0), right: LengthPercentage::Length(8.0), top: LengthPercentage::Length(0.0), bottom: LengthPercentage::Length(0.0) },
            ..Default::default()
        }}>
            {common::icon_badge(icon, color)}
            {Box::new(Text::new(label).font_size(13.0)) as BoxedWidget}
        </RawView>
    }));
    Box::new(item)
}

fn sidebar_group(label: &str, items: Vec<BoxedWidget>) -> BoxedWidget {
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={4.0}>
            {Box::new(Text::secondary(label).font_size(10.0).bold(true).padding_left(8.0)) as BoxedWidget}
            {Box::new(RawView::new(Style { flex_direction: FlexDirection::Column, ..Default::default() }).with_children(items)) as BoxedWidget}
        </Flex>
    })
}

fn settings_sidebar(
    current: &Section,
    view: &Signal<Section>,
    icons: &SettingsIcons,
) -> BoxedWidget {
    use creamui_theme::Color;
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={16.0} padding={12.0}>
            {sidebar_group("NETWORK", vec![
                sidebar_item(current, Section::Connectivity, "Connectivity", icons.status.clone(), Color::rgb(0, 153, 214), view),
                sidebar_item(current, Section::Hardware, "Hardware", icons.devices.clone(), Color::rgb(0, 173, 188), view),
            ])}
            {sidebar_group("LOOK & FEEL", vec![
                sidebar_item(current, Section::Personalization, "Personalization", icons.paintbrush.clone(), Color::rgb(193, 99, 190), view),
                sidebar_item(current, Section::Desktop, "Desktop", icons.wallpaper.clone(), Color::rgb(83, 132, 232), view),
                sidebar_item(current, Section::Windows, "Windows", icons.windows.clone(), Color::rgb(78, 125, 222), view),
            ])}
            {sidebar_group("WORKFLOW", vec![
                sidebar_item(current, Section::ShortcutsCategory, "Shortcuts", icons.shortcuts.clone(), Color::rgb(222, 126, 54), view),
                sidebar_item(current, Section::Applications, "Applications", icons.applications.clone(), Color::rgb(0, 177, 115), view),
            ])}
            {sidebar_group("PEOPLE", vec![
                sidebar_item(current, Section::Users, "Users & Accounts", icons.users.clone(), Color::rgb(213, 88, 91), view),
                sidebar_item(current, Section::Privacy, "Privacy & Security", icons.privacy.clone(), Color::rgb(54, 171, 107), view),
                sidebar_item(current, Section::Accessibility, "Accessibility", icons.accessibility.clone(), Color::rgb(0, 167, 187), view),
            ])}
            {sidebar_group("SYSTEM", vec![
                sidebar_item(current, Section::System, "System", icons.system.clone(), Color::rgb(77, 142, 229), view),
                sidebar_item(current, Section::About, "About", icons.about.clone(), Color::rgb(0, 148, 191), view),
            ])}
        </Flex>
    })
}

fn distribution_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|release| {
            release.lines().find_map(|line| {
                line.strip_prefix("ID=")
                    .map(|id| id.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| std::env::consts::OS.to_owned())
}

fn window_controls(
    window: &Rc<RefCell<Option<WindowHandle>>>,
    icons: &SettingsIcons,
    maximized: &Signal<bool>,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let buttons = [
        icons.minimize.clone(),
        icons.maximize.clone(),
        icons.close.clone(),
    ]
    .into_iter()
    .enumerate()
    .map(|(action, icon)| {
        let window = window.clone();
        let maximized = maximized.clone();
        Box::new(
            RawButton::new(
                Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(28.0),
                        height: Dimension::Length(28.0),
                    },
                    align_items: Some(creamui_core::layout::AlignItems::Center),
                    justify_content: Some(creamui_core::layout::JustifyContent::Center),
                    ..Default::default()
                },
                move || {
                    if let Some(handle) = window.borrow().as_ref() {
                        match action {
                            0 => handle.minimize(),
                            1 => {
                                let next = !maximized.peek();
                                handle.set_maximized(next);
                                maximized.set(next);
                            }
                            _ => handle.close(),
                        }
                    }
                },
            )
            .background(theme.colors.text_secondary.mix(theme.colors.surface, 0.88))
            .hover_style(
                creamui_core::StateStyle::new()
                    .background(theme.colors.text_secondary.mix(theme.colors.surface, 0.75)),
            )
            .corner_radius(14.0)
            .child(Box::new(
                Icon::new(icon, theme.colors.text_primary).size(16.0),
            )),
        ) as BoxedWidget
    })
    .collect();
    Box::new(
        RawView::new(Style {
            position: Position::Absolute,
            inset: creamui_core::layout::Rect {
                right: LengthPercentageAuto::Length(16.0),
                top: LengthPercentageAuto::Length(14.0),
                left: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            gap: creamui_core::layout::Size {
                width: LengthPercentage::Length(6.0),
                height: LengthPercentage::Length(0.0),
            },
            ..Default::default()
        })
        .with_children(buttons),
    )
}

pub fn run() {
    common::load_settings_fonts();
    let config = Signal::new(ShellConfig::load());
    let integrations = coconut_registry::detect();
    let appearance = Signal::new(load_system_appearance());
    let view = Signal::new(Section::Connectivity);
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
    let connectivity = sections::connectivity::State::new();
    let native = sections::native::State::new(integrations.settings.clone());
    let window_settings = sections::window::WindowState::load();
    let shortcut_settings = sections::window::ShortcutState::load();
    let content_scroll = ScrollController::new(0.0);
    let sidebar_scroll = ScrollController::new(0.0);
    let dock_scroll = ScrollController::new(0.0);
    let dock_tab = Signal::new(sections::general::DockTab::Display);
    let islands_detail: Signal<Option<&'static str>> = Signal::new(None);
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
                    connectivity_revision.get();
                    let current = view.get();
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
                        &content_scroll,
                        &sidebar_scroll,
                        &dock_scroll,
                        &dock_tab,
                        &islands_detail,
                        &settings_search,
                        &connectivity,
                        &maximized,
                        &native,
                    )
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
    integrations: &coconut_api::Registry,
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
    content_scroll: &ScrollController,
    sidebar_scroll: &ScrollController,
    dock_scroll: &ScrollController,
    dock_tab: &Signal<sections::general::DockTab>,
    islands_detail: &Signal<Option<&'static str>>,
    settings_search: &TextController,
    connectivity: &sections::connectivity::State,
    maximized: &Signal<bool>,
    native: &sections::native::State,
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
    let current = view.get();
    let connectivity_detail = current == Section::Connectivity && connectivity.showing_detail();
    let native_detail = sections::native::parent(&current) != current;
    let decorations = creamui_render::use_window_decorations();
    let show_page_header = !connectivity_detail && !native_detail;
    let account_list = users.get();
    let title = page_title(&current);
    let description = page_description(&current);
    let _ = (&sidebar_style, nav);
    let sidebar = settings_sidebar(&current, view, icons);

    let content_has_own_scroll = matches!(current, Section::Statusbar | Section::Dockbar);
    let content = match current.clone() {
        Section::Connectivity => sections::connectivity::build(
            size,
            integrations.network.clone(),
            integrations.bluetooth.clone(),
            connectivity,
            icons,
        ),
        Section::Hardware => sections::native::hardware(native, view),
        Section::Personalization => sections::native::personalization(
            native,
            config,
            appearance,
            view,
            sections::appearance::overview(
                appearance,
                window,
                appearance_accent_picker,
                appearance_custom_accent,
            ),
        ),
        Section::Desktop => sections::native::desktop(native, config, view),
        Section::Windows => sections::native::windows(native, view, window_settings),
        Section::Detail(ref page) => sections::native::detail(
            page,
            native,
            view,
            config,
            window_settings,
            appearance,
            window,
        ),
        Section::ShortcutsCategory => sections::window::build_shortcuts(size, shortcut_settings),
        Section::Applications => sections::native::applications(native, view),
        Section::Users => sections::native::accounts(native, &account_list, view),
        Section::Privacy => sections::native::privacy(native, view),
        Section::Accessibility => sections::native::accessibility(native, window_settings),
        Section::System => sections::native::system(native, view),
        Section::About => sections::native::about(native),
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
        Section::Statusbar => sections::general::build_bar_page(
            size,
            config,
            dock_scroll,
            dock_tab,
            sections::general::BarKind::Statusbar,
        ),
        Section::Dockbar => sections::general::build_bar_page(
            size,
            config,
            dock_scroll,
            dock_tab,
            sections::general::BarKind::Dockbar,
        ),
        Section::Tray => sections::tray::build(size, config),
        Section::Islands => sections::islands::build(size, config, islands_detail),
        Section::Layout => sections::window::build_layout(size, window_settings),
        Section::Titlebar => sections::window::build_titlebar(size, window_settings),
        Section::Compositor => sections::window::build_general(size, window_settings),
        Section::WorkingArea => sections::window::build_working_area(size, window_settings),
        Section::Effects => sections::window::build_effects(size, window_settings),
        Section::Keyboard => sections::window::build_keyboard(size, window_settings),
        Section::Mouse => category_pages(vec![
            sections::window::build_mouse(size, window_settings),
            sections::window::build_touchpad(size, window_settings),
        ]),
        Section::CursorTheme => sections::asset_packs::cursor_themes(size, config),
        Section::Touchpad => sections::window::build_touchpad(size, window_settings),
        Section::Focus => sections::window::build_focus(size, window_settings),
        Section::Shortcuts => sections::window::build_shortcuts(size, shortcut_settings),
        Section::Profile => users::build(size, profile),
        Section::User(ref username) => users::account_view(size, &account_list, username),
        Section::CreateUser => users::create_user_view(size),
        Section::Unavailable(ref name) => unavailable_page(name),
        Section::GroupNetwork
        | Section::GroupLookAndFeel
        | Section::GroupWorkflow
        | Section::GroupPeople
        | Section::GroupSystem => unreachable!("navigation group is not selectable"),
    };

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
    let category_icon = match &current {
        Section::Connectivity => icons.status.clone(),
        Section::Hardware => icons.devices.clone(),
        Section::Personalization => icons.paintbrush.clone(),
        Section::Desktop => icons.wallpaper.clone(),
        Section::Windows => icons.windows.clone(),
        Section::ShortcutsCategory => icons.shortcuts.clone(),
        Section::Users => icons.users.clone(),
        Section::Applications => icons.applications.clone(),
        Section::Privacy => icons.privacy.clone(),
        Section::Accessibility => icons.accessibility.clone(),
        Section::System => icons.system.clone(),
        Section::About => icons.about.clone(),
        _ => IconSource::Symbol(Symbol::Controls),
    };
    let category_color = match current {
        Section::Connectivity => creamui_theme::Color::rgb(0, 153, 214),
        Section::Hardware => creamui_theme::Color::rgb(0, 173, 188),
        Section::Personalization => creamui_theme::Color::rgb(193, 99, 190),
        Section::Desktop | Section::Windows => creamui_theme::Color::rgb(83, 132, 232),
        Section::ShortcutsCategory | Section::Shortcuts => creamui_theme::Color::rgb(207, 91, 36),
        _ => theme.colors.accent,
    };
    let page_header: BoxedWidget = Box::new(jsx! {
        <Flex direction={FlexDirection::Row} style={page_header_style} gap={16.0} align={creamui_widgets::layout::Align::Center}>
            <Flex size={(56.0, 56.0)} align={creamui_widgets::layout::Align::Center} justify={creamui_widgets::layout::Justify::Center} background={creamui_core::LinearGradient::new(180.0, category_color.mix(creamui_theme::Color::rgb(255, 255, 255), 0.22), category_color)} corner_radius={16.0}>
                {Box::new(Icon::new(category_icon, theme.colors.selection_text).size(28.0)) as BoxedWidget}
            </Flex>
            <Flex direction={FlexDirection::Column} gap={4.0} justify={creamui_widgets::layout::Justify::Center}>
                {Box::new(Heading::lg(title).font_size(24.0)) as BoxedWidget}
                {Box::new(Text::secondary(description).font_size(14.0)) as BoxedWidget}
            </Flex>
        </Flex>
    });
    let panel_content: BoxedWidget = if content_has_own_scroll {
        let header = if native_detail {
            sections::native::detail_header(&current, view, size.width)
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
                        // `PageContainer` in cnpt is `max-w-3xl mx-auto`.
                        // The cap prevents cards from becoming billboard-wide
                        // on a large monitor.
                        size: creamui_core::layout::Size {
                            width: Dimension::Percent(1.0),
                            height: Dimension::Auto,
                        },
                        max_size: creamui_core::layout::Size {
                            width: Dimension::Length(768.0),
                            height: Dimension::Auto,
                        },
                        align_self: Some(creamui_core::layout::AlignSelf::Center),
                        gap: creamui_core::layout::Size {
                            width: LengthPercentage::Length(0.0),
                            height: LengthPercentage::Length(32.0),
                        },
                        padding: creamui_core::layout::Rect {
                            // Matches the mock's `px-10`: it remains roomy
                            // on wide windows without starving the content
                            // column on a smaller display.
                            left: LengthPercentage::Length(40.0),
                            right: LengthPercentage::Length(40.0),
                            top: LengthPercentage::Length(if show_page_header {
                                0.0
                            } else if connectivity_detail
                                && matches!(
                                    connectivity.view.get(),
                                    sections::connectivity::View::KnownNetworks
                                        | sections::connectivity::View::NearbyNetworks
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
                sections::connectivity::detail_header(connectivity, size.width)
            } else {
                sections::native::detail_header(&current, view, size.width)
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
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} size={(size.width, size.height)} background={window_surface}>
            <RawView style={sidebar_shell_style}>
                {sidebar_header}
                {Box::new(creamui_widgets::RawScrollView::controlled(Style { flex_grow: 1.0, min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Length(0.0) }, ..Default::default() }, sidebar_scroll.clone()).scrollbar(false).child(sidebar)) as BoxedWidget}
                {Box::new(RawView::new(Style { position: Position::Absolute, inset: creamui_core::layout::Rect { left: LengthPercentageAuto::Auto, right: LengthPercentageAuto::Length(0.0), top: LengthPercentageAuto::Length(0.0), bottom: LengthPercentageAuto::Length(0.0) }, size: creamui_core::layout::Size { width: Dimension::Length(1.0), height: Dimension::Percent(1.0) }, ..Default::default() }).background(theme.colors.border)) as BoxedWidget}
            </RawView>
            <RawView style={main_style}>
                {panel_content}
            </RawView>
            {drag_area}
            {sidebar_drag_area}
            {controls}
        </Flex>
    })
}
