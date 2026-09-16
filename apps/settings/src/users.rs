use crate::common::{group, row, section};
use coconut_core::UserProfile;
use creamui_core::layout::Style;
use creamui_core::{BoxedWidget, Size, StateStyle, Style as WidgetStyle, Styled};
use creamui_theme::use_theme;
use creamui_widgets::layout::fixed;
use creamui_widgets::{
    IconImage, IconSource, RawButton, Select, SelectController, Text, TextController, TextInput,
    TextSize,
};
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::Connection;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

#[derive(Clone)]
pub struct Account {
    pub username: String,
    pub real_name: String,
    pub administrator: bool,
    pub icon_path: Option<PathBuf>,
}

/// The account's avatar, decoded from disk, or a filled circle with its
/// display name's initial when it has none set.
pub fn account_icon(account: &Account) -> IconSource {
    if let Some(image) = account
        .icon_path
        .as_deref()
        .and_then(|path| creamui_image::ImageData::from_path(path).ok())
    {
        return IconSource::Image(IconImage {
            width: image.width(),
            height: image.height(),
            rgba: Rc::from(image.pixels()),
            monochrome: false,
        });
    }
    let theme = use_theme();
    let display_name = if account.real_name.is_empty() {
        &account.username
    } else {
        &account.real_name
    };
    IconSource::Initial {
        letter: display_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_ascii_uppercase(),
        background: theme.accent,
        text_color: theme.selection_text,
    }
}

#[derive(Clone)]
pub struct ProfileControllers {
    first_name: TextController,
    last_name: TextController,
    city: TextController,
    region: TextController,
    country: TextController,
    latitude: Option<f64>,
    longitude: Option<f64>,
    country_select: SelectController,
    region_select: SelectController,
}

impl ProfileControllers {
    pub fn load(accounts: &[Account]) -> Self {
        let mut profile = UserProfile::load();
        if profile.display_name().is_empty() {
            if let Some(account) = accounts.iter().find(|account| {
                std::env::var("USER")
                    .ok()
                    .is_some_and(|username| username == account.username)
            }) {
                let mut names = account.real_name.splitn(2, ' ');
                profile.first_name = names.next().unwrap_or_default().to_owned();
                profile.last_name = names.next().unwrap_or_default().to_owned();
            }
        }
        let country_index = coconut_core::geo::countries("")
            .iter()
            .position(|item| item.code == profile.country)
            .unwrap_or(0);
        let region_index = coconut_core::geo::regions(&profile.country, "")
            .iter()
            .position(|item| item.code == profile.region)
            .unwrap_or(0);
        Self {
            first_name: TextController::new(profile.first_name),
            last_name: TextController::new(profile.last_name),
            city: TextController::new(profile.city),
            region: TextController::new(profile.region),
            country: TextController::new(profile.country),
            latitude: profile.latitude,
            longitude: profile.longitude,
            country_select: SelectController::new(country_index),
            region_select: SelectController::new(region_index),
        }
    }

    fn profile(&self) -> UserProfile {
        UserProfile {
            first_name: self.first_name.peek(),
            last_name: self.last_name.peek(),
            city: self.city.peek(),
            region: self.region.peek(),
            country: self.country.peek(),
            latitude: self.latitude,
            longitude: self.longitude,
        }
    }
}

pub fn build(_: Size, profile: &ProfileControllers) -> BoxedWidget {
    let own_profile = profile.clone();
    let save = Box::new(
        RawButton::new(button_style(), move || {
            let mut profile = own_profile.profile();
            let saved_profile = UserProfile::load();
            if profile.location() != saved_profile.location() {
                profile.latitude = None;
                profile.longitude = None;
            }
            if let Err(error) = profile.save() {
                eprintln!("settings: could not save profile: {error}");
                return;
            }
            if let Err(error) = coconut_core::ipc::publish_user_profile_changed() {
                eprintln!("settings: could not notify the desktop process about the profile update: {error}");
            }
            if let Err(error) = sync_own_account(&profile) {
                eprintln!("settings: could not update the system account: {error}");
            }
        })
        .child(Box::new(Text::new("Save profile").size(TextSize::Sm)) as BoxedWidget),
    ) as BoxedWidget;

    let avatar = avatar_button();
    let profile_group = group(vec![
        row("First name", input(&profile.first_name, "First name")),
        row("Last name", input(&profile.last_name, "Last name")),
        row("Avatar", avatar),
        row("City", input(&profile.city, "City")),
        row(
            "Country",
            country_search(&profile.country, &profile.country_select),
        ),
        row(
            "Region",
            region_search(&profile.country, &profile.region, &profile.region_select),
        ),
        row("", save),
    ]);

    section(
        "Users",
        "Your profile is used by the desktop. Location is saved for weather services.",
        vec![profile_group],
    )
}

