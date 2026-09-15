use super::{NetworkDevice, NetworkDeviceKind, NetworkIntegration, WifiNetwork};
use crate::integrations::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
use dbus::arg::{PropMap, RefArg, Variant};
use dbus::blocking::stdintf::org_freedesktop_dbus::{Properties, PropertiesPropertiesChanged};
use dbus::blocking::{Connection, SyncConnection};
use dbus::message::SignalArgs;
use dbus::Path;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// NetworkManager exposes its state natively over the system D-Bus; this
/// reads it straight from there and reacts to its `PropertiesChanged`
/// signal instead of polling `nmcli`.
const SERVICE: &str = "org.freedesktop.NetworkManager";
const ROOT_PATH: &str = "/org/freedesktop/NetworkManager";
const SETTINGS_PATH: &str = "/org/freedesktop/NetworkManager/Settings";
const SETTINGS_INTERFACE: &str = "org.freedesktop.NetworkManager.Settings";
const CONNECTION_INTERFACE: &str = "org.freedesktop.NetworkManager.Settings.Connection";
const DEVICE_INTERFACE: &str = "org.freedesktop.NetworkManager.Device";
const WIRELESS_DEVICE_INTERFACE: &str = "org.freedesktop.NetworkManager.Device.Wireless";
const ACTIVE_CONNECTION_INTERFACE: &str = "org.freedesktop.NetworkManager.Connection.Active";
const ACCESS_POINT_INTERFACE: &str = "org.freedesktop.NetworkManager.AccessPoint";
const IP4_CONFIG_INTERFACE: &str = "org.freedesktop.NetworkManager.IP4Config";
const NM_STATE_CONNECTED_LOCAL: u32 = 50;
const AP_FLAG_PRIVACY: u32 = 0x1;
const AP_SEC_KEY_MGMT_PSK: u32 = 0x100;
const AP_SEC_KEY_MGMT_802_1X: u32 = 0x200;
const AP_SEC_KEY_MGMT_SAE: u32 = 0x400;
const AP_SEC_KEY_MGMT_OWE: u32 = 0x800;

const DEVICE_TYPE_ETHERNET: u32 = 1;
const DEVICE_TYPE_WIFI: u32 = 2;
const DEVICE_TYPE_TUN: u32 = 16;
const DEVICE_TYPE_IP_TUNNEL: u32 = 17;
const DEVICE_TYPE_WIREGUARD: u32 = 29;
const DEVICE_TYPE_LOOPBACK: u32 = 32;
/// Bond, VLAN, bridge, team, veth: container/bonding plumbing classified as
/// `Virtual` rather than a real point-to-point tunnel (see [`device_kind`]).
const VIRTUAL_DEVICE_TYPES: [u32; 5] = [10, 11, 13, 15, 20];

