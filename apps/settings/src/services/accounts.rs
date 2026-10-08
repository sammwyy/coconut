use coconut_core::UserProfile;
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::Connection;
use std::path::PathBuf;
use std::time::Duration;

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

pub(crate) fn sync_own_account(profile: &UserProfile) -> Result<(), String> {
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

pub(crate) fn set_own_avatar(path: &PathBuf) -> Result<(), String> {
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

#[derive(Clone)]
pub struct Account {
    pub username: String,
    pub real_name: String,
    pub administrator: bool,
    pub icon_path: Option<PathBuf>,
}
