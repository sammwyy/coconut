use crate::integrations::network::NetworkIntegration;
use crate::panels::chrome::{hero_card, panel_header, toggle_row, BORDER, CARD, CARD_RADIUS};
use creamui_core::layout::FlexDirection;
use creamui_core::{BoxedWidget, Size};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 420;

pub fn build(
    _: Size,
    network: Rc<dyn NetworkIntegration>,
    wifi_enabled: Signal<bool>,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let wifi_on = wifi_enabled.get();
    let connected = wifi_on && network.connected();
    let network_name = if wifi_on {
        network
            .network_name()
            .unwrap_or_else(|| "No network".into())
    } else {
        "Off".into()
    };
    let wifi_icon = if !wifi_on {
        "wifi-off"
    } else {
        match network.strength().unwrap_or(100) {
            75.. => "wifi-high",
            50..=74 => "wifi-mid",
            25..=49 => "wifi-low",
            _ => "wifi-none",
        }
    };
    let wifi_caption = if connected {
        "Connected"
    } else if wifi_on {
        "Disconnected"
    } else {
        "Radio off"
    };

    let toggle_wifi = Rc::new(move || {
        let next = !wifi_enabled.get();
        wifi_enabled.set(next);
        network.set_enabled(next);
    });

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("NETWORK", on_back)}
            {hero_card(wifi_icon, network_name, wifi_caption.to_owned())}
            {toggle_row("Wi-Fi", wifi_on, toggle_wifi)}
        </Flex>
    })
}
