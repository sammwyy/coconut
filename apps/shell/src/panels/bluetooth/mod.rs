use crate::icons::pixel_icon;
use crate::panels::chrome::{
    action_button, compact_switch, detail_row, icon_button, list_row, panel_header, section_label,
    BORDER, CARD, CARD_RADIUS, ISLAND_RADIUS, LIST_WIDTH, MUTED, PANEL,
};
use coconut_api::bluetooth::{BluetoothDevice, BluetoothIntegration};
use creamui_core::layout::{Dimension, FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, Styled, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::{ScrollController, ScrollView};
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

pub fn build(
    _: Size,
    bluetooth: Rc<dyn BluetoothIntegration>,
    bluetooth_powered: Signal<bool>,
    scroll: ScrollController,
    view: Signal<Option<String>>,
    on_back: Option<Rc<dyn Fn()>>,
) -> BoxedWidget {
    let powered = bluetooth_powered.get();
    let devices = if powered {
        bluetooth.devices()
    } else {
        Vec::new()
    };

    // `view` holds the address to show details for; re-resolved fresh each
    // render so it stays live, and falls back to the list if the device
    // has since vanished.
    if let Some(address) = view.get() {
        if let Some(device) = devices.iter().find(|device| device.address == address) {
            let back = {
                let view = view.clone();
                Rc::new(move || view.set(None))
            };
            return build_detail(device, bluetooth.clone(), back);
        }
    }

    let toggle_bluetooth = {
        let bluetooth = bluetooth.clone();
        let bluetooth_powered = bluetooth_powered.clone();
        Rc::new(move || {
            let next = !bluetooth_powered.get();
            bluetooth_powered.set(next);
            bluetooth.set_powered(next);
        })
    };

    let scanning = bluetooth.scanning();

    let mut list = Flex::column().gap(8.0);
    let on_toggle_scan = {
        let bluetooth = bluetooth.clone();
        Rc::new(move || {
            if scanning {
                bluetooth.stop_scan();
            } else {
                bluetooth.start_scan();
            }
        })
    };
    list = list.child(devices_section_header(
        powered,
        scanning,
        toggle_bluetooth,
        on_toggle_scan,
    ));
    if !powered {
        list = list.child(empty_row("Bluetooth is off"));
    } else if devices.is_empty() {
        list = list.child(empty_row(if scanning {
            "Scanning…"
        } else {
            "No devices found"
        }));
    } else {
        // Paired devices (this system's own) are kept apart from whatever
        // a scan has merely discovered nearby, each under its own label.
        let (paired, discovered): (Vec<&BluetoothDevice>, Vec<&BluetoothDevice>) =
            devices.iter().partition(|device| device.paired);
        if !paired.is_empty() {
            list = list.child(section_label("PAIRED"));
            for device in paired {
                list = list.child(paired_device_row(device, &view));
            }
        }
        if !discovered.is_empty() {
            list = list.child(section_label("AVAILABLE"));
            for device in discovered {
                list = list.child(discovered_device_row(device, bluetooth.clone()));
            }
        }
    }

    let scroll_view = Box::new(
        ScrollView::controlled(scroll_style(), scroll)
            .background(PANEL)
            .border(BORDER, 1.0)
            .corner_radius(ISLAND_RADIUS)
            .child(Box::new(list)),
    ) as BoxedWidget;

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header("BLUETOOTH", on_back)}
            {scroll_view}
        </Flex>
    })
}

/// A paired device's row: tapping it opens its detail view. Connect,
/// disconnect and forget all live there instead of cluttering this row —
/// the only inline affordance is a checkmark while connected.
fn paired_device_row(device: &BluetoothDevice, view: &Signal<Option<String>>) -> BoxedWidget {
    let icon = device_row_icon(device);
    let subtitle = if device.connected {
        "Connected"
    } else {
        "Paired"
    };
    let trailing = device.connected.then(|| pixel_icon("check", 14.0));
    let address = device.address.clone();
    let view = view.clone();
    let on_click = Rc::new(move || view.set(Some(address.clone())));
    list_row(
        icon,
        device.name.clone(),
        subtitle.to_owned(),
        trailing,
        device.connected,
        Some(on_click),
    )
}

/// A discovered-but-unpaired device's row: nothing to show details on yet,
/// so the row itself isn't interactive — just a Connect action.
fn discovered_device_row(
    device: &BluetoothDevice,
    bluetooth: Rc<dyn BluetoothIntegration>,
) -> BoxedWidget {
    let icon = device_row_icon(device);
    let address = device.address.clone();
    let connect = icon_button(
        "bluetooth-on",
        22.0,
        Rc::new(move || bluetooth.connect(&address)),
    );
    list_row(
        icon,
        device.name.clone(),
        "Available".to_owned(),
        Some(connect),
        false,
        None,
    )
}

