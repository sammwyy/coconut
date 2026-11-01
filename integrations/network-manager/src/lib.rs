use coconut_api::network::{NetworkDevice, NetworkDeviceKind, NetworkIntegration, WifiNetwork};
use coconut_api::{spawn_event_bridge, ChangeListener, EventBridgeGuard};
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
    saved_wifi_networks: Vec<String>,
    ap_paths: HashMap<String, Path<'static>>,
    scanning: bool,
    connecting: Option<String>,
    error: Option<String>,
    operation_revision: u64,
    revealed_password: Option<(String, String)>,
    password_revision: u64,
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
        if let Ok(connection) = SyncConnection::new_system() {
            refresh_enabled(&connection, &state);
        }
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

impl NetworkManager {
    fn start_connection(&self, ssid: &str, password: Option<String>) {
        if let Ok(mut current) = self.state.lock() {
            if current.connecting.is_some() {
                return;
            }
            current.connecting = Some(ssid.to_owned());
            current.error = None;
            current.operation_revision += 1;
        }
        let state = self.state.clone();
        let ssid = ssid.to_owned();
        thread::spawn(move || {
            let result = connect_to_wifi(&state, &ssid, password.as_deref());
            if let Err(error) = &result {
                eprintln!("network manager: connection failed: {error}");
            }
            if let Ok(mut current) = state.lock() {
                current.connecting = None;
                current.error = result.err();
                current.operation_revision += 1;
            }
        });
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

    fn saved_wifi_networks(&self) -> Vec<String> {
        self.state
            .lock()
            .map(|state| state.saved_wifi_networks.clone())
            .unwrap_or_default()
    }

    fn scanning(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.scanning)
            .unwrap_or(false)
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
            let device = state
                .lock()
                .ok()
                .and_then(|current| current.wifi_device.clone());
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
        self.start_connection(ssid, None);
    }

    fn supports_password_connection(&self) -> bool {
        true
    }
    fn connect_wifi_with_password(&self, ssid: &str, password: &str) {
        self.start_connection(ssid, Some(password.to_owned()));
    }
    fn connecting_network(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|state| state.connecting.clone())
    }
    fn last_error(&self) -> Option<String> {
        self.state.lock().ok().and_then(|state| state.error.clone())
    }
    fn can_disconnect(&self) -> bool {
        true
    }
    fn disconnect(&self, interface: &str) {
        let interface = interface.to_owned();
        let state = self.state.clone();
        thread::spawn(move || {
            let result = disconnect_interface(&interface);
            if let Ok(mut state) = state.lock() {
                state.error = result.err();
                state.operation_revision += 1;
            }
        });
    }

    fn forget_wifi(&self, ssid: &str) {
        let ssid = ssid.to_owned();
        let state = self.state.clone();
        thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let connection = Connection::new_system().map_err(|error| error.to_string())?;
                if let Some(path) = find_saved_wifi_connection(&connection, &ssid) {
                    connection
                        .with_proxy(SERVICE, &path, Duration::from_secs(5))
                        .method_call::<(), _, _, _>(CONNECTION_INTERFACE, "Delete", ())
                        .map_err(|error| error.to_string())?;
                }
                Ok(())
            })();
            if let Err(error) = &result {
                eprintln!("network manager: could not forget network: {error}");
            }
            if let Ok(mut state) = state.lock() {
                if result.is_ok() {
                    state.saved_wifi_networks.retain(|name| name != &ssid);
                }
                state.revealed_password = None;
                state.password_revision += 1;
                state.error = result.err();
                state.operation_revision += 1;
            }
        });
    }

    fn request_wifi_password(&self, ssid: &str) {
        let revision = if let Ok(mut state) = self.state.lock() {
            state.password_revision += 1;
            state.revealed_password = None;
            state.password_revision
        } else {
            return;
        };
        let state = self.state.clone();
        let ssid = ssid.to_owned();
        thread::spawn(move || {
            let result = read_wifi_password(&ssid);
            if let Ok(mut state) = state.lock() {
                if state.password_revision != revision {
                    return;
                }
                match result {
                    Ok(password) => {
                        state.revealed_password = Some((ssid, password));
                    }
                    Err(error) => {
                        state.error = Some(error);
                    }
                }
                state.operation_revision += 1;
            }
        });
    }
    fn cached_wifi_password(&self, ssid: &str) -> Option<String> {
        self.state.lock().ok().and_then(|state| {
            state
                .revealed_password
                .as_ref()
                .filter(|(name, _)| name == ssid)
                .map(|(_, password)| password.clone())
        })
    }
    fn clear_wifi_password(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.revealed_password = None;
            state.password_revision += 1;
        }
    }
    fn wifi_password(&self, ssid: &str) -> Option<String> {
        read_wifi_password(ssid).ok()
    }
}

