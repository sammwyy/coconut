use creamshell_api::battery::BatteryIntegration;
use creamshell_api::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use dbus::blocking::stdintf::org_freedesktop_dbus::{Properties, PropertiesPropertiesChanged};
use dbus::blocking::{Connection, SyncConnection};
use dbus::message::SignalArgs;
use dbus::Path;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// UPower exposes battery state natively over the system D-Bus; this reads
/// it straight from there and reacts to its `PropertiesChanged` signal
/// instead of polling.
const SERVICE: &str = "org.freedesktop.UPower";
const DEVICE_INTERFACE: &str = "org.freedesktop.UPower.Device";
const BATTERY_KIND: u32 = 2;

#[derive(Default)]
struct State {
    percentage: Option<u8>,
    charging: bool,
}

pub struct UPower {
    state: Arc<Mutex<State>>,
    changes: ChangeListener,
    _bridge: EventBridgeGuard,
}

impl UPower {
    pub fn detect() -> Option<Self> {
        let connection = Connection::new_system().ok()?;
        let device = display_device(&connection)?;
        let proxy = connection.with_proxy(SERVICE, &device, Duration::from_secs(2));
        let kind: u32 = proxy.get(DEVICE_INTERFACE, "Type").ok()?;
        (kind == BATTERY_KIND).then(|| Self::new(device))
    }

    fn new(device: Path<'static>) -> Self {
        let state = Arc::new(Mutex::new(State::default()));
        let cache = state.clone();
        let (changes, bridge) = spawn_event_bridge(move |change_tx, shutdown_rx| {
            if let Err(error) = run_event_bridge(device, cache, change_tx, shutdown_rx) {
                eprintln!("upower: {error}");
            }
        });
        Self {
            state,
            changes,
            _bridge: bridge,
        }
    }
}

impl BatteryIntegration for UPower {
    fn percentage(&self) -> Option<u8> {
        self.state.lock().ok().and_then(|state| state.percentage)
    }

    fn charging(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.charging)
            .unwrap_or(false)
    }

    fn changes(&self) -> Option<ChangeListener> {
        Some(self.changes.clone())
    }
}

fn display_device(connection: &Connection) -> Option<Path<'static>> {
    let proxy = connection.with_proxy(SERVICE, "/org/freedesktop/UPower", Duration::from_secs(2));
    let (path,): (Path<'static>,) = proxy.method_call(SERVICE, "GetDisplayDevice", ()).ok()?;
    Some(path)
}

fn run_event_bridge(
    device: Path<'static>,
    state: Arc<Mutex<State>>,
    changes: SyncSender<()>,
    shutdown: Receiver<()>,
) -> Result<(), String> {
    let connection = SyncConnection::new_system().map_err(|error| error.to_string())?;
    refresh(&connection, &device, &state);

    let sender = dbus::strings::BusName::new(SERVICE).map_err(|error| error.to_string())?;
    let rule = PropertiesPropertiesChanged::match_rule(Some(&sender), None).static_clone();
    let _token = connection
        .add_match(
            rule,
            move |_: PropertiesPropertiesChanged, connection, _| {
                refresh(connection, &device, &state);
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

fn refresh(connection: &SyncConnection, device: &Path<'static>, state: &Arc<Mutex<State>>) {
    let proxy = connection.with_proxy(SERVICE, device, Duration::from_secs(2));
    let present = proxy
        .get::<bool>(DEVICE_INTERFACE, "IsPresent")
        .unwrap_or(true);
    let percentage = present.then(|| {
        proxy
            .get::<f64>(DEVICE_INTERFACE, "Percentage")
            .ok()
            .map(|value| value.round() as u8)
    });
    let raw_state = proxy.get::<u32>(DEVICE_INTERFACE, "State").unwrap_or(0);
    let next = State {
        percentage: percentage.flatten(),
        charging: matches!(raw_state, 1 | 4),
    };
    if let Ok(mut current) = state.lock() {
        *current = next;
    }
}

#[cfg(test)]
mod tests {
    use super::{BatteryIntegration, UPower};
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    #[ignore = "requires a running UPower system service"]
    fn reads_the_display_device_from_dbus() {
        let battery = UPower::detect().expect("a UPower battery device");
        sleep(Duration::from_millis(200));
        assert!(battery.percentage().is_some());
    }
}
