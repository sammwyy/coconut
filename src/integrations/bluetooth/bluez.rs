use super::BluetoothIntegration;
use crate::integrations::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::{
    ObjectManager, Properties, PropertiesPropertiesChanged,
};
use dbus::blocking::{Connection, SyncConnection};
use dbus::message::SignalArgs;
use dbus::Path;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// BlueZ exposes adapters and devices natively over the system D-Bus; this
/// reads them straight from there and reacts to its `PropertiesChanged`
/// signal instead of polling `bluetoothctl`.
const SERVICE: &str = "org.bluez";
const ADAPTER_INTERFACE: &str = "org.bluez.Adapter1";
const DEVICE_INTERFACE: &str = "org.bluez.Device1";

#[derive(Default)]
struct State {
    powered: bool,
    device_name: Option<String>,
}

pub struct BlueZ {
    state: Arc<Mutex<State>>,
    changes: ChangeListener,
    _bridge: EventBridgeGuard,
}

impl BlueZ {
    pub fn detect() -> Option<Self> {
        let connection = Connection::new_system().ok()?;
        adapter_path(&managed_objects(&connection)?)?;
        Some(Self::new())
    }

    fn new() -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let cache = state.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            if let Err(error) = run_event_bridge(cache, change_tx, shutdown_rx) {
                eprintln!("bluez: {error}");
            }
        });
        Self {
            state,
            changes,
            _bridge: bridge,
        }
    }
}

impl BluetoothIntegration for BlueZ {
    fn powered(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.powered)
            .unwrap_or(false)
    }

    fn connected(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.device_name.is_some())
            .unwrap_or(false)
    }

    fn device_name(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.device_name.clone())
    }

    fn set_powered(&self, powered: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.powered = powered;
            if !powered {
                state.device_name = None;
            }
        }
        std::thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            let Some(objects) = managed_objects(&connection) else {
                return;
            };
            let Some(path) = adapter_path(&objects) else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, path, Duration::from_secs(2));
            let _: Result<(), _> = proxy.set(ADAPTER_INTERFACE, "Powered", powered);
        });
    }

    fn changes(&self) -> Option<ChangeListener> {
        Some(self.changes.clone())
    }
}

type ManagedObjects = HashMap<Path<'static>, HashMap<String, PropMap>>;

fn managed_objects(connection: &Connection) -> Option<ManagedObjects> {
    connection
        .with_proxy(SERVICE, "/", Duration::from_secs(2))
        .get_managed_objects()
        .ok()
}

fn adapter_path(objects: &ManagedObjects) -> Option<Path<'static>> {
    objects
        .iter()
        .find(|(_, interfaces)| interfaces.contains_key(ADAPTER_INTERFACE))
        .map(|(path, _)| path.clone())
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
    let Some(objects) = connection
        .with_proxy(SERVICE, "/", Duration::from_secs(2))
        .get_managed_objects()
        .ok()
    else {
        return;
    };

    let powered = objects
        .values()
        .find_map(|interfaces| interfaces.get(ADAPTER_INTERFACE))
        .and_then(|props| props.get("Powered"))
        .and_then(|value| value.0.as_u64())
        .map(|value| value != 0)
        .unwrap_or(false);

    let device_name = objects.values().find_map(|interfaces| {
        let device = interfaces.get(DEVICE_INTERFACE)?;
        let connected = device
            .get("Connected")
            .and_then(|value| value.0.as_u64())
            .unwrap_or(0)
            != 0;
        if !connected {
            return None;
        }
        device
            .get("Name")
            .and_then(|value| value.0.as_str())
            .map(|name| name.to_owned())
    });

    let next = State {
        powered,
        device_name,
    };
    if let Ok(mut current) = state.lock() {
        *current = next;
    }
}

#[cfg(test)]
mod tests {
    use super::{BlueZ, BluetoothIntegration};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a running BlueZ system service"]
    fn reads_adapter_state_from_dbus() {
        let bluetooth = BlueZ::detect().expect("a BlueZ adapter");
        sleep(Duration::from_millis(200));
        let initial = bluetooth.powered();

        bluetooth.set_powered(!initial);
        sleep(Duration::from_millis(500));
        assert_eq!(bluetooth.powered(), !initial);

        bluetooth.set_powered(initial);
        sleep(Duration::from_millis(500));
        assert_eq!(bluetooth.powered(), initial);
    }

    #[test]
    #[ignore = "requires a running BlueZ system service and toggles the adapter's power"]
    fn reacts_to_an_external_power_toggle() {
        let bluetooth = BlueZ::detect().expect("a BlueZ adapter");
        sleep(Duration::from_millis(200));
        let initial = bluetooth.powered();
        let listener = bluetooth.changes().expect("a native change listener");
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = result_tx.send(listener.wait());
        });
        let _ = std::process::Command::new("bluetoothctl")
            .args(["power", if initial { "off" } else { "on" }])
            .status();
        assert_eq!(result_rx.recv_timeout(Duration::from_secs(10)), Ok(true));

        let _ = std::process::Command::new("bluetoothctl")
            .args(["power", if initial { "on" } else { "off" }])
            .status();
    }
}