pub fn account_view(_: Size, accounts: &[Account], username: &str) -> BoxedWidget {
    let Some(account) = accounts.iter().find(|account| account.username == username) else {
        return section("User", "This account is no longer available.", Vec::new());
    };
    let name = if account.real_name.is_empty() {
        account.username.clone()
    } else {
        account.real_name.clone()
    };
    section(
        &name,
        "Account details",
        vec![group(vec![
            row(
                "Username",
                Box::new(Text::secondary(account.username.clone()).size(TextSize::Sm)),
            ),
            row(
                "Account type",
                Box::new(
                    Text::secondary(if account.administrator {
                        "Administrator"
                    } else {
                        "Standard account"
                    })
                    .size(TextSize::Sm),
                ),
            ),
        ])],
    )
}

pub fn create_user_view(_: Size) -> BoxedWidget {
    section(
        "Create user",
        "Creating accounts will be available once the privileged account service is installed.",
        vec![group(vec![row(
            "Authorization",
            Box::new(Text::secondary("Requires an administrator").size(TextSize::Sm)),
        )])],
    )
}

fn input(controller: &TextController, placeholder: &str) -> BoxedWidget {
    Box::new(
        TextInput::controlled(controller)
            .placeholder(placeholder)
            .layout(Style {
                size: fixed(260.0, 34.0),
                ..Default::default()
            }),
    )
}

fn country_search(controller: &TextController, select: &SelectController) -> BoxedWidget {
    let matches = coconut_core::geo::countries("");
    let labels: Vec<_> = matches
        .iter()
        .map(|item| format!("{} — {}", item.name, item.code))
        .collect();
    let options: Vec<_> = labels.iter().map(String::as_str).collect();
    let value = controller.clone();
    Box::new(
        Select::controlled(&options, select.clone())
            .searchable()
            .on_select(move |index| {
                if let Some(item) = matches.get(index) {
                    value.set_value(item.code.clone());
                }
            }),
    )
}

fn region_search(
    country: &TextController,
    controller: &TextController,
    select: &SelectController,
) -> BoxedWidget {
    let matches = coconut_core::geo::regions(&country.value(), "");
    let labels: Vec<_> = matches
        .iter()
        .map(|item| format!("{} — {}", item.name, item.code))
        .collect();
    let options: Vec<_> = labels.iter().map(String::as_str).collect();
    let value = controller.clone();
    Box::new(
        Select::controlled(&options, select.clone())
            .searchable()
            .on_select(move |index| {
                if let Some(item) = matches.get(index) {
                    value.set_value(item.code.clone());
                }
            }),
    )
}

fn avatar_button() -> BoxedWidget {
    Box::new(
        RawButton::new(button_style(), move || {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Choose an avatar")
                .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
                .pick_file()
            {
                if let Err(error) = set_own_avatar(&path) {
                    eprintln!("settings: could not update avatar: {error}");
                }
            }
        })
        .child(Box::new(Text::new("Choose image").size(TextSize::Sm)) as BoxedWidget),
    )
}

fn button_style() -> WidgetStyle {
    let theme = use_theme();
    WidgetStyle::new()
        .layout(Style {
            size: fixed(124.0, 34.0),
            ..Default::default()
        })
        .background(theme.surface_elevated)
        .corner_radius(theme.button_radius)
        .hover(StateStyle::new().background(theme.surface_hover))
}

pub fn list_accounts() -> Vec<Account> {
    accounts_service().unwrap_or_else(fallback_accounts)
}

