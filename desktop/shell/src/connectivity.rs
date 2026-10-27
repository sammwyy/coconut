use crate::popups::{PopupHost, ShellNotification};
use coconut_api::{
    bluetooth::BluetoothIntegration,
    network::{NetworkDeviceKind, NetworkIntegration},
};
use creamui_reactive::{create_effect, Effect, Signal};
use std::{collections::BTreeMap, rc::Rc};

#[derive(Clone, Default)]
struct ConnectivityState {
    enabled: bool,
    connections: BTreeMap<String, String>,
}

impl ConnectivityState {
    fn wifi(network: &dyn NetworkIntegration) -> Self {
        let enabled = network.enabled();
        let mut connections = BTreeMap::new();
        if enabled {
            let devices = network.devices();
            let wifi: Vec<_> = devices
                .iter()
                .filter(|device| device.kind == NetworkDeviceKind::Wifi)
                .collect();
            if !wifi.is_empty() {
                for device in wifi.into_iter().filter(|device| device.active) {
                    if let Some(name) = &device.connection_name {
                        connections.insert(device.interface.clone(), name.clone());
                    }
                }
            } else {
                for network in network
                    .wifi_networks()
                    .into_iter()
                    .filter(|network| network.active)
                {
                    connections.insert(network.ssid.clone(), network.ssid);
                }
                if connections.is_empty() && network.connected() && network.strength().is_some() {
                    if let Some(name) = network.network_name() {
                        connections.insert("wifi".into(), name);
                    }
                }
            }
        }
        Self {
            enabled,
            connections,
        }
    }

    fn bluetooth(bluetooth: &dyn BluetoothIntegration) -> Self {
        let enabled = bluetooth.powered();
        let mut connections = BTreeMap::new();
        if enabled {
            let devices = bluetooth.devices();
            for device in devices.iter().filter(|device| device.connected) {
                connections.insert(device.address.clone(), device.name.clone());
            }
            if devices.is_empty() && bluetooth.connected() {
                if let Some(name) = bluetooth.device_name() {
                    connections.insert("bluetooth".into(), name);
                }
            }
        }
        Self {
            enabled,
            connections,
        }
    }

