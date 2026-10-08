use crate::components::{group, section, section_label};
use crate::icons::SettingsIcons;
use coconut_api::{
    bluetooth::{BluetoothDevice, BluetoothIntegration},
    network::{NetworkDevice, NetworkDeviceKind, NetworkIntegration, WifiNetwork},
};
use creamui_core::layout::{Dimension, FlexDirection, LengthPercentage, Style};
use creamui_core::{BoxedWidget, Size, Styled};
use creamui_macros::jsx;
use creamui_router::Router;
use creamui_widgets::{Icon, RawButton, RawView, Switch, Symbol, Text, TextSize};
use std::rc::Rc;

#[derive(Clone, PartialEq)]
pub enum View {
    Overview,
    Wifi(String),
    KnownNetworks,
    NearbyNetworks,
    Bluetooth,
    BluetoothDevice(String),
    Ethernet,
    Vpn,
    Network,
}

#[derive(Clone)]
pub struct State {
    pub router: Router,
}
impl State {
    pub fn new(router: Router) -> Self {
        Self { router }
    }
    pub fn current(&self) -> View {
        current(&self.router)
    }
    pub fn showing_detail(&self) -> bool {
        !matches!(current(&self.router), View::Overview)
    }
}

fn chevron() -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    Box::new(Icon::new(Symbol::ChevronRight, theme.colors.text_secondary).size(16.0))
}

pub(super) fn item(
    label: impl Into<String>,
    hint: impl Into<String>,
    trailing: BoxedWidget,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    icon_item(label, hint, None, trailing, click)
}

pub(super) fn icon_item(
    label: impl Into<String>,
    hint: impl Into<String>,
    icon: Option<BoxedWidget>,
    trailing: BoxedWidget,
    click: impl Fn() + 'static,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let hint = hint.into();
    Box::new(RawButton::new(Style {
        size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto },
        min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Length(56.0) },
        flex_shrink: 0.0,
        align_items: Some(creamui_core::layout::AlignItems::Center),
        gap: creamui_core::layout::Size { width: LengthPercentage::Length(12.0), height: LengthPercentage::Length(0.0) },
        padding: creamui_core::layout::Rect { left: LengthPercentage::Length(16.0), right: LengthPercentage::Length(16.0), top: LengthPercentage::Length(10.0), bottom: LengthPercentage::Length(10.0) },
        ..Default::default()
    }, click).hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover)).child(Box::new(jsx! {
        <RawView style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto }, align_items: Some(creamui_core::layout::AlignItems::Center), gap: creamui_core::layout::Size { width: LengthPercentage::Length(12.0), height: LengthPercentage::Length(0.0) }, ..Default::default() }}>
            {icon.unwrap_or_else(|| Box::new(RawView::new(Style { display: creamui_core::layout::Display::None, ..Default::default() })))}
            <RawView style={Style { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_shrink: 1.0, gap: creamui_core::layout::Size { width: LengthPercentage::Length(0.0), height: LengthPercentage::Length(2.0) }, min_size: creamui_core::layout::Size { width: Dimension::Length(0.0), height: Dimension::Auto }, ..Default::default() }}>
                {Box::new(Text::new(label.into())) as BoxedWidget}
                {if hint.is_empty() { Box::new(RawView::new(Style { display: creamui_core::layout::Display::None, ..Default::default() })) as BoxedWidget } else { Box::new(Text::secondary(hint).size(TextSize::Sm)) as BoxedWidget }}
            </RawView>
            <Flex shrink={0.0}>{trailing}</Flex>
        </RawView>
    })))
}

pub(super) fn key_value(label: impl Into<String>, value: impl Into<String>) -> BoxedWidget {
    Box::new(RawView::new(Style {
        size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto },
        ..Default::default()
    }).child(Box::new(jsx! {
        <Flex direction={FlexDirection::Row} style={Style { size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Auto }, ..Default::default() }} align={creamui_widgets::layout::Align::Center} justify={creamui_widgets::layout::Justify::Between} gap={16.0} padding={16.0}>
            {Box::new(Text::secondary(label.into()).size(TextSize::Sm)) as BoxedWidget}
            {Box::new(Text::new(value.into()).size(TextSize::Sm)) as BoxedWidget}
        </Flex>
    })))
}