#[derive(Default)]
struct State {
    connected: bool,
    enabled: bool,
    name: Option<String>,
    strength: Option<u8>,
    devices: Vec<NetworkDevice>,
    wifi_device: Option<Path<'static>>,
    wifi_networks: Vec<WifiNetwork>,
    ap_paths: HashMap<String, Path<'static>>,
    scanning: bool,
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
        thread::spawn(move || {
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

    fn devices(&self) -> Vec<NetworkDevice> {
        self.state
            .lock()
            .map(|state| state.devices.clone())
            .unwrap_or_default()
    }

    fn has_wifi_adapter(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.wifi_device.is_some())
            .unwrap_or(false)
    }

    fn wifi_networks(&self) -> Vec<WifiNetwork> {
        self.state
            .lock()
            .map(|state| state.wifi_networks.clone())
            .unwrap_or_default()
    }

    fn scanning(&self) -> bool {
        self.state.lock().map(|state| state.scanning).unwrap_or(false)
    }

    fn rescan(&self) {
        let state = self.state.clone();
        if let Ok(mut current) = state.lock() {
            current.scanning = true;
        }
        thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                if let Ok(mut current) = state.lock() {
                    current.scanning = false;
                }
                return;
            };
            let device = state.lock().ok().and_then(|current| current.wifi_device.clone());
            if let Some(device) = device {
                let proxy = connection.with_proxy(SERVICE, &device, Duration::from_secs(10));
                let options: PropMap = HashMap::new();
                let _: Result<(), _> =
                    proxy.method_call(WIRELESS_DEVICE_INTERFACE, "RequestScan", (options,));
                thread::sleep(Duration::from_secs(3));
                if let Ok(sync_connection) = SyncConnection::new_system() {
                    refresh(&sync_connection, &state);
                }
            }
            if let Ok(mut current) = state.lock() {
                current.scanning = false;
            }
        });
    }

    fn connect_wifi(&self, ssid: &str) {
        let state = self.state.clone();
        let ssid = ssid.to_owned();
        thread::spawn(move || connect_to_wifi(&state, &ssid));
    }

    fn forget_wifi(&self, ssid: &str) {
        let ssid = ssid.to_owned();
        thread::spawn(move || {
            let Ok(connection) = Connection::new_system() else {
                return;
            };
            if let Some(path) = find_saved_wifi_connection(&connection, &ssid) {
                let profile = connection.with_proxy(SERVICE, &path, Duration::from_secs(5));
                let _: Result<(), _> = profile.method_call(CONNECTION_INTERFACE, "Delete", ());
            }
        });
    }

    fn wifi_password(&self, ssid: &str) -> Option<String> {
        let connection = Connection::new_system().ok()?;
        let path = find_saved_wifi_connection(&connection, ssid)?;
        let profile = connection.with_proxy(SERVICE, &path, Duration::from_secs(5));
        let (secrets,): (HashMap<String, PropMap>,) = profile
            .method_call(CONNECTION_INTERFACE, "GetSecrets", ("802-11-wireless-security",))
            .ok()?;
        let psk = secrets.get("802-11-wireless-security")?.get("psk")?;
        dbus::arg::cast::<String>(psk.0.as_ref()).cloned()
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

    let device_paths: Vec<Path<'static>> = root.get(SERVICE, "Devices").unwrap_or_default();
    let mut devices = Vec::new();
    let mut wifi_device = None;
    let mut wifi_networks = Vec::new();
    let mut ap_paths = HashMap::new();
    let no_object = Path::from("/");

    for device_path in device_paths {
        let device_proxy = connection.with_proxy(SERVICE, &device_path, Duration::from_secs(2));
        let device_type: u32 = device_proxy
            .get(DEVICE_INTERFACE, "DeviceType")
            .unwrap_or(0);
        if device_type == DEVICE_TYPE_LOOPBACK {
            continue;
        }
        let ip_interface: String = device_proxy
            .get(DEVICE_INTERFACE, "IpInterface")
            .unwrap_or_default();
        let interface = if !ip_interface.is_empty() {
            ip_interface
        } else {
            device_proxy
                .get(DEVICE_INTERFACE, "Interface")
                .unwrap_or_default()
        };
        if interface.is_empty() {
            continue;
        }

        let active_path: Path<'static> = device_proxy
            .get(DEVICE_INTERFACE, "ActiveConnection")
            .unwrap_or_else(|_| no_object.clone());
        let (connection_name, is_vpn) = if active_path == no_object {
            (None, false)
        } else {
            let active = connection.with_proxy(SERVICE, &active_path, Duration::from_secs(2));
            let id = active.get::<String>(ACTIVE_CONNECTION_INTERFACE, "Id").ok();
            let vpn = active
                .get::<bool>(ACTIVE_CONNECTION_INTERFACE, "Vpn")
                .unwrap_or(false);
            (id, vpn)
        };

        if device_type == DEVICE_TYPE_WIFI {
            wifi_device = Some(device_path.clone());
            let (networks, paths) = wifi_scan_results(connection, &device_path);
            wifi_networks = networks;
            ap_paths = paths;
        }

        let kind = device_kind(device_type, is_vpn);
        if kind == NetworkDeviceKind::Other {
            // Genuinely unclassified device types (modems, Bluetooth PAN,
            // ...) — nothing downstream has a use for these yet.
            continue;
        }
        let active = connection_name.is_some() || interface_link_is_up(&interface);
        let ip4 = if active_path != no_object {
            ip4_details(connection, &active_path)
        } else {
            Ip4Details::default()
        };
        devices.push(NetworkDevice {
            interface,
            kind,
            connection_name,
            active,
            ip_address: ip4.ip_address,
            subnet_mask: ip4.subnet_mask,
            gateway: ip4.gateway,
            dns: ip4.dns,
            dhcp: ip4.dhcp,
        });
    }

    let scanning = state.lock().map(|current| current.scanning).unwrap_or(false);
    let next = State {
        connected: raw_state >= NM_STATE_CONNECTED_LOCAL,
        enabled,
        name,
        strength,
        devices,
        wifi_device,
        wifi_networks,
        ap_paths,
        scanning,
    };
    if let Ok(mut current) = state.lock() {
        *current = next;
    }
}

