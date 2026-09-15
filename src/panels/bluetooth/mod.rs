use crate::integrations::bluetooth::BluetoothIntegration;
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
    bluetooth: Rc<dyn BluetoothIntegration>,
    bluetooth_powered: Signal<bool>,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let powered = bluetooth_powered.get();
    let connected = powered && bluetooth.connected();
    let device_name = if connected {
        bluetooth
            .device_name()
            .unwrap_or_else(|| "Connected".into())
    } else if powered {
        "On".into()
    } else {
        "Off".into()
    };
    let icon = if connected {
        "bluetooth-connected"
    } else if powered {
        "bluetooth-on"
    } else {
        "bluetooth-off"
    };
    let caption = if connected {
        "Connected"
    } else if powered {
        "Available"
    } else {
        "Powered off"
    };

    let toggle_bluetooth = Rc::new(move || {
        let next = !bluetooth_powered.get();
        bluetooth_powered.set(next);
        bluetooth.set_powered(next);
    });

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("BLUETOOTH", on_back)}
            {hero_card(icon, device_name, caption.to_owned())}
            {toggle_row("Bluetooth", powered, toggle_bluetooth)}
        </Flex>
    })
}
