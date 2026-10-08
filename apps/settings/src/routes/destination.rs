use crate::icons::SettingsIcons;
use creamui_widgets::{IconSource, Symbol};

#[derive(Clone, PartialEq)]
pub(crate) enum Section {
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
    Privacy,
    Accessibility,
    About,
    Detail(crate::routes::detail::Page),
    Unavailable(String),
}

pub(crate) fn page_title(section: &Section) -> String {
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
    }
}

pub(crate) fn page_description(section: &Section) -> &'static str {
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
        Section::Profile | Section::User(_) => "Manage the people and accounts on this computer.",
        Section::Unavailable(_) => "This area is planned but is not supported by Coconut yet.",
        _ => "Customize your Coconut desktop.",
    }
}

pub(crate) fn section_presentation(
    section: &Section,
    icons: &SettingsIcons,
) -> (IconSource, creamui_theme::Color) {
    use creamui_theme::Color;

    match section {
        Section::Connectivity => (icons.status.clone(), Color::rgb(0, 153, 214)),
        Section::Hardware | Section::Keyboard | Section::Mouse | Section::Touchpad => {
            (icons.devices.clone(), Color::rgb(0, 173, 188))
        }
        Section::Personalization | Section::IconPack | Section::Sound | Section::CursorTheme => {
            (icons.paintbrush.clone(), Color::rgb(193, 99, 190))
        }
        Section::Wallpaper => (icons.paintbrush.clone(), Color::rgb(193, 99, 190)),
        Section::DesktopIcons
        | Section::Desktop
        | Section::Statusbar
        | Section::Dockbar
        | Section::Tray
        | Section::Islands => (icons.wallpaper.clone(), Color::rgb(110, 105, 224)),
        Section::Windows
        | Section::Layout
        | Section::Titlebar
        | Section::Compositor
        | Section::WorkingArea
        | Section::Effects
        | Section::Focus => (icons.windows.clone(), Color::rgb(78, 125, 222)),
        Section::ShortcutsCategory | Section::Shortcuts => {
            (icons.shortcuts.clone(), Color::rgb(222, 126, 54))
        }
        Section::Applications => (icons.applications.clone(), Color::rgb(0, 177, 115)),
        Section::Users | Section::Profile | Section::User(_) => {
            (icons.users.clone(), Color::rgb(213, 88, 91))
        }
        Section::Privacy => (icons.privacy.clone(), Color::rgb(54, 171, 107)),
        Section::Accessibility => (icons.accessibility.clone(), Color::rgb(0, 167, 187)),
        Section::System | Section::About => (icons.system.clone(), Color::rgb(77, 142, 229)),
        Section::Detail(_) => {
            section_presentation(&crate::routes::destination::parent(section), icons)
        }
        Section::Unavailable(_) => (
            IconSource::Symbol(Symbol::Controls),
            Color::rgb(110, 105, 224),
        ),
    }
}

pub(crate) fn parent(section: &Section) -> Section {
    match section {
        Section::Detail(page) => page.parent(),
        Section::IconPack | Section::Sound | Section::Wallpaper | Section::CursorTheme => {
            Section::Personalization
        }
        Section::DesktopIcons
        | Section::Statusbar
        | Section::Dockbar
        | Section::Tray
        | Section::Islands => Section::Desktop,
        Section::Layout
        | Section::Titlebar
        | Section::Compositor
        | Section::WorkingArea
        | Section::Effects
        | Section::Focus => Section::Windows,
        Section::Keyboard | Section::Mouse | Section::Touchpad => Section::Hardware,
        Section::Shortcuts => Section::ShortcutsCategory,
        Section::Profile | Section::User(_) => Section::Users,
        other => other.clone(),
    }
}