/// A VPN/mesh tunnel (NM-native VPN, WireGuard, or `tun`/IP-tunnel like
/// Tailscale) becomes `Vpn`; container/bonding plumbing becomes `Virtual`.
fn device_kind(device_type: u32, is_vpn: bool) -> NetworkDeviceKind {
    if is_vpn {
        return NetworkDeviceKind::Vpn;
    }
    match device_type {
        DEVICE_TYPE_ETHERNET => NetworkDeviceKind::Ethernet,
        DEVICE_TYPE_WIFI => NetworkDeviceKind::Wifi,
        DEVICE_TYPE_WIREGUARD | DEVICE_TYPE_TUN | DEVICE_TYPE_IP_TUNNEL => NetworkDeviceKind::Vpn,
        other if VIRTUAL_DEVICE_TYPES.contains(&other) => NetworkDeviceKind::Virtual,
        _ => NetworkDeviceKind::Other,
    }
}

/// NM reports externally-managed interfaces (Tailscale, most VPN tun
/// devices) as permanently "unmanaged" regardless of real link state, so
/// this reads the kernel's own view instead.
fn interface_link_is_up(interface: &str) -> bool {
    std::fs::read_to_string(format!("/sys/class/net/{interface}/operstate"))
        .map(|state| state.trim() == "up")
        .unwrap_or(false)
}

#[derive(Default)]
struct Ip4Details {
    ip_address: Option<String>,
    subnet_mask: Option<String>,
    gateway: Option<String>,
    dns: Vec<String>,
    dhcp: bool,
}

/// Reads the IPv4 details a connected device is actually using: its
/// address/mask from `IP4Config`, and whether that came from DHCP by
/// checking the connection profile's own `ipv4.method` setting (`IP4Config`
/// itself carries no such flag).
fn ip4_details(connection: &SyncConnection, active_path: &Path<'static>) -> Ip4Details {
    let no_object = Path::from("/");
    let active = connection.with_proxy(SERVICE, active_path, Duration::from_secs(2));
    let mut details = Ip4Details::default();

    let ip4_path: Path<'static> = active
        .get(ACTIVE_CONNECTION_INTERFACE, "Ip4Config")
        .unwrap_or_else(|_| no_object.clone());
    if ip4_path != no_object {
        let ip4 = connection.with_proxy(SERVICE, &ip4_path, Duration::from_secs(2));
        let address_data: Vec<PropMap> = ip4
            .get(IP4_CONFIG_INTERFACE, "AddressData")
            .unwrap_or_default();
        if let Some(first) = address_data.first() {
            details.ip_address = first
                .get("address")
                .and_then(|value| value.0.as_str())
                .map(str::to_owned);
            details.subnet_mask = first
                .get("prefix")
                .and_then(|value| value.0.as_u64())
                .map(|prefix| prefix_to_netmask(prefix as u8));
        }
        details.gateway = ip4
            .get::<String>(IP4_CONFIG_INTERFACE, "Gateway")
            .ok()
            .filter(|gateway| !gateway.is_empty());
        let nameservers: Vec<PropMap> = ip4
            .get(IP4_CONFIG_INTERFACE, "NameserverData")
            .unwrap_or_default();
        details.dns = nameservers
            .into_iter()
            .filter_map(|entry| {
                entry
                    .get("address")
                    .and_then(|value| value.0.as_str())
                    .map(str::to_owned)
            })
            .collect();
    }

    let connection_path: Path<'static> = active
        .get(ACTIVE_CONNECTION_INTERFACE, "Connection")
        .unwrap_or_else(|_| no_object.clone());
    if connection_path != no_object {
        let settings = connection.with_proxy(SERVICE, &connection_path, Duration::from_secs(2));
        let result: Result<(HashMap<String, PropMap>,), _> =
            settings.method_call(CONNECTION_INTERFACE, "GetSettings", ());
        if let Ok((settings,)) = result {
            details.dhcp = settings
                .get("ipv4")
                .and_then(|ipv4| ipv4.get("method"))
                .and_then(|value| value.0.as_str())
                .is_some_and(|method| method == "auto");
        }
    }

    details
}

