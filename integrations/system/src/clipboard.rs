use super::{boolean, unavailable, Connection, TIMEOUT};
use coconut_api::settings::{Choice, Entry, Preference, Snapshot, Value};

const SERVICE: &str = "org.coconut.Clipboard";
const PATH: &str = "/org/coconut/Clipboard1";
const INTERFACE: &str = "org.coconut.Clipboard1";
const KEYS: &[&str] = &[
    "clipboard-enabled",
    "clipboard-max-entries",
    "clipboard-max-age",
];

pub fn supports(key: &str) -> bool {
    KEYS.contains(&key)
}

fn is_permission(permission: &str) -> bool {
    matches!(
        permission,
        "clipboard-read" | "clipboard-history" | "clipboard-write"
    )
}

pub fn discover(snapshot: &mut Snapshot) {
    if let Err(error) = discover_service(snapshot) {
        snapshot
            .facts
            .insert("clipboard-service".into(), error.clone());
        for key in KEYS {
            snapshot
                .preferences
                .insert((*key).into(), unavailable(&error));
        }
    }
    let entries = || -> Result<Vec<Entry>, String> {
        let connection = Connection::new_session().map_err(|error| error.to_string())?;
        let (entries,): (Vec<(String, String, String, u64)>,) = connection
            .with_proxy(
                "org.coconut.Permissions",
                "/org/coconut/Permissions1",
                TIMEOUT,
            )
            .method_call("org.coconut.Permissions1", "List", ())
            .map_err(|error| error.to_string())?;
        Ok(entries
            .into_iter()
            .filter(|(_, permission, _, _)| is_permission(permission))
            .map(|(subject, permission, verdict, _)| Entry {
                name: subject.rsplit('/').next().unwrap_or(&subject).into(),
                description: format!("{permission} · {subject}"),
                id: subject,
                enabled: verdict == "allow",
                properties: [("permission".into(), permission)].into(),
            })
            .collect())
    };
    match entries() {
        Ok(entries) => {
            snapshot
                .collections
                .insert("clipboard-permissions".into(), entries);
        }
        Err(error) => {
            snapshot
                .facts
                .insert("clipboard-permission-service".into(), error);
        }
    }
}

fn choices(current: u64, options: &[(u64, &str)]) -> Preference {
    let mut choices: Vec<_> = options
        .iter()
        .map(|(value, label)| Choice {
            id: value.to_string(),
            label: (*label).into(),
        })
        .collect();
    if !options.iter().any(|(value, _)| *value == current) {
        choices.insert(
            0,
            Choice {
                id: current.to_string(),
                label: current.to_string(),
            },
        );
    }
    Preference {
        value: Some(Value::Text(current.to_string())),
        writable: true,
        choices,
        ..Default::default()
    }
}

fn discover_service(snapshot: &mut Snapshot) -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let proxy = connection.with_proxy(SERVICE, PATH, TIMEOUT);
    let (available, enabled, error, _): (bool, bool, String, u64) = proxy
        .method_call(INTERFACE, "Status", ())
        .map_err(|error| error.to_string())?;
    let (entries, _, _, age): (u32, u32, u32, u64) = proxy
        .method_call(INTERFACE, "GetConfig", ())
        .map_err(|error| error.to_string())?;
    snapshot.facts.insert(
        "clipboard-service".into(),
        if available {
            "Coconut clipboard service · text history stays in memory".into()
        } else {
            error
        },
    );
    snapshot
        .preferences
        .insert(KEYS[0].into(), boolean(enabled, true));
    snapshot.preferences.insert(
        KEYS[1].into(),
        choices(
            entries.into(),
            &[
                (20, "20 entries"),
                (50, "50 entries"),
                (100, "100 entries"),
                (200, "200 entries"),
                (1000, "1000 entries"),
            ],
        ),
    );
    snapshot.preferences.insert(
        KEYS[2].into(),
        choices(
            age,
            &[
                (3600, "1 hour"),
                (86400, "24 hours"),
                (604800, "7 days"),
                (2592000, "30 days"),
            ],
        ),
    );
    Ok(())
}

pub fn set(key: &str, value: Value) -> Result<(), String> {
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let proxy = connection.with_proxy(SERVICE, PATH, TIMEOUT);
    if key == KEYS[0] {
        let Value::Bool(enabled) = value else {
            return Err("Expected a collection toggle".into());
        };
        return proxy
            .method_call(INTERFACE, "SetEnabled", (enabled,))
            .map_err(|error| error.to_string());
    }
    let Value::Text(value) = value else {
        return Err("Expected a retention limit".into());
    };
    let value: u64 = value.parse().map_err(|_| "Invalid retention limit")?;
    let (entries, item, total, age): (u32, u32, u32, u64) = proxy
        .method_call(INTERFACE, "GetConfig", ())
        .map_err(|error| error.to_string())?;
    let (entries, age) = match key {
        "clipboard-max-entries" => (
            u32::try_from(value).map_err(|_| "Invalid entry limit")?,
            age,
        ),
        "clipboard-max-age" => (entries, value),
        _ => return Err("Unknown clipboard preference".into()),
    };
    proxy
        .method_call(INTERFACE, "Configure", (entries, item, total, age))
        .map_err(|error| error.to_string())
}

pub fn set_permission(
    subject: &str,
    permission: &str,
    allowed: Option<bool>,
) -> Result<(), String> {
    if !is_permission(permission) {
        return Err("Invalid clipboard permission".into());
    }
    let connection = Connection::new_session().map_err(|error| error.to_string())?;
    let proxy = connection.with_proxy(
        "org.coconut.Permissions",
        "/org/coconut/Permissions1",
        TIMEOUT,
    );
    let (applied,): (bool,) = match allowed {
        Some(allowed) => proxy.method_call(
            "org.coconut.Permissions1",
            "SetDecision",
            (subject, permission, if allowed { "allow" } else { "deny" }),
        ),
        None => proxy.method_call("org.coconut.Permissions1", "Revoke", (subject, permission)),
    }
    .map_err(|error| error.to_string())?;
    if applied {
        Ok(())
    } else {
        Err("Permission was not changed".into())
    }
}
