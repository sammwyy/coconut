use crate::shared::{
    NetworkDetailView, NetworkPasswordReveal, NetworkRevision, NetworkScroll, WifiEnabled,
};
use coconut_api::network::{
    Fallback as NetworkFallback, NetworkDevice, NetworkDeviceKind, NetworkIntegration, WifiNetwork,
};
use coconut_plugin_kit::chrome::{
    action_button, detail_row, detail_row_with_action, icon_button, list_row,
    settings_panel_header, shell_muted, shell_text, wifi_strength_icon,
};
use coconut_plugin_kit::{pixel_icon, Panel, PanelRenderContext, SharedState};
use creamui_core::layout::Style as LayoutStyle;
use creamui_core::{BoxedWidget, Size};
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Flex};
use creamui_widgets::{RawSpinner, ScrollController};
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

#[derive(Clone)]
struct NetworkSecrets {
    _lease: Rc<SecretLease>,
}
struct SecretLease(Rc<dyn NetworkIntegration>);
impl Drop for SecretLease {
    fn drop(&mut self) {
        self.0.clear_wifi_password();
    }
}

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
        build_content(&ctx.shared, &ctx.scratch, None, ctx.size)
    }
}

/// The network panel's actual content, shared between [`NetworkPanel`] (a
/// standalone popup, `on_back: None`) and
/// [`crate::panels::control_center::ControlCenterPanel`] (embedded inline
/// when its internal view is Network, `on_back: Some(_)` to return to the
/// control center's main view). Ported from
/// `desktop/shell/src/panels/network/mod.rs::build`.
pub fn build_content(
    shared: &SharedState,
    scratch: &SharedState,
    on_back: Option<Rc<dyn Fn()>>,
    size: Size,
) -> BoxedWidget {
    let network = shared
        .get::<Rc<dyn NetworkIntegration>>()
        .unwrap_or_else(|| Rc::new(NetworkFallback));
    scratch.get_or_insert_with(|| NetworkSecrets {
        _lease: Rc::new(SecretLease(network.clone())),
    });
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
                let backend = network.clone();
                let password = password.clone();
                Rc::new(move || {
                    backend.clear_wifi_password();
                    password.set(None);
                    view.set(None);
                })
            };
            return build_detail(
                device,
                wifi.as_ref(),
                network.clone(),
                password,
                back,
                size,
                super::detail::scroll_for(scratch, format!("network:{}", device.interface)),
            );
        }
    }

    let wifi_on = wifi_enabled.get();
    let toggle_wifi = {
        let backend = network.clone();
        let enabled = wifi_enabled.clone();
        Rc::new(move || {
            let next = !enabled.get();
            enabled.set(next);
            backend.set_enabled(next);
        })
    };
    let join = scratch.get_or_insert_with(super::network_join::WifiJoin::new);
    let tab = scratch.get_or_insert_with(|| NetworkTab(Signal::new(0))).0;
    let saved = network.saved_wifi_networks();
    let networks = if wifi_on {
        network.wifi_networks()
    } else {
        vec![]
    };
    let mut content = Flex::column().gap(12.0);
    if let Some(error) = network.last_error() {
        content = content.child(super::detail::error(error));
    }
    let mut status =
        Flex::column()
            .gap(10.0)
            .child(super::detail::toggle("Wi-Fi", wifi_on, toggle_wifi));
    if let Some(ssid) = network.connecting_network() {
        status = status.child(Box::new(
            Flex::row()
                .gap(8.0)
                .align(Align::Center)
                .child(Box::new(RawSpinner::new(shell_muted()).size(16.0)))
                .child(super::detail::text(
                    format!("Connecting to {ssid}…"),
                    false,
                    12.0,
                )),
        ));
    } else {
        status = status.child(super::detail::text(
            if wifi_on {
                network
                    .network_name()
                    .unwrap_or_else(|| "Choose a network below".into())
            } else {
                "Wireless is turned off".into()
            },
            false,
            12.0,
        ));
    }
    content = content.child(super::detail::card(Box::new(status)));
    let change = tab.clone();
    let reset = scroll.clone();
    content = content.child(Box::new(
        creamui_widgets::SegmentedControl::new(tab.get(), move |index| {
            reset.set(0.0);
            change.set(index);
        })
        .option("Nearby")
        .option("Saved"),
    ));
    if wifi_on && network.has_wifi_adapter() {
        let backend = network.clone();
        let mut heading = Flex::row()
            .align(Align::Center)
            .gap(8.0)
            .child(super::detail::text(
                if tab.get() == 0 {
                    "Available networks"
                } else {
                    "Saved networks"
                },
                false,
                12.0,
            ))
            .child(Box::new(Flex::row().grow(1.0)));
        heading = if network.scanning() {
            heading.child(Box::new(RawSpinner::new(shell_muted()).size(16.0)))
        } else {
            heading.child(action_button(
                "scan",
                "Scan",
                Rc::new(move || backend.rescan()),
            ))
        };
        content = content.child(Box::new(heading));
        let shown: Vec<_> = if tab.get() == 0 {
            networks.clone()
        } else {
            networks
                .iter()
                .filter(|wifi| saved.contains(&wifi.ssid))
                .cloned()
                .collect()
        };
        for wifi in &shown {
            let backend = network.clone();
            let selected = wifi.clone();
            let view = view.clone();
            let interface = devices
                .iter()
                .find(|device| device.kind == NetworkDeviceKind::Wifi)
                .map(|device| device.interface.clone());
            let join = join.clone();
            let remembered = saved.contains(&wifi.ssid) && network.last_error().is_none();
            let on_click: Rc<dyn Fn()> = Rc::new(move || {
                if backend.connecting_network().is_some() {
                    return;
                }
                if selected.active {
                    if let Some(interface) = &interface {
                        view.set(Some(interface.clone()));
                    }
                } else if selected.secured && selected.security != "Enhanced Open" && !remembered {
                    join.open(selected.clone());
                } else {
                    backend.connect_wifi(&selected.ssid);
                }
            });
            content = content.child(wifi_row(wifi, on_click));
        }
        if tab.get() == 1 {
            for ssid in saved
                .iter()
                .filter(|ssid| !shown.iter().any(|wifi| wifi.ssid == **ssid))
            {
                let name = ssid.clone();
                let backend = network.clone();
                content = content.child(list_row(
                    "wifi-slash",
                    ssid.clone(),
                    "Out of range".into(),
                    Some(icon_button(
                        "trash",
                        24.0,
                        Rc::new(move || backend.forget_wifi(&name)),
                    )),
                    false,
                    None,
                ));
            }
        }
        if shown.is_empty() && (tab.get() == 0 || saved.is_empty()) {
            content = content.child(super::detail::card(super::detail::text(
                if network.scanning() {
                    "Scanning for networks…"
                } else if tab.get() == 0 {
                    "No networks found. Try scanning again."
                } else {
                    "No saved networks."
                },
                false,
                13.0,
            )));
        }
    } else if !network.has_wifi_adapter() {
        content = content.child(super::detail::text(
            "No wireless adapter found.",
            false,
            13.0,
        ));
    }
    let interfaces: Vec<_> = devices
        .iter()
        .filter(|device| {
            matches!(
                device.kind,
                NetworkDeviceKind::Ethernet | NetworkDeviceKind::Vpn
            )
        })
        .collect();
    if !interfaces.is_empty() {
        content = content.child(super::detail::text("Other connections", false, 12.0));
    }
    for device in interfaces {
        let interface = device.interface.clone();
        let open = view.clone();
        content = content.child(list_row(
            if device.kind == NetworkDeviceKind::Vpn {
                "vpn"
            } else {
                "ethernet"
            },
            device
                .connection_name
                .clone()
                .unwrap_or_else(|| device.interface.clone()),
            if device.active {
                "Connected"
            } else {
                "Not connected"
            }
            .into(),
            None,
            device.active,
            Some(Rc::new(move || open.set(Some(interface.clone())))),
        ));
    }
    let content = super::detail::panel(
        size,
        settings_panel_header(
            "wifi-excellent",
            "Network",
            "/connectivity/wifi/nearby",
            on_back,
        ),
        scroll,
        Box::new(content),
    );
    let mut root = creamui_widgets::RawView::new(LayoutStyle {
        size: creamui_widgets::layout::fixed(size.width, size.height),
        ..Default::default()
    })
    .child(content);
    if let Some(dialog) = join.dialog(network, size) {
        root = root.child(dialog);
    }
    Box::new(root)
}