fn prefix_to_netmask(prefix: u8) -> String {
    let mask: u32 = if prefix == 0 {
        0
    } else {
        0xFFFF_FFFFu32 << (32 - prefix as u32)
    };
    format!(
        "{}.{}.{}.{}",
        (mask >> 24) & 0xFF,
        (mask >> 16) & 0xFF,
        (mask >> 8) & 0xFF,
        mask & 0xFF
    )
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

struct ApCandidate {
    strength: u8,
    secured: bool,
    active: bool,
    ap_path: Path<'static>,
    band: Option<&'static str>,
    security: String,
}

/// Reads the wireless adapter's current scan results, deduplicating access
/// points that share an SSID (multiple bands/routers for the same network)
/// down to the strongest signal. Returns the networks alongside an
/// SSID-to-access-point-path map used to activate a connection later.
fn wifi_scan_results(
    connection: &SyncConnection,
    device_path: &Path<'static>,
) -> (Vec<WifiNetwork>, HashMap<String, Path<'static>>) {
    let no_object = Path::from("/");
    let device = connection.with_proxy(SERVICE, device_path, Duration::from_secs(2));
    let active_ap: Path<'static> = device
        .get(WIRELESS_DEVICE_INTERFACE, "ActiveAccessPoint")
        .unwrap_or_else(|_| no_object.clone());
    let access_points: Vec<Path<'static>> = device
        .get(WIRELESS_DEVICE_INTERFACE, "AccessPoints")
        .unwrap_or_default();

    let mut best: HashMap<String, ApCandidate> = HashMap::new();
    for ap_path in access_points {
        let ap = connection.with_proxy(SERVICE, &ap_path, Duration::from_secs(2));
        let ssid_bytes: Vec<u8> = ap.get(ACCESS_POINT_INTERFACE, "Ssid").unwrap_or_default();
        if ssid_bytes.is_empty() {
            continue;
        }
        let ssid = String::from_utf8_lossy(&ssid_bytes).into_owned();
        let strength: u8 = ap.get(ACCESS_POINT_INTERFACE, "Strength").unwrap_or(0);
        let flags: u32 = ap.get(ACCESS_POINT_INTERFACE, "Flags").unwrap_or(0);
        let wpa_flags: u32 = ap.get(ACCESS_POINT_INTERFACE, "WpaFlags").unwrap_or(0);
        let rsn_flags: u32 = ap.get(ACCESS_POINT_INTERFACE, "RsnFlags").unwrap_or(0);
        let frequency: u32 = ap.get(ACCESS_POINT_INTERFACE, "Frequency").unwrap_or(0);
        let secured = flags & AP_FLAG_PRIVACY != 0 || wpa_flags != 0 || rsn_flags != 0;
        let active = ap_path == active_ap;
        let candidate = ApCandidate {
            strength,
            secured,
            active,
            ap_path,
            band: band_label(frequency),
            security: security_label(flags, wpa_flags, rsn_flags),
        };
        // The connected BSSID must win dedup even over a stronger duplicate,
        // or the list can lose track of what's actually connected.
        let better = match best.get(&ssid) {
            None => true,
            Some(existing) if candidate.active && !existing.active => true,
            Some(existing) if !candidate.active && existing.active => false,
            Some(existing) => candidate.strength > existing.strength,
        };
        if better {
            best.insert(ssid, candidate);
        }
    }

    let mut ap_paths = HashMap::new();
    let mut networks: Vec<WifiNetwork> = Vec::new();
    for (ssid, candidate) in best {
        ap_paths.insert(ssid.clone(), candidate.ap_path);
        networks.push(WifiNetwork {
            ssid,
            strength: candidate.strength,
            secured: candidate.secured,
            active: candidate.active,
            band: candidate.band,
            security: candidate.security,
        });
    }
    networks.sort_by(|a, b| b.active.cmp(&a.active).then(b.strength.cmp(&a.strength)));
    (networks, ap_paths)
}

fn band_label(frequency_mhz: u32) -> Option<&'static str> {
    match frequency_mhz {
        2400..=2500 => Some("2.4 GHz"),
        4900..=5895 => Some("5 GHz"),
        5925..=7125 => Some("6 GHz"),
        _ => None,
    }
}