fn read_wifi_password(ssid: &str) -> Result<String, String> {
    let connection = Connection::new_system().map_err(|error| error.to_string())?;
    let path =
        find_saved_wifi_connection(&connection, ssid).ok_or("No saved profile was found.")?;
    let profile = connection.with_proxy(SERVICE, &path, Duration::from_secs(5));
    let (secrets,): (HashMap<String, PropMap>,) = profile
        .method_call(
            CONNECTION_INTERFACE,
            "GetSecrets",
            ("802-11-wireless-security",),
        )
        .map_err(|error| error.to_string())?;
    secrets
        .get("802-11-wireless-security")
        .and_then(|properties| properties.get("psk"))
        .and_then(|value| dbus::arg::cast::<String>(value.0.as_ref()))
        .cloned()
        .ok_or_else(|| "The saved password is unavailable.".into())
}

fn run_event_bridge(
    state: Arc<Mutex<State>>,
    changes: SyncSender<()>,
    shutdown: Receiver<()>,
) -> Result<(), String> {
    let connection = SyncConnection::new_system().map_err(|error| error.to_string())?;
    refresh(&connection, &state);
    let _ = changes.try_send(());

    let sender = dbus::strings::BusName::new(SERVICE).map_err(|error| error.clone())?;
    let rule = PropertiesPropertiesChanged::match_rule(Some(&sender), None).static_clone();
    let signal_state = state.clone();
    let signal_changes = changes.clone();
    let _token = connection
        .add_match(
            rule,
            move |_: PropertiesPropertiesChanged, connection, _| {
                refresh(connection, &signal_state);
                let _ = signal_changes.try_send(());
                true
            },
        )
        .map_err(|error| error.to_string())?;

    let mut operation_revision = 0;
    loop {
        match shutdown.try_recv() {
            Ok(()) | Err(TryRecvError::Disconnected) => return Ok(()),
            Err(TryRecvError::Empty) => {}
        }
        connection
            .process(Duration::from_millis(500))
            .map_err(|error| error.to_string())?;
        let revision = state
            .lock()
            .map(|state| state.operation_revision)
            .unwrap_or(operation_revision);
        if revision != operation_revision {
            operation_revision = revision;
            let _ = changes.try_send(());
        }
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

    let scanning = state
        .lock()
        .map(|current| current.scanning)
        .unwrap_or(false);
    let mut next = State {
        connected: raw_state >= NM_STATE_CONNECTED_LOCAL,
        enabled,
        name,
        strength,
        devices,
        wifi_device,
        wifi_networks,
        saved_wifi_networks: saved_wifi_names(connection),
        ap_paths,
        scanning,
        ..Default::default()
    };
    if let Ok(mut current) = state.lock() {
        next.connecting = current.connecting.clone();
        next.error = current.error.clone();
        next.operation_revision = current.operation_revision;
        next.revealed_password = current.revealed_password.take();
        next.password_revision = current.password_revision;
        *current = next;
    }
}

fn refresh_enabled(connection: &SyncConnection, state: &Arc<Mutex<State>>) {
    let root = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(2));
    let enabled = root
        .get::<bool>(SERVICE, "WirelessEnabled")
        .unwrap_or(false);

    if let Ok(mut current) = state.lock() {
        current.enabled = enabled;
    }
}

