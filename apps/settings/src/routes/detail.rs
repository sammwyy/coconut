use crate::routes::destination::Section;

#[derive(Clone, PartialEq, Debug)]
pub enum Page {
    Displays,
    Devices,
    Printers,
    AddPrinter,
    Fonts,
    InstalledApps,
    App(String),
    FileTypes,
    Startup,
    AddStartup,
    Password,
    AddUser,
    OnlineAccounts,
    Fingerprint,
    Camera,
    Microphone,
    Notifications,
    HotCorners,
    WindowRules,
    Updates,
    Storage,
    Activity,
    DateTime,
    Language,
    Reset,
}
impl Page {
    pub fn title(&self) -> &str {
        match self {
            Self::Displays => "Displays",
            Self::Devices => "Connected devices",
            Self::Printers => "Printers",
            Self::AddPrinter => "Add printer",
            Self::Fonts => "Fonts",
            Self::InstalledApps => "Installed applications",
            Self::App(_) => "Application details",
            Self::FileTypes => "File types",
            Self::Startup => "Startup applications",
            Self::AddStartup => "Add startup application",
            Self::Password => "Change password",
            Self::AddUser => "Add user",
            Self::OnlineAccounts => "Online accounts",
            Self::Fingerprint => "Fingerprint",
            Self::Camera => "Camera permissions",
            Self::Microphone => "Microphone permissions",
            Self::Notifications => "Notifications & popups",
            Self::HotCorners => "Hot corners",
            Self::WindowRules => "Window rules",
            Self::Updates => "Updates",
            Self::Storage => "Storage",
            Self::Activity => "Activity monitor",
            Self::DateTime => "Date & time",
            Self::Language => "Language & region",
            Self::Reset => "Reset shell settings",
        }
    }
    pub(crate) fn parent(&self) -> Section {
        match self {
            Self::Displays | Self::Devices | Self::Printers | Self::AddPrinter => Section::Hardware,
            Self::Fonts => Section::Personalization,
            Self::InstalledApps
            | Self::App(_)
            | Self::FileTypes
            | Self::Startup
            | Self::AddStartup => Section::Applications,
            Self::Password | Self::AddUser | Self::OnlineAccounts => Section::Users,
            Self::Fingerprint | Self::Camera | Self::Microphone => Section::Privacy,
            Self::Notifications | Self::HotCorners => Section::Desktop,
            Self::WindowRules => Section::Windows,
            _ => Section::System,
        }
    }
}
