pub(crate) mod destination;
use crate::views::native::Page;
use creamui_router::{Router, RouterError};
use destination::Section;

pub(crate) const DEFAULT_ROUTE: &str = "/connectivity";

pub(crate) fn initial_route(argument: Option<&str>) -> String {
    argument
        .map(|path| {
            if path.starts_with('/') {
                path.to_owned()
            } else {
                format!("/{path}")
            }
        })
        .unwrap_or_else(|| DEFAULT_ROUTE.into())
}

pub(crate) fn segment(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}

pub(crate) fn navigate(router: &Router, destination: &Section) {
    open(router, &destination.path());
}

pub(crate) fn open(router: &Router, path: &str) {
    if let Err(error) = router.navigate(path) {
        eprintln!("settings: navigation failed: {error}");
    }
}

pub(crate) fn create(initial: &str) -> Result<Router, RouterError> {
    let mut builder = Router::memory(initial).dev_override(false);
    for (pattern, _, render) in routes() {
        builder = builder.route(pattern, render);
    }
    builder
        .route("/", crate::views::connectivity::route_view)
        .not_found(|| crate::views::not_found::route_view("Page not found"))
        .build()
}

pub(crate) fn current(router: &Router) -> Section {
    let Some(matched) = router.current_match() else {
        return Section::Unavailable("Page not found".into());
    };
    if matched.pattern == "/" {
        return Section::Connectivity;
    }
    routes()
        .into_iter()
        .find(|(path, _, _)| *path == matched.pattern)
        .map(|(_, destination, _)| match destination {
            Section::User(_) => {
                Section::User(router.params().remove("username").unwrap_or_default())
            }
            Section::Detail(Page::App(_)) => Section::Detail(Page::App(
                router.params().remove("app_id").unwrap_or_default(),
            )),
            other => other,
        })
        .unwrap_or(Section::Connectivity)
}

