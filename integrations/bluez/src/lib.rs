use creamshell_api::bluetooth::{BluetoothDevice, BluetoothIntegration};
use creamshell_api::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use dbus::arg::{PropMap, RefArg};
use dbus::blocking::stdintf::org_freedesktop_dbus::{
    ObjectManager, ObjectManagerInterfacesAdded, ObjectManagerInterfacesRemoved, Properties,
    PropertiesPropertiesChanged,
};
use dbus::blocking::{Connection, SyncConnection};
use dbus::message::SignalArgs;
use dbus::Path;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// BlueZ exposes adapters and devices natively over the system D-Bus; this
/// reads them straight from there and reacts to its `PropertiesChanged` and
/// `InterfacesAdded`/`InterfacesRemoved` signals instead of polling
/// `bluetoothctl`.
const SERVICE: &str = "org.bluez";
const ADAPTER_INTERFACE: &str = "org.bluez.Adapter1";
const DEVICE_INTERFACE: &str = "org.bluez.Device1";
const BATTERY_INTERFACE: &str = "org.bluez.Battery1";

#[derive(Default)]
struct State {
    powered: bool,
    device_name: Option<String>,
    devices: Vec<BluetoothDevice>,
    scanning: bool,
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
        thread::spawn(move || {
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

    fn devices(&self) -> Vec<BluetoothDevice> {
        self.state
            .lock()
            .map(|state| state.devices.clone())
            .unwrap_or_default()
    }

    fn scanning(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.scanning)
            .unwrap_or(false)
    }

    fn start_scan(&self) {
        with_adapter(|proxy| {
            let _: Result<(), _> = proxy.method_call(ADAPTER_INTERFACE, "StartDiscovery", ());
        });
    }

    fn stop_scan(&self) {
        with_adapter(|proxy| {
            let _: Result<(), _> = proxy.method_call(ADAPTER_INTERFACE, "StopDiscovery", ());
        });
    }

    fn connect(&self, address: &str) {
        let address = address.to_owned();
        thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            let Some(path) = device_path_for(&connection, &address) else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, path, Duration::from_secs(30));
            let _: Result<(), _> = proxy.method_call(DEVICE_INTERFACE, "Connect", ());
        });
    }

    fn disconnect(&self, address: &str) {
        let address = address.to_owned();
        thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            let Some(path) = device_path_for(&connection, &address) else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, path, Duration::from_secs(10));
            let _: Result<(), _> = proxy.method_call(DEVICE_INTERFACE, "Disconnect", ());
        });
    }

    fn forget(&self, address: &str) {
        let address = address.to_owned();
        thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            let Some(objects) = managed_objects(&connection) else {
                return;
            };
            let Some(adapter) = adapter_path(&objects) else {
                return;
            };
            let Some(device) = device_path_for(&connection, &address) else {
                return;
            };
            let proxy = connection.with_proxy(SERVICE, adapter, Duration::from_secs(10));
            let _: Result<(), _> = proxy.method_call(ADAPTER_INTERFACE, "RemoveDevice", (device,));
        });
    }
}

/// Runs `action` against the system adapter on a background thread; used by
/// the fire-and-forget scan controls, whose resulting `Discovering` state
/// change is picked up reactively through the bridge's own signal match.
fn with_adapter(action: impl FnOnce(&dbus::blocking::Proxy<'_, &Connection>) + Send + 'static) {
    thread::spawn(move || {
        let Ok(connection) = Connection::new_system() else {
            return;
        };
        let Some(objects) = managed_objects(&connection) else {
            return;
        };
        let Some(path) = adapter_path(&objects) else {
            return;
        };
        let proxy = connection.with_proxy(SERVICE, path, Duration::from_secs(5));
        action(&proxy);
    });
}

fn device_path_for(connection: &Connection, address: &str) -> Option<Path<'static>> {
    let objects = managed_objects(connection)?;
    objects.into_iter().find_map(|(path, interfaces)| {
        let device = interfaces.get(DEVICE_INTERFACE)?;
        let matches = device.get("Address")?.0.as_str()? == address;
        matches.then_some(path)
    })
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

    let properties_changes = changes.clone();
    let properties_state = state.clone();
    let properties_rule =
        PropertiesPropertiesChanged::match_rule(Some(&sender), None).static_clone();
    let _properties_token = connection
        .add_match(
            properties_rule,
            move |_: PropertiesPropertiesChanged, connection, _| {
                refresh(connection, &properties_state);
                let _ = properties_changes.try_send(());
                true
            },
        )
        .map_err(|error| error.to_string())?;

    // Newly discovered devices arrive as `InterfacesAdded`/`InterfacesRemoved`
    // on the object manager, not as `PropertiesChanged`, so a scan needs its
    // own pair of match rules to populate the device list live.
    let added_changes = changes.clone();
    let added_state = state.clone();
    let added_rule = ObjectManagerInterfacesAdded::match_rule(Some(&sender), None).static_clone();
    let _added_token = connection
        .add_match(
            added_rule,
            move |_: ObjectManagerInterfacesAdded, connection, _| {
                refresh(connection, &added_state);
                let _ = added_changes.try_send(());
                true
            },
        )
        .map_err(|error| error.to_string())?;

    let removed_rule =
        ObjectManagerInterfacesRemoved::match_rule(Some(&sender), None).static_clone();
    let _removed_token = connection
        .add_match(
            removed_rule,
            move |_: ObjectManagerInterfacesRemoved, connection, _| {
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

    let scanning = objects
        .values()
        .find_map(|interfaces| interfaces.get(ADAPTER_INTERFACE))
        .and_then(|props| props.get("Discovering"))
        .and_then(|value| value.0.as_u64())
        .map(|value| value != 0)
        .unwrap_or(false);

    let mut devices: Vec<BluetoothDevice> = objects
        .values()
        .filter_map(|interfaces| {
            let device = interfaces.get(DEVICE_INTERFACE)?;
            let address = device.get("Address")?.0.as_str()?.to_owned();
            let name = device
                .get("Alias")
                .or_else(|| device.get("Name"))
                .and_then(|value| value.0.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| address.clone());
            let paired = device
                .get("Paired")
                .and_then(|value| value.0.as_u64())
                .map(|value| value != 0)
                .unwrap_or(false);
            let connected = device
                .get("Connected")
                .and_then(|value| value.0.as_u64())
                .map(|value| value != 0)
                .unwrap_or(false);
            let trusted = device
                .get("Trusted")
                .and_then(|value| value.0.as_u64())
                .map(|value| value != 0)
                .unwrap_or(false);
            let battery_percent = interfaces
                .get(BATTERY_INTERFACE)
                .and_then(|props| props.get("Percentage"))
                .and_then(|value| value.0.as_u64())
                .map(|value| value as u8);
            let icon_hint = device
                .get("Icon")
                .and_then(|value| value.0.as_str())
                .map(str::to_owned);
            let class = device
                .get("Class")
                .and_then(|value| value.0.as_u64())
                .map(|value| value as u32);
            Some(BluetoothDevice {
                address,
                name,
                paired,
                connected,
                trusted,
                battery_percent,
                icon_hint,
                class,
            })
        })
        .collect();
    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.paired.cmp(&a.paired))
            .then(
                a.name
                    .to_ascii_lowercase()
                    .cmp(&b.name.to_ascii_lowercase()),
            )
    });

    let device_name = devices
        .iter()
        .find(|device| device.connected)
        .map(|device| device.name.clone());

    let next = State {
        powered,
        device_name,
        devices,
        scanning,
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
