use crate::shared::{
    NetworkDetailView, NetworkPasswordReveal, NetworkRevision, NetworkScroll, WifiEnabled,
};
use coconut_api::network::{
    Fallback as NetworkFallback, NetworkDevice, NetworkDeviceKind, NetworkIntegration, WifiNetwork,
};
use coconut_plugin_kit::chrome::{
    action_button, compact_switch, connection_card, detail_row, detail_row_with_action,
    icon_button, list_row, panel_header, section_label, shell_border, shell_card, shell_muted,
    shell_panel, shell_text, wifi_strength_icon, CARD_RADIUS, ISLAND_RADIUS, LIST_WIDTH,
};
use coconut_plugin_kit::{pixel_icon, Panel, PanelRenderContext, SharedState};
use creamui_core::layout::{Dimension, FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, Styled, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{RawSpinner, ScrollController, ScrollView};
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

/// The device panel wrapper opened directly by the `"tray.wifi"` island
/// (individual tray mode) or by `PanelHost` for any other caller. Delegates
/// to [`build_content`] with `on_back: None`, since a standalone open has no
/// "back to control center" to offer.
///
/// # `SharedState` this panel expects pre-populated
/// - `Rc<dyn coconut_api::network::NetworkIntegration>`
/// - [`crate::WifiEnabled`] (`Signal<bool>`)
/// - [`crate::NetworkRevision`] (`Signal<()>`) — read for its reactive
///   subscription only; pulsed by a background thread relaying the
///   backend's native change hook (or a poll fallback) so this panel
///   re-reads `devices()`/`wifi_networks()`/`scanning()` live.
///
/// This panel's own `scratch` additionally holds (lazily created via
/// `SharedState::get_or_insert_with`, so nothing needs to pre-populate it):
/// [`crate::NetworkScroll`], [`crate::NetworkDetailView`],
/// [`crate::NetworkPasswordReveal`].
pub struct NetworkPanel;

impl Panel for NetworkPanel {
    fn id(&self) -> &'static str {
        "network"
    }

    fn title(&self) -> &'static str {
        "Coconut Network"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        build_content(&ctx.shared, &ctx.scratch, None)
    }
}