/// The mock keeps pairing actions in a two-column footer of the Bluetooth
/// card, rather than treating them as ordinary setting rows.
fn bluetooth_card(
    mut rows: Vec<BoxedWidget>,
    bluetooth: Rc<dyn BluetoothIntegration>,
    state: &State,
) -> BoxedWidget {
    let theme = creamui_theme::use_theme();
    let scan = bluetooth.clone();
    let pair_page = state.router.clone();
    let saved = state.router.clone();
    let footer_button = |label: &str, action: Box<dyn Fn()>| -> BoxedWidget {
        Box::new(
            RawButton::new(
                Style {
                    flex_grow: 1.0,
                    flex_basis: Dimension::Length(0.0),
                    size: creamui_core::layout::Size {
                        width: Dimension::Auto,
                        height: Dimension::Percent(1.0),
                    },
                    align_items: Some(creamui_core::layout::AlignItems::Center),
                    justify_content: Some(creamui_core::layout::JustifyContent::Center),
                    ..Default::default()
                },
                action,
            )
            .hover_style(creamui_core::StateStyle::new().background(theme.colors.surface_hover))
            .child(Box::new(Text::new(label).size(TextSize::Sm).bold(true))),
        )
    };
    rows.push(Box::new(
        RawView::new(Style {
            size: creamui_core::layout::Size {
                width: Dimension::Percent(1.0),
                height: Dimension::Length(50.0),
            },
            ..Default::default()
        })
        .with_children(vec![
            footer_button(
                "Pair new device",
                Box::new(move || {
                    scan.start_scan();
                    navigate(&pair_page, View::Bluetooth);
                }),
            ),
            Box::new(
                RawView::new(Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(1.0),
                        height: Dimension::Percent(1.0),
                    },
                    flex_shrink: 0.0,
                    ..Default::default()
                })
                .background(theme.colors.border),
            ),
            footer_button(
                "Saved devices",
                Box::new(move || navigate(&saved, View::Bluetooth)),
            ),
        ]),
    ));
    group(rows)
}

pub fn detail_header(state: &State, window_width: f32) -> BoxedWidget {
    let label = match current(&state.router) {
        View::Wifi(ssid) => ssid,
        View::KnownNetworks => "Known networks".into(),
        View::NearbyNetworks => "Wi-Fi networks".into(),
        View::Bluetooth => "Bluetooth".into(),
        View::BluetoothDevice(address) => address,
        View::Ethernet => "Ethernet".into(),
        View::Vpn => "VPN".into(),
        View::Network => "Network".into(),
        View::Overview => "Connectivity".into(),
    };
    let back = state.router.clone();
    let decorations = creamui_render::use_window_decorations();
    let controls = decorations.controls;
    let right_padding = match decorations.mode {
        creamui_render::WindowDecorationMode::Client => 128.0,
        creamui_render::WindowDecorationMode::Hybrid
            if controls.width > 0 && controls.x as f32 > window_width / 2.0 =>
        {
            (window_width - controls.x as f32 + 16.0).max(36.0)
        }
        _ => 36.0,
    };
    let left_padding = if decorations.mode == creamui_render::WindowDecorationMode::Hybrid
        && controls.width > 0
        && (controls.x as f32) < window_width / 2.0
    {
        // The main panel starts after the 256px sidebar. Respect left-handed
        // compositor button layouts if they extend into this panel.
        ((controls.x + controls.width) as f32 - 256.0 + 16.0).max(36.0)
    } else {
        36.0
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={creamui_widgets::layout::Align::Center} gap={12.0} style={Style { flex_direction: FlexDirection::Row, flex_shrink: 0.0, size: creamui_core::layout::Size { width: Dimension::Percent(1.0), height: Dimension::Length(56.0) }, align_items: Some(creamui_core::layout::AlignItems::Center), padding: creamui_core::layout::Rect { left: LengthPercentage::Length(left_padding), right: LengthPercentage::Length(right_padding), top: LengthPercentage::Length(0.0), bottom: LengthPercentage::Length(0.0) }, ..Default::default() }}>
            {crate::components::back_button(move || navigate(&back, View::Overview))}
            {Box::new(Text::new(label).font_size(14.0).bold(true)) as BoxedWidget}
        </Flex>
    })
}

