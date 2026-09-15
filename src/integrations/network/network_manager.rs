use super::NetworkIntegration;
use crate::integrations::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use dbus::blocking::stdintf::org_freedesktop_dbus::{Properties, PropertiesPropertiesChanged};
use dbus::blocking::{Connection, SyncConnection};
use dbus::message::SignalArgs;
use dbus::Path;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// NetworkManager exposes its state natively over the system D-Bus; this
/// reads it straight from there and reacts to its `PropertiesChanged`
/// signal instead of polling `nmcli`.
const SERVICE: &str = "org.freedesktop.NetworkManager";
const ROOT_PATH: &str = "/org/freedesktop/NetworkManager";
const ACTIVE_CONNECTION_INTERFACE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const ACCESS_POINT_INTERFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";
const WIRELESS_TYPE: &str = "802-11-wireless";
const NM_STATE_CONNECTED_LOCAL: u32 = 50;

#[derive(Default)]
struct State {
    connected: bool,
    enabled: bool,
    name: Option<String>,
    strength: Option<u8>,
}

pub struct NetworkManager {
    state: Arc<Mutex<State>>,
    changes: ChangeListener,
    _bridge: EventBridgeGuard,
}

impl NetworkManager {
    pub fn detect() -> Option<Self> {
        let connection = Connection::new_system().ok()?;
        let proxy = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(2));
        proxy.get::<u32>(SERVICE, "State").ok().map(|_| Self::new())
    }

    fn new() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let cache = state.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            if let Err(error) = run_event_bridge(cache, change_tx, shutdown_rx) {
                eprintln!("network manager: {error}");
            }
        });
        Self {
            state,
            changes,
            _bridge: bridge,
        }
    }
}

impl NetworkIntegration for NetworkManager {
    fn connected(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.connected)
            .unwrap_or(false)
    }

    fn network_name(&self) -> Option<String> {
        self.state.lock().ok().and_then(|state| state.name.clone())
    }

    fn strength(&self) -> Option<u8> {
        self.state.lock().ok().and_then(|state| state.strength)
    }

    fn enabled(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.enabled)
            .unwrap_or(false)
    }

    fn set_enabled(&self, enabled: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.enabled = enabled;
            if !enabled {
                state.connected = false;
                state.name = None;
                state.strength = None;
            }
        }
        std::thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(2));
            let _: Result<(), _> = proxy.set(SERVICE, "WirelessEnabled", enabled);
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
    let root = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(2));
    let raw_state = root.get::<u32>(SERVICE, "State").unwrap_or(0);
    let enabled = root
        .get::<bool>(SERVICE, "WirelessEnabled")
        .unwrap_or(false);
    let primary: Path<'static> = root
        .get(SERVICE, "PrimaryConnection")
        .unwrap_or_else(|_| Path::from("/"));

    let (name, strength) = active_connection_details(connection, &primary);

    let next = State {
        connected: raw_state >= NM_STATE_CONNECTED_LOCAL,
        enabled,
        name,
        strength,
    };
    if let Ok(mut current) = state.lock() {
        *current = next;
    }
}

fn active_connection_details(
    connection: &SyncConnection,
    primary: &Path<'static>,
) -> (Option<String>, Option<u8>) {
    let no_object = Path::from("/");
    if *primary == no_object {
        return (None, None);
    }
    let active = connection.with_proxy(SERVICE, primary, Duration::from_secs(2));
    let Ok(name) = active.get::<String>(ACTIVE_CONNECTION_INTERFACE, "Id") else {
        return (None, None);
    };
    let connection_type = active
        .get::<String>(ACTIVE_CONNECTION_INTERFACE, "Type")
        .unwrap_or_default();
    if connection_type != WIRELESS_TYPE {
        return (Some(name), None);
    }
    let access_point: Path<'static> = active
        .get(ACTIVE_CONNECTION_INTERFACE, "SpecificObject")
        .unwrap_or_else(|_| no_object.clone());
    if access_point == no_object {
        return (Some(name), None);
    }
    let access_point = connection.with_proxy(SERVICE, access_point, Duration::from_secs(2));
    let strength = access_point
        .get::<u8>(ACCESS_POINT_INTERFACE, "Strength")
        .ok();
    (Some(name), strength)
}

#[cfg(test)]
mod tests {
    use super::{NetworkIntegration, NetworkManager};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a running NetworkManager system service"]
    fn reads_wireless_state_from_dbus() {
        let network = NetworkManager::detect().expect("NetworkManager over D-Bus");
        sleep(Duration::from_millis(200));
        assert!(network.enabled());
        assert!(network.network_name().is_some());
    }

    #[test]
    #[ignore = "requires a running NetworkManager system service and observing an external change"]
    fn reacts_to_an_external_wifi_rescan() {
        let network = NetworkManager::detect().expect("NetworkManager over D-Bus");
        let listener = network.changes().expect("a native change listener");
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = result_tx.send(listener.wait());
        });
        let _ = std::process::Command::new("nmcli")
            .args(["device", "wifi", "rescan"])
            .status();
        assert_eq!(result_rx.recv_timeout(Duration::from_secs(15)), Ok(true));
    }
}