/// The network panel's actual content, shared between [`NetworkPanel`] (a
/// standalone popup, `on_back: None`) and
/// [`crate::panels::control_center::ControlCenterPanel`] (embedded inline
/// when its internal view is Network, `on_back: Some(_)` to return to the
/// control center's main view). Ported from
/// `apps/shell/src/panels/network/mod.rs::build`.
pub fn build_content(
    shared: &SharedState,
    scratch: &SharedState,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let network = shared
        .get::<Rc<dyn NetworkIntegration>>()
        .unwrap_or_else(|| Rc::new(NetworkFallback));
    let wifi_enabled = shared
        .get::<WifiEnabled>()
        .unwrap_or_else(|| WifiEnabled(Signal::new(false)))
        .0;
    if let Some(revision) = shared.get::<NetworkRevision>() {
        revision.0.get();
    }
    let scroll = scratch
        .get_or_insert_with(|| NetworkScroll(ScrollController::new(0.0)))
        .0;
    let view = scratch
        .get_or_insert_with(|| NetworkDetailView(Signal::new(None)))
        .0;
    let password = scratch
        .get_or_insert_with(|| NetworkPasswordReveal(Signal::new(None)))
        .0;

    let devices = network.devices();

    // `view` holds the interface to show details for; re-resolved fresh
    // each render so it stays live, and falls back to the list if the
    // device has since disappeared.
    if let Some(interface) = view.get() {
        if let Some(device) = devices.iter().find(|device| device.interface == interface) {
            let wifi = (device.kind == NetworkDeviceKind::Wifi)
                .then(|| network.wifi_networks())
                .and_then(|networks| networks.into_iter().find(|network| network.active));
            let back = {
                let view = view.clone();
                Rc::new(move || view.set(None))
            };
            return build_detail(device, wifi.as_ref(), network.clone(), password, back);
        }
    }

    let wifi_on = wifi_enabled.get();

    let toggle_wifi = {
        let network = network.clone();
        let wifi_enabled = wifi_enabled.clone();
        Rc::new(move || {
            let next = !wifi_enabled.get();
            wifi_enabled.set(next);
            network.set_enabled(next);
        })
    };

    // Only LAN/VPN get cards here (greyed out when idle); Wi-Fi has its own
    // section below instead.
    let wifi_interface = devices
        .iter()
        .find(|device| device.kind == NetworkDeviceKind::Wifi)
        .map(|device| device.interface.clone());
    let connections_row = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} gap={6.0}>
            {section_label("CONNECTIONS")}
            <Flex direction={FlexDirection::Row} gap={8.0}>
                {connection_card_for_kind(&devices, NetworkDeviceKind::Ethernet, "ethernet", "LAN", &view)}
                {connection_card_for_kind(&devices, NetworkDeviceKind::Vpn, "vpn", "VPN", &view)}
            </Flex>
        </Flex>
    }) as BoxedWidget;

    let has_wifi_adapter = network.has_wifi_adapter();
    let wifi_networks = if wifi_on {
        network.wifi_networks()
    } else {
        Vec::new()
    };
    let scanning = network.scanning();

    let mut list = Flex::column().gap(8.0);
    if has_wifi_adapter {
        let rescan = {
            let network = network.clone();
            Rc::new(move || network.rescan())
        };
        list = list.child(wifi_section_header(wifi_on, scanning, toggle_wifi, rescan));
        if !wifi_on {
            list = list.child(empty_row("Wi-Fi is off"));
        } else if wifi_networks.is_empty() {
            list = list.child(empty_row(if scanning {
                "Scanning…"
            } else {
                "No networks found"
            }));
        } else {
            for wifi_network in &wifi_networks {
                let on_click: Rc<dyn Fn()> = if wifi_network.active {
                    match wifi_interface.clone() {
                        Some(interface) => {
                            let view = view.clone();
                            Rc::new(move || view.set(Some(interface.clone())))
                        }
                        None => Rc::new(|| {}),
                    }
                } else {
                    let ssid = wifi_network.ssid.clone();
                    let backend = network.clone();
                    Rc::new(move || backend.connect_wifi(&ssid))
                };
                list = list.child(wifi_row(wifi_network, on_click));
            }
        }
    } else {
        list = list.child(empty_row("No wireless adapter found"));
    }

    let scroll_view = Box::new(
        ScrollView::controlled(scroll_style(), scroll)
            .background(shell_panel())
            .border(shell_border(), 1.0)
            .corner_radius(ISLAND_RADIUS)
            .child(Box::new(list)),
    ) as BoxedWidget;

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("NETWORK", on_back)}
            {connections_row}
            {scroll_view}
        </Flex>
    })
}

/// Renders the active device of `kind`, or a greyed-out `fallback_label`
/// placeholder if there isn't one; the active card opens its detail view.
fn connection_card_for_kind(
    devices: &[NetworkDevice],
    kind: NetworkDeviceKind,
    icon: &str,
    fallback_label: &str,
    view: &Signal<Option<String>>,
) -> BoxedWidget {
    match devices
        .iter()
        .find(|device| device.kind == kind && device.active)
    {
        Some(device) => {
            let title = device
                .connection_name
                .clone()
                .unwrap_or_else(|| device.interface.clone());
            let interface = device.interface.clone();
            let view = view.clone();
            let on_click = Rc::new(move || view.set(Some(interface.clone())));
            connection_card(icon, title, "Connected".to_owned(), true, Some(on_click))
        }
        None => connection_card(
            icon,
            fallback_label.to_owned(),
            "Not connected".to_owned(),
            false,
            None,
        ),
    }
}