pub fn build(
    _: Size,
    network: Rc<dyn NetworkIntegration>,
    bluetooth: Rc<dyn BluetoothIntegration>,
    state: &State,
    icons: &SettingsIcons,
) -> BoxedWidget {
    match current(&state.router) {
        View::Overview => overview(network, bluetooth, state, icons),
        View::Wifi(ssid) => wifi_detail(network, state, &ssid),
        View::KnownNetworks => wifi_list(network, state, true, icons),
        View::NearbyNetworks => wifi_list(network, state, false, icons),
        View::Bluetooth => bluetooth_detail(bluetooth, state),
        View::BluetoothDevice(address) => bluetooth_device(bluetooth, state, &address),
        View::Ethernet => network_detail(network, state, NetworkDeviceKind::Ethernet),
        View::Vpn => network_detail(network, state, NetworkDeviceKind::Vpn),
        View::Network => network_detail(network, state, NetworkDeviceKind::Wifi),
    }
}

fn overview(
    network: Rc<dyn NetworkIntegration>,
    bluetooth: Rc<dyn BluetoothIntegration>,
    state: &State,
    icons: &SettingsIcons,
) -> BoxedWidget {
    let enabled = network.enabled();
    let set_enabled = network.clone();
    let wifi_toggle: BoxedWidget = Box::new(Switch::new(enabled, move || {
        set_enabled.set_enabled(!set_enabled.enabled())
    }));
    let mut wifi_rows = vec![icon_item(
        "Wi-Fi",
        network.network_name().unwrap_or_else(|| {
            if enabled {
                "Searching for nearby networks".into()
            } else {
                "Off".into()
            }
        }),
        Some(crate::components::icon_badge(
            icons.status.clone(),
            creamui_theme::use_theme().colors.accent,
        )),
        wifi_toggle,
        || {},
    )];
    if enabled {
        // This is the connection summary, not a scan result list. Showing
        // every access point here made the card both noisy and unlike the
        // reference layout; available networks belong in their own view.
        for wifi in network
            .wifi_networks()
            .into_iter()
            .filter(|wifi| wifi.active)
        {
            let label = wifi.ssid.clone();
            let active = wifi.active;
            let detail = wifi_hint(&wifi);
            let page = state.router.clone();
            let connect = network.clone();
            let ssid = label.clone();
            wifi_rows.push(item(label, detail, chevron(), move || {
                if active {
                    navigate(&page, View::Wifi(ssid.clone()))
                } else {
                    connect.connect_wifi(&ssid)
                }
            }));
        }
    }
    if wifi_rows.len() == 1 {
        let page = state.router.clone();
        let scan = network.clone();
        wifi_rows.push(item(
            "No Wi-Fi connection",
            "Choose a network to get online",
            chevron(),
            move || {
                scan.rescan();
                navigate(&page, View::NearbyNetworks);
            },
        ));
    }
    // Joining prompts are not exposed by the network API yet. Show the
    // reference's resting control as disabled instead of pretending to set it.
    wifi_rows.push(item(
        "Ask to join new networks",
        "",
        Box::new(Switch::new(false, || {}).customize(|switch| switch.disabled = true)),
        || {},
    ));
    let known = state.router.clone();
    wifi_rows.push(item(
        "Known networks",
        format!("{} saved networks", network.saved_wifi_networks().len()),
        chevron(),
        move || navigate(&known, View::KnownNetworks),
    ));
    let powered = bluetooth.powered();
    let set_powered = bluetooth.clone();
    let open_bt = state.router.clone();
    let mut bt_rows = vec![item(
        "Bluetooth",
        if powered {
            "On · discoverable while this page is open"
        } else {
            "Off"
        },
        Box::new(Switch::new(powered, move || {
            set_powered.set_powered(!set_powered.powered())
        })),
        move || navigate(&open_bt, View::Bluetooth),
    )];
    for device in bluetooth.devices().into_iter().filter(|d| d.connected) {
        let address = device.address.clone();
        let page = state.router.clone();
        let hint = bluetooth_hint(&device);
        let icon = device
            .icon_hint
            .as_deref()
            .filter(|hint| hint.contains("head") || hint.contains("audio"))
            .map(|_| {
                Box::new(
                    Icon::new(
                        icons.headphones.clone(),
                        creamui_theme::use_theme().colors.text_secondary,
                    )
                    .size(18.0),
                ) as BoxedWidget
            });
        bt_rows.push(icon_item(device.name, hint, icon, chevron(), move || {
            navigate(&page, View::BluetoothDevice(address.clone()))
        }));
    }
    let devices = network.devices();
    let mut device_rows = Vec::new();
    for kind in [NetworkDeviceKind::Ethernet, NetworkDeviceKind::Vpn] {
        let (name, detail) = device_summary(kind, devices.iter().find(|d| d.kind == kind));
        let page = state.router.clone();
        device_rows.push(item(name, detail, chevron(), move || {
            navigate(
                &page,
                if kind == NetworkDeviceKind::Ethernet {
                    View::Ethernet
                } else {
                    View::Vpn
                },
            )
        }));
    }
    let network_page = state.router.clone();
    device_rows.push(item(
        "Network",
        "DNS, proxies, hostname",
        chevron(),
        move || navigate(&network_page, View::Network),
    ));
    section(
        "",
        "",
        vec![
            Box::new(
                jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Wi-Fi")}{group(wifi_rows)}</Flex> },
            ),
            Box::new(
                jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Bluetooth")}{bluetooth_card(bt_rows, bluetooth, state)}</Flex> },
            ),
            Box::new(
                jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Wired & virtual")}{group(device_rows)}</Flex> },
            ),
        ],
    )
}