    fn changes(&self, next: &Self, kind: Kind) -> Vec<ShellNotification> {
        let mut notifications = Vec::new();
        if self.enabled != next.enabled {
            if !next.enabled {
                let body = if self.connections.is_empty() {
                    format!("{} is turned off.", kind.name())
                } else {
                    format!(
                        "Disconnected from {}.",
                        self.connections
                            .values()
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                notifications.push(kind.notification(
                    false,
                    format!("{} disabled", kind.name()),
                    body,
                ));
            } else if next.connections.is_empty() {
                notifications.push(kind.notification(
                    true,
                    format!("{} enabled", kind.name()),
                    format!("{} is ready to connect.", kind.name()),
                ));
            }
        }
        if !next.enabled {
            return notifications;
        }
        for (id, name) in &self.connections {
            if !next.connections.contains_key(id) {
                notifications.push(kind.notification(
                    false,
                    format!("{} disconnected", kind.name()),
                    format!("Disconnected from {name}."),
                ));
            }
        }
        for (id, name) in &next.connections {
            match self.connections.get(id) {
                None => notifications.push(kind.notification(
                    true,
                    format!("{} connected", kind.name()),
                    format!("Connected to {name}."),
                )),
                Some(previous) if previous != name && matches!(kind, Kind::Wifi) => {
                    notifications.push(kind.notification(
                        true,
                        "Wi-Fi network changed".into(),
                        format!("Connected to {name} (previously {previous})."),
                    ));
                }
                _ => {}
            }
        }
        notifications
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Wifi,
    Bluetooth,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Wifi => "Wi-Fi",
            Self::Bluetooth => "Bluetooth",
        }
    }

    fn notification(self, connected: bool, summary: String, body: String) -> ShellNotification {
        ShellNotification {
            icon: match (self, connected) {
                (Self::Wifi, true) => "wifi-excellent",
                (Self::Wifi, false) => "wifi-slash",
                (Self::Bluetooth, true) => "bluetooth-connected",
                (Self::Bluetooth, false) => "bluetooth-off",
            },
            summary,
            body,
        }
    }
}

#[derive(Default)]
struct Observer {
    subscribed: bool,
    previous: Option<ConnectivityState>,
}

impl Observer {
    fn update(
        &mut self,
        read: impl FnOnce() -> ConnectivityState,
        kind: Kind,
    ) -> Vec<ShellNotification> {
        if !self.subscribed {
            self.subscribed = true;
            return Vec::new();
        }
        let next = read();
        let notifications = self
            .previous
            .as_ref()
            .map(|previous| previous.changes(&next, kind))
            .unwrap_or_default();
        self.previous = Some(next);
        notifications
    }
}

pub(crate) fn watch(
    host: &Rc<PopupHost>,
    network: Rc<dyn NetworkIntegration>,
    bluetooth: Rc<dyn BluetoothIntegration>,
    network_revision: Signal<()>,
    bluetooth_revision: Signal<()>,
) -> Vec<Effect> {
    vec![
        observe(
            host,
            network_revision,
            move || ConnectivityState::wifi(network.as_ref()),
            Kind::Wifi,
        ),
        observe(
            host,
            bluetooth_revision,
            move || ConnectivityState::bluetooth(bluetooth.as_ref()),
            Kind::Bluetooth,
        ),
    ]
}

fn observe(
    host: &Rc<PopupHost>,
    revision: Signal<()>,
    read: impl Fn() -> ConnectivityState + 'static,
    kind: Kind,
) -> Effect {
    let host = Rc::downgrade(host);
    let mut observer = Observer::default();
    create_effect(move || {
        revision.get();
        for notification in observer.update(&read, kind) {
            if let Some(host) = host.upgrade() {
                host.notify(notification);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use coconut_api::{
        bluetooth::BluetoothDevice,
        network::{NetworkDevice, WifiNetwork},
    };

    struct Network {
        enabled: bool,
        devices: Vec<NetworkDevice>,
        networks: Vec<WifiNetwork>,
    }

    impl NetworkIntegration for Network {
        fn enabled(&self) -> bool {
            self.enabled
        }
        fn connected(&self) -> bool {
            self.devices.iter().any(|device| device.active)
        }
        fn network_name(&self) -> Option<String> {
            Some("Ethernet".into())
        }
        fn strength(&self) -> Option<u8> {
            None
        }
        fn set_enabled(&self, _: bool) {}
        fn devices(&self) -> Vec<NetworkDevice> {
            self.devices.clone()
        }
        fn wifi_networks(&self) -> Vec<WifiNetwork> {
            self.networks.clone()
        }
    }

    fn wifi(name: Option<&str>) -> NetworkDevice {
        NetworkDevice {
            interface: "wlan0".into(),
            kind: NetworkDeviceKind::Wifi,
            connection_name: name.map(str::to_owned),
            active: name.is_some(),
            ip_address: None,
            subnet_mask: None,
            gateway: None,
            dns: Vec::new(),
            dhcp: true,
        }
    }

    #[test]
    fn startup_and_repeated_state_do_not_notify() {
        let mut observer = Observer::default();
        assert!(observer
            .update(
                || panic!("do not read the cache before its first refresh"),
                Kind::Wifi
            )
            .is_empty());
        let state = ConnectivityState {
            enabled: true,
            connections: [("wlan0".into(), "Home".into())].into(),
        };
        assert!(observer.update(|| state.clone(), Kind::Wifi).is_empty());
        assert!(observer.update(|| state.clone(), Kind::Wifi).is_empty());
        let disconnected = ConnectivityState {
            enabled: true,
            connections: BTreeMap::new(),
        };
        assert_eq!(
            observer.update(|| disconnected.clone(), Kind::Wifi).len(),
            1
        );
        assert!(observer.update(|| disconnected, Kind::Wifi).is_empty());
    }

    #[test]
    fn wifi_changes_include_network_names_and_do_not_report_ethernet_as_wifi() {
        let mut network = Network {
            enabled: true,
            devices: vec![wifi(None)],
            networks: Vec::new(),
        };
        let disconnected = ConnectivityState::wifi(&network);
        network.devices = vec![wifi(Some("Home"))];
        let home = ConnectivityState::wifi(&network);
        let notifications = disconnected.changes(&home, Kind::Wifi);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].summary, "Wi-Fi connected");
        assert_eq!(notifications[0].body, "Connected to Home.");
        network.devices = vec![wifi(Some("Office"))];
        let office = ConnectivityState::wifi(&network);
        let notifications = home.changes(&office, Kind::Wifi);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].summary, "Wi-Fi network changed");
        assert!(notifications[0].body.contains("Office") && notifications[0].body.contains("Home"));
        let notifications = office.changes(&disconnected, Kind::Wifi);
        assert_eq!(notifications[0].body, "Disconnected from Office.");
        network.devices[0].kind = NetworkDeviceKind::Ethernet;
        assert!(ConnectivityState::wifi(&network).connections.is_empty());
    }

    #[test]
    fn wifi_scans_and_signal_changes_are_silent_and_radio_off_is_one_notice() {
        let mut network = Network {
            enabled: true,
            devices: vec![wifi(Some("Home"))],
            networks: Vec::new(),
        };
        let previous = ConnectivityState::wifi(&network);
        network.networks.push(WifiNetwork {
            ssid: "Nearby".into(),
            strength: 90,
            secured: true,
            active: false,
            band: None,
            security: "WPA3".into(),
        });
        network.devices[0].ip_address = Some("192.168.1.3".into());
        assert!(previous
            .changes(&ConnectivityState::wifi(&network), Kind::Wifi)
            .is_empty());
        network.enabled = false;
        let notifications = previous.changes(&ConnectivityState::wifi(&network), Kind::Wifi);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].summary, "Wi-Fi disabled");
        assert_eq!(notifications[0].body, "Disconnected from Home.");
    }

    struct Bluetooth {
        powered: bool,
        devices: Vec<BluetoothDevice>,
    }

    impl BluetoothIntegration for Bluetooth {
        fn powered(&self) -> bool {
            self.powered
        }
        fn connected(&self) -> bool {
            self.devices.iter().any(|device| device.connected)
        }
        fn device_name(&self) -> Option<String> {
            self.devices
                .iter()
                .find(|device| device.connected)
                .map(|device| device.name.clone())
        }
        fn set_powered(&self, _: bool) {}
        fn devices(&self) -> Vec<BluetoothDevice> {
            self.devices.clone()
        }
    }

    fn device(address: &str, name: &str, connected: bool) -> BluetoothDevice {
        BluetoothDevice {
            address: address.into(),
            name: name.into(),
            connected,
            paired: true,
            trusted: true,
            battery_percent: Some(90),
            icon_hint: None,
            class: None,
        }
    }

    #[test]
    fn bluetooth_tracks_each_device_and_ignores_discovery_battery_and_order() {
        let mut bluetooth = Bluetooth {
            powered: true,
            devices: vec![device("AA", "Headphones", true)],
        };
        let previous = ConnectivityState::bluetooth(&bluetooth);
        bluetooth.devices[0].battery_percent = Some(80);
        bluetooth.devices.push(device("BB", "Mouse", false));
        bluetooth.devices.reverse();
        assert!(previous
            .changes(&ConnectivityState::bluetooth(&bluetooth), Kind::Bluetooth)
            .is_empty());
        bluetooth.devices[0].connected = true;
        let connected = ConnectivityState::bluetooth(&bluetooth);
        let notifications = previous.changes(&connected, Kind::Bluetooth);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].body, "Connected to Mouse.");
        bluetooth.devices.retain(|device| device.address != "AA");
        let notifications =
            connected.changes(&ConnectivityState::bluetooth(&bluetooth), Kind::Bluetooth);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].body, "Disconnected from Headphones.");
        bluetooth.powered = false;
        let notifications =
            connected.changes(&ConnectivityState::bluetooth(&bluetooth), Kind::Bluetooth);
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].summary, "Bluetooth disabled");
    }
}