/// The detail screen for one connected interface: address/mask/gateway/DNS,
/// plus Wi-Fi-specific rows (signal, band, security, password) and a
/// Forget action when `wifi` is given.
fn build_detail(
    device: &NetworkDevice,
    wifi: Option<&WifiNetwork>,
    network: Rc<dyn NetworkIntegration>,
    password: Signal<Option<String>>,
    on_back: Rc<dyn Fn()>,
) -> BoxedWidget {
    let title = device
        .connection_name
        .clone()
        .unwrap_or_else(|| device.interface.clone());
    let dash = || "—".to_owned();
    let mut rows = Flex::column()
        .gap(8.0)
        .child(detail_row("Interface", device.interface.clone()))
        .child(detail_row(
            "IP Address",
            device.ip_address.clone().unwrap_or_else(dash),
        ))
        .child(detail_row(
            "Subnet Mask",
            device.subnet_mask.clone().unwrap_or_else(dash),
        ))
        .child(detail_row(
            "Gateway",
            device.gateway.clone().unwrap_or_else(dash),
        ))
        .child(detail_row(
            "DNS",
            if device.dns.is_empty() {
                dash()
            } else {
                device.dns.join(", ")
            },
        ))
        .child(detail_row(
            "Configuration",
            if device.dhcp {
                "Automatic (DHCP)".to_owned()
            } else {
                "Manual".to_owned()
            },
        ));

    if let Some(wifi) = wifi {
        rows = rows.child(detail_row("Signal", format!("{}%", wifi.strength)));
        if let Some(band) = wifi.band {
            rows = rows.child(detail_row("Band", band.to_owned()));
        }
        rows = rows.child(detail_row("Security", wifi.security.clone()));
        if wifi.secured {
            rows = rows.child(password_row(&wifi.ssid, network.clone(), password));
        }
    }

    let mut actions = Flex::row().gap(8.0);
    if let Some(wifi) = wifi {
        let ssid = wifi.ssid.clone();
        let backend = network.clone();
        let back = on_back.clone();
        let forget = Rc::new(move || {
            backend.forget_wifi(&ssid);
            back();
        });
        actions = actions.child(action_button("trash", "Forget", forget));
    }

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header(&title, Some(on_back))}
            {Box::new(rows) as BoxedWidget}
            {Box::new(actions) as BoxedWidget}
        </Flex>
    })
}

/// Masked until the eye button fetches the saved secret (a quick blocking
/// D-Bus call — fine for this rare, explicit action); tap again to hide.
fn password_row(
    ssid: &str,
    network: Rc<dyn NetworkIntegration>,
    password: Signal<Option<String>>,
) -> BoxedWidget {
    let revealed = password.get();
    let value = revealed.clone().unwrap_or_else(|| "••••••••".to_owned());
    let toggle: BoxedWidget = if revealed.is_some() {
        let password = password.clone();
        icon_button("eye-off", 18.0, Rc::new(move || password.set(None)))
    } else {
        let ssid = ssid.to_owned();
        icon_button(
            "eye",
            18.0,
            Rc::new(move || password.set(network.wifi_password(&ssid))),
        )
    };
    detail_row_with_action("Password", value, toggle)
}

fn wifi_row(network: &WifiNetwork, on_click: Rc<dyn Fn()>) -> BoxedWidget {
    let icon = wifi_strength_icon(network.strength);
    let title = network.ssid.clone();
    let subtitle = if network.active {
        "Connected".to_owned()
    } else if network.secured {
        "Secured".to_owned()
    } else {
        "Open".to_owned()
    };
    let mut trailing = Flex::row().gap(6.0).align(Align::Center);
    if network.secured {
        trailing = trailing.child(pixel_icon("lock", 12.0, shell_text()));
    }
    if network.active {
        trailing = trailing.child(pixel_icon("check", 14.0, shell_text()));
    }
    list_row(
        icon,
        title,
        subtitle,
        Some(Box::new(trailing)),
        network.active,
        Some(on_click),
    )
}

fn wifi_section_header(
    wifi_on: bool,
    scanning: bool,
    on_toggle: Rc<dyn Fn()>,
    on_rescan: Rc<dyn Fn()>,
) -> BoxedWidget {
    let action: BoxedWidget = if !wifi_on {
        Box::new(jsx! { <Flex/> })
    } else if scanning {
        Box::new(RawSpinner::new(shell_muted()).size(14.0))
    } else {
        icon_button("scan", 22.0, on_rescan)
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0} padding={4.0}>
            <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>"WI-FI NETWORKS"</RawText>
            {compact_switch(wifi_on, on_toggle)}
            <Flex grow={1.0} />
            {action}
        </Flex>
    })
}

fn empty_row(message: &str) -> BoxedWidget {
    Box::new(jsx! {
        <Flex padding={10.0} align={Align::Center} justify={creamui_widgets::layout::Justify::Center}>
            <RawText color={shell_muted()} font_size={12.0}>{message.to_owned()}</RawText>
        </Flex>
    })
}

/// A fixed width, but no fixed height: `flex_grow` lets the list fill
/// whatever vertical space is left in the panel once the fixed-height
/// content above it (header, connection cards) is accounted for.
fn scroll_style() -> LayoutStyle {
    LayoutStyle {
        size: creamui_core::layout::Size {
            width: Dimension::Length(LIST_WIDTH),
            height: Dimension::Auto,
        },
        flex_grow: 1.0,
        ..Default::default()
    }
}