fn wifi_list(
    network: Rc<dyn NetworkIntegration>,
    state: &State,
    saved: bool,
    icons: &SettingsIcons,
) -> BoxedWidget {
    let available = network.wifi_networks();
    let names: Vec<String> = if saved {
        network.saved_wifi_networks()
    } else {
        network
            .wifi_networks()
            .into_iter()
            .map(|wifi| wifi.ssid)
            .collect()
    };
    let rows = if names.is_empty() {
        vec![key_value(
            if saved {
                "Saved networks"
            } else {
                "Nearby networks"
            },
            "No networks found",
        )]
    } else {
        names
            .into_iter()
            .map(|name| {
                let page = state.router.clone();
                let ssid = name.clone();
                let hint = available
                    .iter()
                    .find(|wifi| wifi.ssid == name)
                    .map(wifi_hint)
                    .unwrap_or_else(|| {
                        if saved {
                            "Saved Wi-Fi profile".into()
                        } else {
                            "Available network".into()
                        }
                    });
                item(name, hint, chevron(), move || {
                    navigate(&page, View::Wifi(ssid.clone()))
                })
            })
            .collect()
    };
    let description = if saved {
        "Networks saved on this device. Select one to change its connection settings."
    } else {
        "Nearby Wi-Fi networks. Select one to view its connection settings."
    };
    let mut body = vec![
        Box::new(
            Text::secondary(description)
                .size(TextSize::Sm)
                .padding_left(4.0),
        ) as BoxedWidget,
        Box::new(
            jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label(if saved { "Saved networks" } else { "Nearby networks" })}{group(rows)}</Flex> },
        ) as BoxedWidget,
    ];
    if saved {
        body.push(Box::new(jsx! {
            <Flex direction={FlexDirection::Column} gap={8.0}>
                {section_label("Preferences")}
                {group(vec![icon_item(
                    "Ask to join new networks",
                    "Notify when a known network is unavailable",
                    Some(crate::components::icon_badge(icons.about.clone(), creamui_theme::use_theme().colors.accent)),
                    Box::new(Switch::new(false, || {}).customize(|switch| switch.disabled = true)),
                    || {},
                )])}
            </Flex>
        }));
    }
    section("", "", body)
}