/// Derives a human-readable security protocol label from an access point's
/// flags, preferring the strongest key management scheme it advertises.
fn security_label(flags: u32, wpa_flags: u32, rsn_flags: u32) -> String {
    if rsn_flags & AP_SEC_KEY_MGMT_SAE != 0 {
        "WPA3 Personal".to_owned()
    } else if rsn_flags & AP_SEC_KEY_MGMT_802_1X != 0 || wpa_flags & AP_SEC_KEY_MGMT_802_1X != 0 {
        "WPA Enterprise".to_owned()
    } else if rsn_flags & AP_SEC_KEY_MGMT_OWE != 0 {
        "Enhanced Open".to_owned()
    } else if rsn_flags & AP_SEC_KEY_MGMT_PSK != 0 {
        "WPA2 Personal".to_owned()
    } else if wpa_flags & AP_SEC_KEY_MGMT_PSK != 0 {
        "WPA Personal".to_owned()
    } else if flags & AP_FLAG_PRIVACY != 0 {
        "WEP".to_owned()
    } else {
        "Open".to_owned()
    }
}

/// Finds the saved connection profile for a Wi-Fi SSID, if one exists.
fn find_saved_wifi_connection(connection: &Connection, ssid: &str) -> Option<Path<'static>> {
    let settings = connection.with_proxy(SERVICE, SETTINGS_PATH, Duration::from_secs(2));
    let (connections,): (Vec<Path<'static>>,) = settings
        .method_call(SETTINGS_INTERFACE, "ListConnections", ())
        .ok()?;
    connections.into_iter().find(|candidate| {
        let profile = connection.with_proxy(SERVICE, candidate, Duration::from_secs(2));
        let result: Result<(HashMap<String, PropMap>,), _> =
            profile.method_call(CONNECTION_INTERFACE, "GetSettings", ());
        result
            .ok()
            .and_then(|(settings_map,)| connection_ssid(&settings_map))
            .as_deref()
            == Some(ssid)
    })
}

/// Activates a visible Wi-Fi network on a background thread: reuses a saved
/// connection profile for the SSID when one exists, or creates a minimal one
/// on the fly for an open network. A secured network with no saved profile
/// is left alone, since NetworkManager has no secrets to complete it with.
fn connect_to_wifi(state: &Arc<Mutex<State>>, ssid: &str) {
    let Ok(connection) = Connection::new_system() else {
        return;
    };
    let Some((device_path, ap_path)) = state.lock().ok().and_then(|current| {
        Some((
            current.wifi_device.clone()?,
            current.ap_paths.get(ssid).cloned()?,
        ))
    }) else {
        return;
    };

    let root = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(10));
    if let Some(saved) = find_saved_wifi_connection(&connection, ssid) {
        let _: Result<(Path<'static>,), _> =
            root.method_call(SERVICE, "ActivateConnection", (saved, device_path, ap_path));
        return;
    }

    let secured = state
        .lock()
        .ok()
        .and_then(|current| {
            current
                .wifi_networks
                .iter()
                .find(|network| network.ssid == ssid)
                .map(|network| network.secured)
        })
        .unwrap_or(true);
    if secured {
        return;
    }

    let mut wireless: PropMap = HashMap::new();
    wireless.insert(
        "ssid".to_owned(),
        Variant(Box::new(ssid.as_bytes().to_vec()) as Box<dyn RefArg>),
    );
    let mut settings_map: HashMap<String, PropMap> = HashMap::new();
    settings_map.insert("802-11-wireless".to_owned(), wireless);
    let _: Result<(Path<'static>, Path<'static>), _> = root.method_call(
        SERVICE,
        "AddAndActivateConnection",
        (settings_map, device_path, ap_path),
    );
}

fn connection_ssid(settings: &HashMap<String, PropMap>) -> Option<String> {
    let wireless = settings.get("802-11-wireless")?;
    let ssid = wireless.get("ssid")?;
    let bytes = dbus::arg::cast::<Vec<u8>>(ssid.0.as_ref())?;
    Some(String::from_utf8_lossy(bytes).into_owned())
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