fn accounts_service() -> Option<Vec<Account>> {
    let connection = Connection::new_system().ok()?;
    let proxy = connection.with_proxy(
        "org.freedesktop.Accounts",
        "/org/freedesktop/Accounts",
        Duration::from_secs(2),
    );
    let (paths,): (Vec<dbus::Path<'static>>,) = proxy
        .method_call("org.freedesktop.Accounts", "ListCachedUsers", ())
        .ok()?;
    Some(
        paths
            .into_iter()
            .filter_map(|path| account_at(&connection, path))
            .filter(|account| !account.username.is_empty())
            .collect(),
    )
}

fn account_at(connection: &Connection, path: dbus::Path<'static>) -> Option<Account> {
    let proxy = connection.with_proxy("org.freedesktop.Accounts", path, Duration::from_secs(2));
    let (properties,): (PropMap,) = proxy
        .method_call(
            "org.freedesktop.DBus.Properties",
            "GetAll",
            ("org.freedesktop.Accounts.User",),
        )
        .ok()?;
    let system = properties
        .get("SystemAccount")
        .and_then(|value| value.0.as_i64())
        .is_some_and(|value| value != 0);
    (!system).then(|| {
        let icon_file = property_string(&properties, "IconFile");
        Account {
            username: property_string(&properties, "UserName"),
            real_name: property_string(&properties, "RealName"),
            administrator: properties
                .get("AccountType")
                .and_then(|value| value.0.as_i64())
                == Some(1),
            icon_path: (!icon_file.is_empty() && PathBuf::from(&icon_file).is_file())
                .then(|| PathBuf::from(icon_file)),
        }
    })
}

fn property_string(properties: &PropMap, key: &str) -> String {
    properties
        .get(key)
        .and_then(|value| value.0.as_str())
        .unwrap_or_default()
        .to_owned()
}

fn fallback_accounts() -> Vec<Account> {
    std::fs::read_to_string("/etc/passwd")
        .ok()
        .into_iter()
        .flat_map(|contents| contents.lines().map(str::to_owned).collect::<Vec<_>>())
        .filter_map(|line| {
            let fields: Vec<_> = line.split(':').collect();
            let uid = fields.get(2)?.parse::<u32>().ok()?;
            let shell = *fields.get(6)?;
            (uid >= 1000 && !shell.ends_with("nologin") && !shell.ends_with("false")).then(|| {
                Account {
                    username: fields[0].to_owned(),
                    real_name: fields[4].split(',').next().unwrap_or_default().to_owned(),
                    administrator: false,
                    icon_path: None,
                }
            })
        })
        .collect()
}

fn sync_own_account(profile: &UserProfile) -> Result<(), String> {
    let connection = Connection::new_system().map_err(|error| error.to_string())?;
    let path = own_account_path(&connection)?;
    let proxy = connection.with_proxy("org.freedesktop.Accounts", path, Duration::from_secs(10));
    let name = profile.display_name();
    if !name.is_empty() {
        proxy
            .method_call::<(), _, _, _>("org.freedesktop.Accounts.User", "SetRealName", (name,))
            .map_err(|error| error.to_string())?;
    }
    let location = profile.location();
    if !location.is_empty() {
        proxy
            .method_call::<(), _, _, _>("org.freedesktop.Accounts.User", "SetLocation", (location,))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn set_own_avatar(path: &PathBuf) -> Result<(), String> {
    let connection = Connection::new_system().map_err(|error| error.to_string())?;
    let account = own_account_path(&connection)?;
    connection
        .with_proxy("org.freedesktop.Accounts", account, Duration::from_secs(10))
        .method_call(
            "org.freedesktop.Accounts.User",
            "SetIconFile",
            (path.to_string_lossy().to_string(),),
        )
        .map_err(|error| error.to_string())
}

fn own_account_path(connection: &Connection) -> Result<dbus::Path<'static>, String> {
    let username = std::env::var("USER").map_err(|_| "current user is unavailable".to_owned())?;
    connection
        .with_proxy(
            "org.freedesktop.Accounts",
            "/org/freedesktop/Accounts",
            Duration::from_secs(10),
        )
        .method_call("org.freedesktop.Accounts", "FindUserByName", (username,))
        .map(|(path,): (dbus::Path<'static>,)| path)
        .map_err(|error| error.to_string())
}
