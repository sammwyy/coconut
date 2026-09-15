#[cfg(not(target_os = "windows"))]
mod network_manager;
#[cfg(target_os = "windows")]
mod windows;

/// The kind of network interface a [`NetworkDevice`] represents. Every
/// interface is classified, including plumbing a panel may not render
/// (`Virtual`) — it's up to each panel to pick which kinds to show.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NetworkDeviceKind {
    Ethernet,
    Wifi,
    Vpn,
    /// A bond, VLAN, bridge, team, or veth interface — container/bonding
    /// plumbing, not an externally meaningful connection.
    Virtual,
    Other,
}

/// One network interface known to the system, with whatever connection is
/// currently active on it, if any. The IP details are only populated while
/// `active` is true — there's nothing to show for an idle interface.
#[derive(Clone)]
pub struct NetworkDevice {
    pub interface: String,
    pub kind: NetworkDeviceKind,
    pub connection_name: Option<String>,
    pub active: bool,
    pub ip_address: Option<String>,
    pub subnet_mask: Option<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    /// Whether the address came from DHCP (`true`) or a manual/static
    /// configuration (`false`).
    pub dhcp: bool,
}

/// A Wi-Fi network visible to the wireless adapter's most recent scan.
#[derive(Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    pub active: bool,
    /// The radio band it's broadcasting on ("2.4 GHz", "5 GHz", "6 GHz"),
    /// when the frequency reported is recognizable.
    pub band: Option<&'static str>,
    /// A human-readable security protocol label ("WPA2 Personal", "WPA3
    /// Personal", "WEP", "Open", ...).
    pub security: String,
}

pub trait NetworkIntegration {
    fn connected(&self) -> bool;
    fn network_name(&self) -> Option<String>;
    fn strength(&self) -> Option<u8>;
    fn enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool);
    /// A listener that wakes whenever this integration's native hook (a
    /// D-Bus signal, a platform event) observes a state change. `None` when
    /// the backend has no such hook, so callers fall back to polling.
    fn changes(&self) -> Option<super::ChangeListener> {
        None
    }

    /// Every network interface the system knows about (LAN, VPN, virtual
    /// interfaces such as a Tailscale mesh device, and the Wi-Fi adapter
    /// itself), with its active connection if any. Empty when the backend
    /// can't enumerate devices.
    fn devices(&self) -> Vec<NetworkDevice> {
        Vec::new()
    }
    /// Whether a Wi-Fi adapter is present at all.
    fn has_wifi_adapter(&self) -> bool {
        false
    }
    /// The wireless adapter's most recent scan results.
    fn wifi_networks(&self) -> Vec<WifiNetwork> {
        Vec::new()
    }
    /// Whether a Wi-Fi scan is currently in flight.
    fn scanning(&self) -> bool {
        false
    }
    /// Asks the wireless adapter to rescan for nearby networks.
    fn rescan(&self) {}
    /// Connects to a visible Wi-Fi network: reuses a saved profile for that
    /// SSID when one exists, otherwise creates one on the fly if the
    /// network is open. A secured network with no saved profile is a no-op.
    fn connect_wifi(&self, _ssid: &str) {}
    /// Deletes the saved profile for a Wi-Fi network, if one exists.
    fn forget_wifi(&self, _ssid: &str) {}
    /// Reads the saved password for a Wi-Fi network's saved profile, if any
    /// — a blocking call (a local D-Bus round trip), meant for an explicit,
    /// infrequent "reveal password" action rather than routine polling.
    fn wifi_password(&self, _ssid: &str) -> Option<String> {
        None
    }
}

#[cfg(not(target_os = "windows"))]
pub use network_manager::NetworkManager;
#[cfg(target_os = "windows")]
pub use windows::WindowsNetwork;
pub struct Fallback;
impl NetworkIntegration for Fallback {
    fn connected(&self) -> bool {
        false
    }
    fn network_name(&self) -> Option<String> {
        None
    }
    fn strength(&self) -> Option<u8> {
        None
    }
    fn enabled(&self) -> bool {
        false
    }
    fn set_enabled(&self, _: bool) {}
}