/// The row icon for a device: its recognized type (headphones, mouse,
/// gamepad, ...) from the device's raw class or BlueZ's icon hint, in that
/// order, otherwise the generic connected/paired/off glyph.
fn device_row_icon(device: &BluetoothDevice) -> &'static str {
    if let Some(icon) = class_device_icon(device.class) {
        return icon;
    }
    if let Some(icon) = device_type_icon(device.icon_hint.as_deref()) {
        return icon;
    }
    if device.connected {
        "bluetooth-connected"
    } else if device.paired {
        "bluetooth-on"
    } else {
        "bluetooth-off"
    }
}

/// Maps a BlueZ icon hint (freedesktop icon-naming-spec names such as
/// `"audio-headset"`, `"input-mouse"`) to one of our bundled device icons.
fn device_type_icon(hint: Option<&str>) -> Option<&'static str> {
    match hint? {
        "audio-headset" | "audio-headphones" => Some("headphones"),
        "audio-card" | "multimedia-player" => Some("speaker"),
        "input-mouse" => Some("mouse"),
        "input-keyboard" => Some("keyboard"),
        "input-gaming" => Some("gamepad"),
        "phone" => Some("phone"),
        "computer" => Some("laptop"),
        _ => None,
    }
}

/// Reads the raw Bluetooth Class of Device bitmask for cases BlueZ's own
/// `icon_hint` collapses too coarsely — a TV/set-top box reports the same
/// Audio/Video major class as a real speaker or headset, and only the
/// minor device class (bits 2-7) tells them apart. See the Bluetooth
/// Assigned Numbers "Baseband" class-of-device table.
fn class_device_icon(class: Option<u32>) -> Option<&'static str> {
    const MAJOR_AUDIO_VIDEO: u32 = 4;
    const MINOR_SET_TOP_BOX: u32 = 9;
    const MINOR_VIDEO_DISPLAY_AND_LOUDSPEAKER: u32 = 15;

    let class = class?;
    let major = (class >> 8) & 0x1F;
    let minor = (class >> 2) & 0x3F;
    (major == MAJOR_AUDIO_VIDEO
        && matches!(
            minor,
            MINOR_SET_TOP_BOX | MINOR_VIDEO_DISPLAY_AND_LOUDSPEAKER
        ))
    .then_some("tv")
}

/// The detail screen for one paired device: its address, connection and
/// trust status, battery level when the device reports one, and the
/// connect/disconnect and forget actions.
fn build_detail(
    device: &BluetoothDevice,
    bluetooth: Rc<dyn BluetoothIntegration>,
    on_back: Rc<dyn Fn()>,
) -> BoxedWidget {
    let mut rows = Flex::column()
        .gap(8.0)
        .child(detail_row("Address", device.address.clone()))
        .child(detail_row(
            "Status",
            if device.connected {
                "Connected".to_owned()
            } else {
                "Not connected".to_owned()
            },
        ))
        .child(detail_row(
            "Trusted",
            if device.trusted { "Yes" } else { "No" }.to_owned(),
        ));
    if let Some(percent) = device.battery_percent {
        rows = rows.child(detail_row("Battery", format!("{percent}%")));
    }

    let connected = device.connected;
    let toggle_backend = bluetooth.clone();
    let toggle_address = device.address.clone();
    let toggle_action = Rc::new(move || {
        if connected {
            toggle_backend.disconnect(&toggle_address);
        } else {
            toggle_backend.connect(&toggle_address);
        }
    });
    let forget_address = device.address.clone();
    let forget_action = Rc::new(move || bluetooth.forget(&forget_address));

    let actions = Flex::row()
        .gap(8.0)
        .child(action_button(
            if connected { "close" } else { "bluetooth-on" },
            if connected { "Disconnect" } else { "Connect" },
            toggle_action,
        ))
        .child(action_button("trash", "Forget", forget_action));

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            {panel_header(&device.name, Some(on_back))}
            {Box::new(rows) as BoxedWidget}
            {Box::new(actions) as BoxedWidget}
        </Flex>
    })
}

fn devices_section_header(
    powered: bool,
    scanning: bool,
    on_toggle_power: Rc<dyn Fn()>,
    on_toggle_scan: Rc<dyn Fn()>,
) -> BoxedWidget {
    let action: BoxedWidget = if !powered {
        Box::new(jsx! { <Flex/> })
    } else if scanning {
        action_button("close", "Stop", on_toggle_scan)
    } else {
        action_button("scan", "Scan", on_toggle_scan)
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0} padding={4.0}>
            <RawText color={MUTED} font_size={11.0} align={TextAlign::Start}>"DEVICES"</RawText>
            {compact_switch(powered, on_toggle_power)}
            <Flex grow={1.0} />
            {action}
        </Flex>
    })
}

fn empty_row(message: &str) -> BoxedWidget {
    Box::new(jsx! {
        <Flex padding={10.0} align={Align::Center} justify={Justify::Center}>
            <RawText color={MUTED} font_size={12.0}>{message.to_owned()}</RawText>
        </Flex>
    })
}

/// A fixed width, but no fixed height: `flex_grow` lets the list fill
/// whatever vertical space is left in the panel below the header.
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