fn saved_wifi_names(connection: &SyncConnection) -> Vec<String> {
    let settings = connection.with_proxy(SERVICE, SETTINGS_PATH, Duration::from_secs(2));
    let (profiles,): (Vec<Path<'static>>,) = settings
        .method_call(SETTINGS_INTERFACE, "ListConnections", ())
        .unwrap_or_default();
    let mut names: Vec<String> = profiles
        .into_iter()
        .filter_map(|path| {
            let profile = connection.with_proxy(SERVICE, path, Duration::from_secs(2));
            let (settings,): (HashMap<String, PropMap>,) = profile
                .method_call(CONNECTION_INTERFACE, "GetSettings", ())
                .ok()?;
            connection_ssid(&settings)
        })
        .collect();
    names.sort();
    names.dedup();
    names
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

fn connect_to_wifi(
    state: &Arc<Mutex<State>>,
    ssid: &str,
    password: Option<&str>,
) -> Result<(), String> {
    let connection = Connection::new_system().map_err(|error| error.to_string())?;
    let (device_path, ap_path, security) = state
        .lock()
        .ok()
        .and_then(|current| {
            Some((
                current.wifi_device.clone()?,
                current.ap_paths.get(ssid).cloned()?,
                current
                    .wifi_networks
                    .iter()
                    .find(|network| network.ssid == ssid)?
                    .security
                    .clone(),
            ))
        })
        .ok_or("This network is no longer available. Scan and try again.")?;
    let root = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(10));
    let active = if let Some(saved) = find_saved_wifi_connection(&connection, ssid) {
        if let Some(password) = password {
            let proxy = connection.with_proxy(SERVICE, &saved, Duration::from_secs(5));
            let (mut settings,): (HashMap<String, PropMap>,) = proxy
                .method_call(CONNECTION_INTERFACE, "GetSettings", ())
                .map_err(|error| error.to_string())?;
            let security = personal_security(&security, password)?;
            settings.insert("802-11-wireless-security".into(), security);
            let _: () = proxy
                .method_call(CONNECTION_INTERFACE, "Update", (settings,))
                .map_err(|error| error.to_string())?;
        }
        let (active,): (Path<'static>,) = root
            .method_call(SERVICE, "ActivateConnection", (saved, device_path, ap_path))
            .map_err(|error| error.to_string())?;
        active
    } else {
        let settings = wifi_settings(ssid, &security, password)?;
        let (_, active): (Path<'static>, Path<'static>) = root
            .method_call(
                SERVICE,
                "AddAndActivateConnection",
                (settings, device_path, ap_path),
            )
            .map_err(|error| error.to_string())?;
        active
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let proxy = connection.with_proxy(SERVICE, &active, Duration::from_secs(2));
    while std::time::Instant::now() < deadline {
        match proxy.get::<u32>(ACTIVE_CONNECTION_INTERFACE, "State") {
            Ok(2) => return Ok(()),
            Ok(3 | 4) | Err(_) => {
                return Err("Could not connect. Check the password and try again.".into())
            }
            _ => thread::sleep(Duration::from_millis(150)),
        }
    }
    Err("The connection timed out. Check the password and try again.".into())
}

fn personal_security(security: &str, password: &str) -> Result<PropMap, String> {
    let key_management = match security {
        "WPA3 Personal" => "sae",
        "WPA2 Personal" | "WPA Personal" => "wpa-psk",
        _ => return Err("Configure this security type in Network settings.".into()),
    };
    let valid = if key_management == "sae" {
        !password.is_empty()
    } else {
        (8..=63).contains(&password.len())
            || (password.len() == 64 && password.bytes().all(|byte| byte.is_ascii_hexdigit()))
    };
    if !valid {
        return Err("Enter a valid Wi-Fi password (8–63 characters for WPA/WPA2).".into());
    }
    let mut result = PropMap::new();
    result.insert(
        "key-mgmt".into(),
        Variant(Box::new(key_management.to_owned())),
    );
    result.insert("psk".into(), Variant(Box::new(password.to_owned())));
    Ok(result)
}

fn wifi_settings(
    ssid: &str,
    security: &str,
    password: Option<&str>,
) -> Result<HashMap<String, PropMap>, String> {
    let mut wireless = PropMap::new();
    wireless.insert("ssid".into(), Variant(Box::new(ssid.as_bytes().to_vec())));
    let mut settings = HashMap::from([("802-11-wireless".into(), wireless)]);
    if security == "Enhanced Open" {
        settings.insert(
            "802-11-wireless-security".into(),
            HashMap::from([(
                "key-mgmt".into(),
                Variant(Box::new("owe".to_owned()) as Box<dyn RefArg>),
            )]),
        );
    } else if security != "Open" {
        settings.insert(
            "802-11-wireless-security".into(),
            personal_security(security, password.ok_or("This network needs a password.")?)?,
        );
    }
    Ok(settings)
}

fn disconnect_interface(interface: &str) -> Result<(), String> {
    let connection = Connection::new_system().map_err(|error| error.to_string())?;
    let root = connection.with_proxy(SERVICE, ROOT_PATH, Duration::from_secs(3));
    let paths: Vec<Path<'static>> = root
        .get(SERVICE, "Devices")
        .map_err(|error| error.to_string())?;
    for path in paths {
        let device = connection.with_proxy(SERVICE, path, Duration::from_secs(3));
        if device
            .get::<String>(DEVICE_INTERFACE, "Interface")
            .ok()
            .as_deref()
            == Some(interface)
        {
            return device
                .method_call(DEVICE_INTERFACE, "Disconnect", ())
                .map_err(|error| error.to_string());
        }
    }
    Err("This network interface is no longer available.".into())
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
    fn personal_network_settings_preserve_ssid_bytes_and_security_type() {
        let settings =
            super::wifi_settings("Café 日本語", "WPA2 Personal", Some("fixture password")).unwrap();
        let wireless = settings.get("802-11-wireless").unwrap();
        let bytes = dbus::arg::cast::<Vec<u8>>(wireless["ssid"].0.as_ref()).unwrap();
        assert_eq!(bytes, "Café 日本語".as_bytes());
        assert_eq!(
            settings["802-11-wireless-security"]["key-mgmt"].0.as_str(),
            Some("wpa-psk")
        );
        let sae = super::wifi_settings("Test", "WPA3 Personal", Some("x")).unwrap();
        assert_eq!(
            sae["802-11-wireless-security"]["key-mgmt"].0.as_str(),
            Some("sae")
        );
    }

    #[test]
    fn unsupported_security_and_invalid_passwords_fail_before_activation() {
        assert!(super::wifi_settings("Test", "WPA2 Personal", Some("short")).is_err());
        assert!(super::wifi_settings("Test", "WPA Enterprise", Some("fixture password")).is_err());
        assert!(super::wifi_settings("Test", "WPA2 Personal", Some(&"a".repeat(64))).is_ok());
        assert!(!super::wifi_settings("Test", "Open", None)
            .unwrap()
            .contains_key("802-11-wireless-security"));
        assert_eq!(
            super::wifi_settings("Test", "Enhanced Open", None).unwrap()
                ["802-11-wireless-security"]["key-mgmt"]
                .0
                .as_str(),
            Some("owe")
        );
    }

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