#[derive(Clone)]
struct NetworkTab(Signal<usize>);

/// The detail screen for one connected interface: address/mask/gateway/DNS,
/// plus Wi-Fi-specific rows (signal, band, security, password) and a
/// Forget action when `wifi` is given.
fn build_detail(
    device: &NetworkDevice,
    wifi: Option<&WifiNetwork>,
    network: Rc<dyn NetworkIntegration>,
    password: Signal<Option<String>>,
    on_back: Rc<dyn Fn()>,
    size: Size,
    scroll: ScrollController,
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
    if device.active && network.can_disconnect() {
        let backend = network.clone();
        let interface = device.interface.clone();
        let back = on_back.clone();
        actions = actions.child(action_button(
            "close",
            "Disconnect",
            Rc::new(move || {
                backend.disconnect(&interface);
                back();
            }),
        ));
    }
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

    super::detail::panel(
        size,
        settings_panel_header(
            "wifi-excellent",
            &title,
            "/connectivity/wifi/nearby",
            Some(on_back),
        ),
        scroll,
        Box::new(
            Flex::column()
                .gap(12.0)
                .child(super::detail::card(Box::new(rows)))
                .child(Box::new(actions)),
        ),
    )
}

fn password_row(
    ssid: &str,
    network: Rc<dyn NetworkIntegration>,
    password: Signal<Option<String>>,
) -> BoxedWidget {
    let revealed = password.get().as_deref() == Some(ssid);
    let value = if revealed {
        network.cached_wifi_password(ssid).unwrap_or_else(|| {
            if network.last_error().is_some() {
                "Unavailable".into()
            } else {
                "Loading…".into()
            }
        })
    } else {
        "••••••••".into()
    };
    let ssid = ssid.to_owned();
    let toggle = icon_button(
        if revealed { "eye-off" } else { "eye" },
        20.0,
        Rc::new(move || {
            if revealed {
                network.clear_wifi_password();
                password.set(None);
            } else {
                network.request_wifi_password(&ssid);
                password.set(Some(ssid.clone()));
            }
        }),
    );
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
