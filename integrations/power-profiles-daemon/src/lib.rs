use creamshell_api::power_profile::{PowerProfile, PowerProfileIntegration};
use creamshell_api::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::{Properties, PropertiesPropertiesChanged};
use dbus::blocking::{Connection, SyncConnection};
use dbus::message::SignalArgs;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// power-profiles-daemon exposes the system's power plans natively over the
/// system D-Bus; this reads them straight from there and reacts to its
/// `PropertiesChanged` signal instead of polling `powerprofilesctl`.
const SERVICE: &str = "net.hadess.PowerProfiles";
const PATH: &str = "/net/hadess/PowerProfiles";
const INTERFACE: &str = "net.hadess.PowerProfiles";

#[derive(Default)]
struct State {
    profiles: Vec<PowerProfile>,
}

pub struct PowerProfilesDaemon {
    state: Arc<Mutex<State>>,
    changes: ChangeListener,
    _bridge: EventBridgeGuard,
}

impl PowerProfilesDaemon {
    pub fn detect() -> Option<Self> {
        let connection = Connection::new_system().ok()?;
        let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
        proxy
            .get::<String>(INTERFACE, "ActiveProfile")
            .ok()
            .map(|_| Self::new())
    }

    fn new() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let cache = state.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            if let Err(error) = run_event_bridge(cache, change_tx, shutdown_rx) {
                eprintln!("power profiles daemon: {error}");
            }
        });
        Self {
            state,
            changes,
            _bridge: bridge,
        }
    }
}

impl PowerProfileIntegration for PowerProfilesDaemon {
    fn profiles(&self) -> Vec<PowerProfile> {
        self.state
            .lock()
            .map(|state| state.profiles.clone())
            .unwrap_or_default()
    }

    fn set_profile(&self, id: &str) {
        let id = id.to_owned();
        thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
            let _: Result<(), _> = proxy.set(INTERFACE, "ActiveProfile", id);
        });
    }

    fn changes(&self) -> Option<ChangeListener> {
        Some(self.changes.clone())
    }
}

fn run_event_bridge(
    state: Arc<Mutex<State>>,
    changes: SyncSender<()>,
    shutdown: Receiver<()>,
) -> Result<(), String> {
    let connection = SyncConnection::new_system().map_err(|error| error.to_string())?;
    refresh(&connection, &state);

    let sender = dbus::strings::BusName::new(SERVICE).map_err(|error| error.to_string())?;
    let rule = PropertiesPropertiesChanged::match_rule(Some(&sender), None).static_clone();
    let _token = connection
        .add_match(
            rule,
            move |_: PropertiesPropertiesChanged, connection, _| {
                refresh(connection, &state);
                let _ = changes.try_send(());
                true
            },
        )
        .map_err(|error| error.to_string())?;

    loop {
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => return Ok(()),
            Err(TryRecvError::Empty) => {}
        }
        connection
            .process(Duration::from_millis(500))
            .map_err(|error| error.to_string())?;
    }
}

fn refresh(connection: &SyncConnection, state: &Arc<Mutex<State>>) {
    let proxy = connection.with_proxy(SERVICE, PATH, Duration::from_secs(2));
    let Ok(active) = proxy.get::<String>(INTERFACE, "ActiveProfile") else {
        return;
    };
    let raw: Vec<PropMap> = proxy.get(INTERFACE, "Profiles").unwrap_or_default();
    let profiles = raw
        .into_iter()
        .filter_map(|entry| {
            let id = entry.get("Profile")?.0.as_str()?.to_owned();
            let profile_active = id == active;
            Some(PowerProfile {
                id,
                active: profile_active,
            })
        })
        .collect();
    if let Ok(mut current) = state.lock() {
        current.profiles = profiles;
    }
}

#[cfg(test)]
mod tests {
    use super::{PowerProfileIntegration, PowerProfilesDaemon};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a running power-profiles-daemon system service"]
    fn reads_profiles_from_dbus() {
        let power_profile =
            PowerProfilesDaemon::detect().expect("power-profiles-daemon over D-Bus");
        sleep(Duration::from_millis(200));
        let profiles = power_profile.profiles();
        assert!(!profiles.is_empty());
        assert!(profiles.iter().any(|profile| profile.active));
    }
}