fn wifi_detail(network: Rc<dyn NetworkIntegration>, _state: &State, ssid: &str) -> BoxedWidget {
    let wifi = network
        .wifi_networks()
        .into_iter()
        .find(|wifi| wifi.ssid == ssid);
    let device = network
        .devices()
        .into_iter()
        .find(|device| device.kind == NetworkDeviceKind::Wifi && device.active);
    let active = wifi.as_ref().is_some_and(|wifi| wifi.active);
    let security = wifi
        .as_ref()
        .map(|wifi| wifi.security.clone())
        .unwrap_or_else(|| "Unknown".into());
    let band = wifi
        .as_ref()
        .and_then(|wifi| wifi.band)
        .unwrap_or("Unknown");
    let action = network.clone();
    let selected = ssid.to_owned();
    let forget = network.clone();
    let saved = ssid.to_owned();
    section(
        "",
        "",
        vec![
            Box::new(
                jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Connection")}{group(vec![
                    item("Connection", if active { "Connected · excellent signal" } else { "Not connected" }, Box::new(Text::new(if active { "Disconnect" } else { "Connect" }).size(TextSize::Sm)), move || { if !active { action.connect_wifi(&selected) } }),
                    item("Connect automatically", "Join when this network is in range", Box::new(Switch::new(true, || {})), || {}),
                ])}</Flex> },
            ),
            Box::new(
                jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Network details")}{group(vec![
                    key_value("IPv4 address", device.as_ref().and_then(|d| d.ip_address.clone()).unwrap_or_else(|| "Not assigned".into())),
                    key_value("Subnet mask", device.as_ref().and_then(|d| d.subnet_mask.clone()).unwrap_or_else(|| "—".into())),
                    key_value("Router", device.as_ref().and_then(|d| d.gateway.clone()).unwrap_or_else(|| "—".into())),
                    key_value("DNS servers", device.as_ref().map(|d| d.dns.join(" · ")).filter(|s| !s.is_empty()).unwrap_or_else(|| "Automatic".into())),
                    key_value("Security", security), key_value("Band", band),
                ])}</Flex> },
            ),
            Box::new(
                jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Network")}{group(vec![item("Forget this network", "Removes saved password and settings", Box::new(Text::new("Forget").size(TextSize::Sm)), move || forget.forget_wifi(&saved))])}</Flex> },
            ),
        ],
    )
}

fn bluetooth_detail(bluetooth: Rc<dyn BluetoothIntegration>, state: &State) -> BoxedWidget {
    let powered = bluetooth.powered();
    let set_powered = bluetooth.clone();
    let mut rows = vec![item(
        "Bluetooth",
        if powered { "On" } else { "Off" },
        Box::new(Switch::new(powered, move || {
            set_powered.set_powered(!set_powered.powered())
        })),
        || {},
    )];
    for device in bluetooth.devices() {
        let address = device.address.clone();
        let page = state.router.clone();
        let hint = bluetooth_hint(&device);
        rows.push(item(device.name, hint, chevron(), move || {
            navigate(&page, View::BluetoothDevice(address.clone()))
        }));
    }
    section(
        "",
        "",
        vec![Box::new(
            jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Devices")}{group(rows)}</Flex> },
        )],
    )
}

fn bluetooth_device(
    bluetooth: Rc<dyn BluetoothIntegration>,
    _state: &State,
    address: &str,
) -> BoxedWidget {
    let device = bluetooth
        .devices()
        .into_iter()
        .find(|device| device.address == address);
    let name = device
        .as_ref()
        .map(|d| d.name.clone())
        .unwrap_or_else(|| "Bluetooth device".into());
    let connected = device.as_ref().is_some_and(|d| d.connected);
    let action = bluetooth.clone();
    let id = address.to_owned();
    let forget = bluetooth.clone();
    let old = address.to_owned();
    section(
        "",
        "",
        vec![Box::new(
            jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Status")}{group(vec![
                item(name.clone(), if connected { "Connected" } else { "Not connected" }, Box::new(Text::new(if connected { "Disconnect" } else { "Connect" }).size(TextSize::Sm)), move || if connected { action.disconnect(&id) } else { action.connect(&id) }),
                item("Battery", device.as_ref().and_then(|d| d.battery_percent).map(|n| format!("{n}%")).unwrap_or_else(|| "Not reported".into()), Box::new(Text::new("")), || {}),
                item("Forget device", "Remove this pairing from the computer", Box::new(Text::new("Forget").size(TextSize::Sm)), move || forget.forget(&old)),
            ])}</Flex> },
        )],
    )
}

