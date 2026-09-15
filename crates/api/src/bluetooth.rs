/// One device known to the Bluetooth adapter, whether paired or only just
/// discovered by a scan.
#[derive(Clone)]
pub struct BluetoothDevice {
    pub address: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
    pub trusted: bool,
    /// The device's own reported battery level, when it exposes one (many
    /// headphones and mice do over `org.bluez.Battery1`).
    pub battery_percent: Option<u8>,
    /// A freedesktop icon-naming-spec hint the backend derives from the
    /// device's class/appearance (e.g. `"audio-headset"`, `"input-mouse"`),
    /// when it has one to offer.
    pub icon_hint: Option<String>,
    /// The raw Bluetooth Class of Device bitmask, when the device reports
    /// one — finer-grained than `icon_hint`, which collapses several minor
    /// device classes (e.g. a TV's "Set-top box"/"Video Display and
    /// Loudspeaker") down to the same generic icon.
    pub class: Option<u32>,
}

pub trait BluetoothIntegration {
    fn powered(&self) -> bool;
    fn connected(&self) -> bool;
    fn device_name(&self) -> Option<String>;
    fn set_powered(&self, powered: bool);
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }

    /// Every device the adapter currently knows about: paired devices plus
    /// whatever a scan in progress has discovered.
    fn devices(&self) -> Vec<BluetoothDevice> {
        Vec::new()
    }
    /// Whether the adapter is actively scanning for nearby devices.
    fn scanning(&self) -> bool {
        false
    }
    fn start_scan(&self) {}
    fn stop_scan(&self) {}
    /// Pairs (if needed) and connects to a device by its address.
    fn connect(&self, _address: &str) {}
    fn disconnect(&self, _address: &str) {}
    /// Unpairs a device and removes it from the adapter's known-device list.
    fn forget(&self, _address: &str) {}
}

pub struct Fallback;

impl BluetoothIntegration for Fallback {
    fn powered(&self) -> bool {
        false
    }
    fn connected(&self) -> bool {
        false
    }
    fn device_name(&self) -> Option<String> {
        None
    }
    fn set_powered(&self, _: bool) {}
}