type RouteDefinition = (&'static str, Section, fn() -> creamui_core::BoxedWidget);
fn routes() -> Vec<RouteDefinition> {
    vec![
        (
            "/connectivity",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/wifi/known",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/wifi/nearby",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/wifi/:ssid",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/bluetooth",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/bluetooth/:address",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/ethernet",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/vpn",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/connectivity/network",
            Section::Connectivity,
            crate::views::connectivity::route_view,
        ),
        (
            "/hardware",
            Section::Hardware,
            crate::views::hardware::overview,
        ),
        (
            "/personalization",
            Section::Personalization,
            crate::views::personalization::overview,
        ),
        (
            "/desktop",
            Section::Desktop,
            crate::views::desktop::overview,
        ),
        (
            "/desktop/widgets/:widget_id",
            Section::Islands,
            crate::views::desktop::islands::route_view,
        ),
        (
            "/windows",
            Section::Windows,
            crate::views::windows::overview,
        ),
        (
            "/shortcuts",
            Section::ShortcutsCategory,
            crate::views::shortcuts::overview,
        ),
        (
            "/applications",
            Section::Applications,
            crate::views::applications::overview,
        ),
        ("/users", Section::Users, crate::views::users::overview),
        (
            "/privacy",
            Section::Privacy,
            crate::views::privacy::overview,
        ),
        (
            "/accessibility",
            Section::Accessibility,
            crate::views::accessibility::overview,
        ),
        ("/system", Section::System, crate::views::system::overview),
        ("/about", Section::About, crate::views::about::overview),
        (
            "/personalization/icons",
            Section::IconPack,
            crate::views::personalization::asset_packs::icons_view,
        ),
        (
            "/personalization/sounds",
            Section::Sound,
            crate::views::personalization::asset_packs::sounds_view,
        ),
        (
            "/personalization/wallpaper",
            Section::Wallpaper,
            crate::views::personalization::wallpaper::route_view,
        ),
        (
            "/desktop/icons",
            Section::DesktopIcons,
            crate::views::desktop::icons::route_view,
        ),
        (
            "/desktop/statusbar",
            Section::Statusbar,
            crate::views::desktop::bars::statusbar_view,
        ),
        (
            "/desktop/dock",
            Section::Dockbar,
            crate::views::desktop::bars::dock_view,
        ),
        (
            "/desktop/tray",
            Section::Tray,
            crate::views::desktop::tray::route_view,
        ),
        (
            "/desktop/widgets",
            Section::Islands,
            crate::views::desktop::islands::route_view,
        ),
        (
            "/windows/layout",
            Section::Layout,
            crate::views::windows::layout_view,
        ),
        (
            "/windows/titlebar",
            Section::Titlebar,
            crate::views::windows::titlebar_view,
        ),
        (
            "/windows/advanced",
            Section::Compositor,
            crate::views::windows::advanced_view,
        ),
        (
            "/windows/workspaces",
            Section::WorkingArea,
            crate::views::windows::workspaces_view,
        ),
        (
            "/windows/effects",
            Section::Effects,
            crate::views::windows::effects_view,
        ),
        (
            "/hardware/keyboard",
            Section::Keyboard,
            crate::views::hardware::keyboard::route_view,
        ),
        (
            "/hardware/mouse",
            Section::Mouse,
            crate::views::hardware::mouse::route_view,
        ),
        (
            "/personalization/cursor",
            Section::CursorTheme,
            crate::views::personalization::asset_packs::cursor_view,
        ),
        (
            "/hardware/touchpad",
            Section::Touchpad,
            crate::views::hardware::touchpad::route_view,
        ),
        (
            "/windows/focus",
            Section::Focus,
            crate::views::windows::focus_view,
        ),
        (
            "/shortcuts/keyboard",
            Section::Shortcuts,
            crate::views::shortcuts::keyboard_view,
        ),
        (
            "/users/me",
            Section::Profile,
            crate::views::users::profile::route_view,
        ),
        (
            "/users/:username",
            Section::User(String::new()),
            crate::views::users::account::route_view,
        ),
        (
            "/hardware/displays",
            Section::Detail(Page::Displays),
            || crate::views::native::detail_view(&Page::Displays),
        ),
        ("/hardware/devices", Section::Detail(Page::Devices), || {
            crate::views::native::detail_view(&Page::Devices)
        }),
        (
            "/hardware/printers",
            Section::Detail(Page::Printers),
            || crate::views::native::detail_view(&Page::Printers),
        ),
        (
            "/hardware/printers/new",
            Section::Detail(Page::AddPrinter),
            || crate::views::native::detail_view(&Page::AddPrinter),
        ),
        (
            "/personalization/fonts",
            Section::Detail(Page::Fonts),
            || crate::views::native::detail_view(&Page::Fonts),
        ),
        (
            "/applications/installed",
            Section::Detail(Page::InstalledApps),
            || crate::views::native::detail_view(&Page::InstalledApps),
        ),
        (
            "/applications/installed/:app_id",
            Section::Detail(Page::App(String::new())),
            || {
                crate::views::native::detail_view(&Page::App(
                    creamui_router::use_params()
                        .remove("app_id")
                        .unwrap_or_default(),
                ))
            },
        ),
        (
            "/applications/file-types",
            Section::Detail(Page::FileTypes),
            || crate::views::native::detail_view(&Page::FileTypes),
        ),
        (
            "/applications/startup",
            Section::Detail(Page::Startup),
            || crate::views::native::detail_view(&Page::Startup),
        ),
        (
            "/applications/startup/new",
            Section::Detail(Page::AddStartup),
            || crate::views::native::detail_view(&Page::AddStartup),
        ),
        (
            "/users/me/password",
            Section::Detail(Page::Password),
            || crate::views::native::detail_view(&Page::Password),
        ),
        ("/users/new", Section::Detail(Page::AddUser), || {
            crate::views::native::detail_view(&Page::AddUser)
        }),
        (
            "/users/online-accounts",
            Section::Detail(Page::OnlineAccounts),
            || crate::views::native::detail_view(&Page::OnlineAccounts),
        ),
        (
            "/privacy/fingerprint",
            Section::Detail(Page::Fingerprint),
            || crate::views::native::detail_view(&Page::Fingerprint),
        ),
        ("/privacy/camera", Section::Detail(Page::Camera), || {
            crate::views::native::detail_view(&Page::Camera)
        }),
        (
            "/privacy/microphone",
            Section::Detail(Page::Microphone),
            || crate::views::native::detail_view(&Page::Microphone),
        ),
        (
            "/desktop/notifications",
            Section::Detail(Page::Notifications),
            || crate::views::native::detail_view(&Page::Notifications),
        ),
        (
            "/desktop/hot-corners",
            Section::Detail(Page::HotCorners),
            || crate::views::native::detail_view(&Page::HotCorners),
        ),
        ("/windows/rules", Section::Detail(Page::WindowRules), || {
            crate::views::native::detail_view(&Page::WindowRules)
        }),
        ("/system/updates", Section::Detail(Page::Updates), || {
            crate::views::native::detail_view(&Page::Updates)
        }),
        ("/system/storage", Section::Detail(Page::Storage), || {
            crate::views::native::detail_view(&Page::Storage)
        }),
        ("/system/activity", Section::Detail(Page::Activity), || {
            crate::views::native::detail_view(&Page::Activity)
        }),
        ("/system/date-time", Section::Detail(Page::DateTime), || {
            crate::views::native::detail_view(&Page::DateTime)
        }),
        ("/system/language", Section::Detail(Page::Language), || {
            crate::views::native::detail_view(&Page::Language)
        }),
        ("/system/reset", Section::Detail(Page::Reset), || {
            crate::views::native::detail_view(&Page::Reset)
        }),
    ]
}

impl Section {
    pub(crate) fn path(&self) -> String {
        match self {
            Self::User(username) => format!("/users/{}", segment(username)),
            Self::Detail(Page::App(id)) => format!("/applications/installed/{}", segment(id)),
            Self::CreateUser => "/users/new".into(),
            Self::Unavailable(_) => "/not-found".into(),
            other => routes()
                .into_iter()
                .find(|(_, destination, _)| destination == other)
                .map(|(path, _, _)| path.into())
                .expect("registered settings destination"),
        }
    }
}

pub(crate) fn set_query(router: &Router, key: &str, value: &str) {
    let mut location = router.location();
    let mut pairs = location.query_params();
    pairs.retain(|(existing, _)| existing != key);
    pairs.push((key.into(), value.into()));
    location.search = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish();
    if let Err(error) = router.navigate(&location.url()) {
        eprintln!("settings: navigation failed: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argument_selects_the_initial_route_with_query() {
        assert_eq!(initial_route(None), DEFAULT_ROUTE);
        let router = create(&initial_route(Some("desktop/dock?tab=contents"))).unwrap();
        assert_eq!(router.location().query("tab").as_deref(), Some("contents"));
        assert!(current(&router) == Section::Dockbar);
        assert!(current(&create("/users/me").unwrap()) == Section::Profile);
        assert!(create("https://example.com/settings").is_err());
    }

    #[test]
    fn destinations_round_trip_through_registered_routes() {
        for (_, destination, _) in routes() {
            let destination = match destination {
                Section::User(_) => Section::User("sample-user".into()),
                Section::Detail(Page::App(_)) => {
                    Section::Detail(Page::App("org.example.App.desktop".into()))
                }
                other => other,
            };
            let router = create(&destination.path()).unwrap();
            assert!(current(&router) == destination);
        }
        for destination in [
            Section::User("sam my/%ñ".into()),
            Section::Detail(Page::App("org.example/App %ñ.desktop".into())),
        ] {
            let router = create(&destination.path()).unwrap();
            assert!(current(&router) == destination);
        }
        assert!(current(&create("/users/new").unwrap()) == Section::Detail(Page::AddUser));
    }

    #[test]
    fn tabs_preserve_other_parameters_and_follow_history() {
        let router = create("/desktop/dock?filter=hello%20world&tab=position#preview").unwrap();
        set_query(&router, "tab", "appearance");
        assert_eq!(
            router.location().query("filter").as_deref(),
            Some("hello world")
        );
        assert_eq!(router.location().hash, "preview");
        assert_eq!(
            router.location().query("tab").as_deref(),
            Some("appearance")
        );
        router.back().unwrap();
        assert_eq!(router.location().query("tab").as_deref(), Some("position"));
        router.forward().unwrap();
        assert_eq!(
            router.location().query("tab").as_deref(),
            Some("appearance")
        );
        open(&router, "/desktop/statusbar");
        assert!(router.location().query("tab").is_none());
    }

    #[test]
    fn connectivity_and_widgets_have_direct_routes() {
        for path in [
            "/connectivity/wifi/known",
            "/connectivity/wifi/nearby",
            "/connectivity/wifi/Home%20Network",
            "/connectivity/bluetooth/AA%3ABB%3ACC",
            "/connectivity/ethernet",
            "/connectivity/vpn",
            "/desktop/widgets/clock",
        ] {
            assert!(create(path).unwrap().current_match().is_some(), "{path}");
        }
        assert!(create("/missing").unwrap().current_match().is_none());
    }
}