fn network_detail(
    network: Rc<dyn NetworkIntegration>,
    _state: &State,
    kind: NetworkDeviceKind,
) -> BoxedWidget {
    let device = network
        .devices()
        .into_iter()
        .find(|device| device.kind == kind);
    let (_, hint) = device_summary(kind, device.as_ref());
    section(
        "",
        "",
        vec![Box::new(
            jsx! { <Flex direction={FlexDirection::Column} gap={8.0}>{section_label("Connection")}{group(vec![
                item("Status", hint, Box::new(Text::new("")), || {}),
                item("Interface", device.as_ref().map(|d| d.interface.clone()).unwrap_or_else(|| "—".into()), Box::new(Text::new("")), || {}),
                item("IPv4 address", device.as_ref().and_then(|d| d.ip_address.clone()).unwrap_or_else(|| "Not assigned".into()), Box::new(Text::new("")), || {}),
                item("DNS servers", device.as_ref().map(|d| d.dns.join(" · ")).filter(|s| !s.is_empty()).unwrap_or_else(|| "Automatic".into()), Box::new(Text::new("")), || {}),
            ])}</Flex> },
        )],
    )
}

fn wifi_hint(wifi: &WifiNetwork) -> String {
    if wifi.active {
        format!(
            "Connected · {} · {}",
            wifi.security,
            wifi.band.unwrap_or("Wi-Fi")
        )
    } else if wifi.secured {
        format!("Secured · {}%", wifi.strength)
    } else {
        format!("Open · {}%", wifi.strength)
    }
}
fn bluetooth_hint(device: &BluetoothDevice) -> String {
    let mut parts = vec![if device.connected {
        "Connected".into()
    } else if device.paired {
        "Paired".into()
    } else {
        "Available".into()
    }];
    if let Some(battery) = device.battery_percent {
        parts.push(format!("Battery {battery}%"));
    }
    parts.join(" · ")
}
fn device_summary(kind: NetworkDeviceKind, device: Option<&NetworkDevice>) -> (String, String) {
    let fallback = if kind == NetworkDeviceKind::Ethernet {
        "Ethernet"
    } else {
        "VPN"
    };
    let name = device
        .and_then(|d| d.connection_name.clone())
        .unwrap_or_else(|| fallback.into());
    let hint = match device {
        Some(d) if d.active => format!("Connected · {}", d.interface),
        Some(d) => format!("Not connected · {}", d.interface),
        None => "Not available".into(),
    };
    (name, hint)
}

pub(crate) fn route_view() -> creamui_core::BoxedWidget {
    let context = creamui_reactive::use_context::<crate::views::context::ViewContext>();
    let size = context.size;
    let integrations = &context.integrations;
    let icons = &context.icons;
    let connectivity = &context.connectivity;
    crate::views::connectivity::build(
        size,
        integrations.network.clone(),
        integrations.bluetooth.clone(),
        connectivity,
        icons,
    )
}

fn current(router: &Router) -> View {
    let matched = router.current_match();
    let pattern = matched
        .as_ref()
        .map(|m| m.pattern.as_str())
        .unwrap_or_default();
    let params = router.params();
    match pattern {
        "/connectivity/wifi/known" => View::KnownNetworks,
        "/connectivity/wifi/nearby" => View::NearbyNetworks,
        "/connectivity/wifi/:ssid" => View::Wifi(params.get("ssid").cloned().unwrap_or_default()),
        "/connectivity/bluetooth" => View::Bluetooth,
        "/connectivity/bluetooth/:address" => {
            View::BluetoothDevice(params.get("address").cloned().unwrap_or_default())
        }
        "/connectivity/ethernet" => View::Ethernet,
        "/connectivity/vpn" => View::Vpn,
        "/connectivity/network" => View::Network,
        _ => View::Overview,
    }
}

fn navigate(router: &Router, view: View) {
    let path = match view {
        View::Overview => "/connectivity".into(),
        View::KnownNetworks => "/connectivity/wifi/known".into(),
        View::NearbyNetworks => "/connectivity/wifi/nearby".into(),
        View::Wifi(ssid) => format!("/connectivity/wifi/{}", crate::routes::segment(&ssid)),
        View::Bluetooth => "/connectivity/bluetooth".into(),
        View::BluetoothDevice(address) => format!(
            "/connectivity/bluetooth/{}",
            crate::routes::segment(&address)
        ),
        View::Ethernet => "/connectivity/ethernet".into(),
        View::Vpn => "/connectivity/vpn".into(),
        View::Network => "/connectivity/network".into(),
    };
    if let Err(error) = router.navigate(&path) {
        eprintln!("settings: navigation failed: {error}");
    }
}
