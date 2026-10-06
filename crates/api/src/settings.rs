//! System settings use explicit capabilities: an absent value is not a fake
//! default. Backends are Send + Sync because discovery and changes run off UI.
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Number(f64),
    Text(String),
}

#[derive(Clone, Debug, Default)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct Preference {
    pub value: Option<Value>,
    pub writable: bool,
    pub choices: Vec<Choice>,
    pub reason: String,
}

#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub properties: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub loaded: bool,
    pub session: String,
    pub preferences: BTreeMap<String, Preference>,
    pub collections: BTreeMap<String, Vec<Entry>>,
    pub facts: BTreeMap<String, String>,
    pub errors: Vec<String>,
}
impl Snapshot {
    pub fn entries(&self, key: &str) -> &[Entry] {
        self.collections
            .get(key)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    pub fn fact(&self, key: &str) -> &str {
        self.facts
            .get(key)
            .map(String::as_str)
            .unwrap_or("Unavailable")
    }
}

#[derive(Clone)]
pub enum Action {
    Set {
        key: String,
        value: Value,
    },
    SetDefaultApp {
        mime: String,
        desktop_id: String,
    },
    SetStartup {
        id: String,
        enabled: bool,
    },
    AddStartup {
        desktop_id: String,
    },
    SetCameraPermission {
        app: String,
        allowed: bool,
    },
    SetDevicePermission {
        device: String,
        app: String,
        allowed: bool,
    },
    AddPrinter {
        name: String,
        uri: String,
    },
    CheckUpdates,
    ChangePassword {
        password: String,
    },
    CreateUser {
        username: String,
        real_name: String,
        administrator: bool,
    },
}

pub trait SettingsIntegration: Send + Sync {
    fn snapshot(&self) -> Snapshot;
    fn apply(&self, action: Action) -> Result<String, String>;
}

pub struct Fallback;
impl SettingsIntegration for Fallback {
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            loaded: true,
            session: std::env::consts::OS.into(),
            ..Default::default()
        }
    }
    fn apply(&self, _: Action) -> Result<String, String> {
        Err("No system settings backend is available on this platform".into())
    }
}
