use crate::icons::pixel_icon;
use crate::panels::chrome::{
    compact_switch, fat_slider, wifi_strength_icon, BORDER, CARD, CARD_RADIUS, CONTROL_HOVER,
    ISLAND_RADIUS, MUTED, PANEL, SELECTED, TEXT,
};
use crate::panels::{
    bluetooth as bluetooth_panel, brightness as brightness_panel, energy as energy_panel,
    network as network_panel, volume as volume_panel,
};
use coconut_api::{
    battery::BatteryIntegration, bluetooth::BluetoothIntegration,
    brightness::BrightnessIntegration, network::NetworkIntegration,
    power_profile::PowerProfileIntegration, volume::VolumeIntegration,
};
use coconut_core::TrayConfig;
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{Border, BoxedWidget, Size, StateStyle, Style, StyleProp, Styled, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::{
    layout::{fixed, Align, Flex},
    RawButton, RawMarquee, ScrollController,
};
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

const TILE_W: f32 = 169.0;
const TILE_H: f32 = 86.0;
const SLIDER_W: f32 = 324.0;

/// Which content the control center popup currently shows. Navigating into
/// a device's dedicated panel and back is just a signal flip, so the popup
/// window never has to be closed and reopened.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PanelView {
    Main,
    Network,
    Bluetooth,
    Energy,
    Brightness,
    Volume,
}

pub fn build_with_integrations(
    size: Size,
    network: Rc<dyn NetworkIntegration>,
    brightness: Rc<dyn BrightnessIntegration>,
    battery: Rc<dyn BatteryIntegration>,
    volume: Rc<dyn VolumeIntegration>,
    bluetooth: Rc<dyn BluetoothIntegration>,
    power_profile: Rc<dyn PowerProfileIntegration>,
    toggle_awake: Rc<dyn Fn()>,
    awake: bool,
    brightness_level: Signal<f32>,
    volume_level: Signal<f32>,
    wifi_enabled: Signal<bool>,
    bluetooth_powered: Signal<bool>,
    network_scroll: ScrollController,
    bluetooth_scroll: ScrollController,
    network_detail: Signal<Option<String>>,
    bluetooth_detail: Signal<Option<String>>,
    network_password: Signal<Option<String>>,
    tray: &TrayConfig,
    view: Signal<PanelView>,
) -> BoxedWidget {
    match view.get() {
        PanelView::Network => {
            let back = back_to_main(view.clone());
            return network_panel::build(
                size,
                network,
                wifi_enabled,
                network_scroll,
                network_detail,
                network_password,
                Some(back),
            );
        }
        PanelView::Bluetooth => {
            let back = back_to_main(view.clone());
            return bluetooth_panel::build(
                size,
                bluetooth,
                bluetooth_powered,
                bluetooth_scroll,
                bluetooth_detail,
                Some(back),
            );
        }
        PanelView::Energy => {
            let back = back_to_main(view.clone());
            return energy_panel::build(
                size,
                battery,
                power_profile,
                awake,
                toggle_awake,
                Some(back),
            );
        }
        PanelView::Brightness => {
            let back = back_to_main(view.clone());
            return brightness_panel::build(size, brightness, brightness_level, Some(back));
        }
        PanelView::Volume => {
            let back = back_to_main(view.clone());
            return volume_panel::build(size, volume, volume_level, Some(back));
        }
        PanelView::Main => {}
    }

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
        "wifi-slash"
    } else {
        wifi_strength_icon(network.strength().unwrap_or(100))
    };
    let wifi_caption = if connected {
        "Connected"
    } else if wifi_on {
        "Disconnected"
    } else {
        "Radio off"
    };

    let bluetooth_on = bluetooth_powered.get();
    let bluetooth_connected = bluetooth_on && bluetooth.connected();
    let bluetooth_icon = if bluetooth_connected {
        "bluetooth-connected"
    } else if bluetooth_on {
        "bluetooth-on"
    } else {
        "bluetooth-off"
    };
    let bluetooth_name = if bluetooth_connected {
        bluetooth
            .device_name()
            .unwrap_or_else(|| "Connected".into())
    } else if bluetooth_on {
        "On".into()
    } else {
        "Off".into()
    };
    let bluetooth_caption = if bluetooth_connected {
        "Connected"
    } else if bluetooth_on {
        "Available"
    } else {
        "Powered off"
    };

    let battery_percent = battery.percentage();
    let battery_icon = if battery.charging() {
        "battery-bolt"
    } else {
        match battery_percent.unwrap_or(0) {
            80.. => "battery-full",
            50..=79 => "battery-mid",
            20..=49 => "battery-low",
            _ => "battery-empty",
        }
    };
    let battery_value = battery_percent
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "--".into());
    let battery_caption = if battery_percent.is_none() {
        "(Unavailable)"
    } else if battery.charging() {
        "(Charging)"
    } else {
        ""
    };

    let brightness_value = brightness_level.get();
    let volume_value = volume_level.get();
    let brightness_text = format!("{}%", (brightness_value * 100.0).round());
    let volume_text = format!(
        "{}%{}",
        (volume_value * 100.0).round(),
        if volume.muted() { "  M" } else { "" }
    );
    let volume_icon = if volume.muted() || volume_value <= 0.01 {
        "volume-mute"
    } else if volume_value < 0.34 {
        "volume-low"
    } else if volume_value < 0.67 {
        "volume-mid"
    } else {
        "volume-high"
    };

    let brightness_backend = brightness.clone();
    let volume_backend = volume.clone();
    let set_brightness = brightness_level.clone();
    let set_volume = volume_level.clone();
    let toggle_wifi = {
        let network = network.clone();
        let wifi_enabled = wifi_enabled.clone();
        Rc::new(move || {
            let next = !wifi_enabled.get();
            wifi_enabled.set(next);
            network.set_enabled(next);
        })
    };
    let toggle_bluetooth = {
        let bluetooth = bluetooth.clone();
        let bluetooth_powered = bluetooth_powered.clone();
        Rc::new(move || {
            let next = !bluetooth_powered.get();
            bluetooth_powered.set(next);
            bluetooth.set_powered(next);
        })
    };

    let open_network_view = {
        let view = view.clone();
        Rc::new(move || view.set(PanelView::Network))
    };
    let open_bluetooth_view = {
        let view = view.clone();
        Rc::new(move || view.set(PanelView::Bluetooth))
    };
    let open_energy_view = {
        let view = view.clone();
        Rc::new(move || view.set(PanelView::Energy))
    };
    let open_brightness_view = {
        let view = view.clone();
        Rc::new(move || view.set(PanelView::Brightness))
    };
    let open_volume_view = {
        let view = view.clone();
        Rc::new(move || view.set(PanelView::Volume))
    };

    let mut device_row = Flex::row().gap(10.0);
    if tray.wifi.shows_in_panel() {
        device_row = device_row.child(device_tile(
            wifi_icon,
            "Wi-Fi",
            network_name,
            wifi_caption,
            wifi_on,
            toggle_wifi,
            open_network_view,
        ));
    }
    if tray.bluetooth.shows_in_panel() {
        device_row = device_row.child(device_tile(
            bluetooth_icon,
            "Bluetooth",
            bluetooth_name,
            bluetooth_caption,
            bluetooth_on,
            toggle_bluetooth,
            open_bluetooth_view,
        ));
    }

    let mut status_row = Flex::row().gap(10.0);
    if tray.battery.shows_in_panel() {
        status_row = status_row.child(status_tile(
            battery_icon,
            "Battery",
            battery_value,
            battery_caption,
            open_energy_view,
        ));
    }
    status_row = status_row.child(awake_tile(awake, toggle_awake));

    let brightness_card: BoxedWidget = if tray.brightness.shows_in_panel() {
        Box::new(slider_container().child(fat_slider(
            "brightness",
            "Brightness",
            brightness_value,
            brightness_text,
            SLIDER_W,
            Some(open_brightness_view),
            move |level| {
                set_brightness.set(level);
                brightness_backend.set_level(level);
            },
        )))
    } else {
        Box::new(jsx! { <Flex/> })
    };
    let volume_card: BoxedWidget = if tray.volume.shows_in_panel() {
        Box::new(slider_container().child(fat_slider(
            volume_icon,
            "Volume",
            volume_value,
            volume_text,
            SLIDER_W,
            Some(open_volume_view),
            move |level| {
                set_volume.set(level);
                volume_backend.set_level(level);
            },
        )))
    } else {
        Box::new(jsx! { <Flex/> })
    };

    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={CARD} border={(BORDER, 1.0)} corner_radius={CARD_RADIUS}>
            <RawText color={MUTED} font_size={14.0}>"CONTROL"</RawText>
            {Box::new(device_row) as BoxedWidget}
            {Box::new(status_row) as BoxedWidget}
            {brightness_card}
            {volume_card}
        </Flex>
    })
}

/// A single slider's own full-width card — brightness and volume each get
/// one, rather than sharing a single box, but both still grow to fill
/// whatever vertical space is left, split evenly between them.
fn slider_container() -> Flex {
    Flex::column()
        .grow(1.0)
        .padding(12.0)
        .justify(creamui_widgets::layout::Justify::Center)
        .property(StyleProp::Background(PANEL.into()))
        .property(StyleProp::Border(Border::new(BORDER, 1.0)))
        .property(StyleProp::CornerRadius(ISLAND_RADIUS))
}

fn device_tile(
    icon: &str,
    kicker: &str,
    name: String,
    caption: &str,
    enabled: bool,
    on_toggle: Rc<dyn Fn()>,
    on_open: Rc<dyn Fn()>,
) -> BoxedWidget {
    let mut name = RawMarquee::new(name, TEXT, 15.0, TILE_W - 20.0);
    name.style.layout.size.height = creamui_core::layout::Dimension::Length(18.0);
    let name = Box::new(name) as BoxedWidget;
    let content = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE_W, TILE_H)} padding={10.0} gap={7.0}>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                {pixel_icon(icon, 16.0, TEXT)}
                <RawText color={MUTED} font_size={11.0} align={TextAlign::Start}>{kicker}</RawText>
                <Flex grow={1.0} />
                {compact_switch(enabled, on_toggle)}
            </Flex>
            {name}
            <RawText color={MUTED} font_size={10.0} align={TextAlign::Start}>{caption}</RawText>
        </Flex>
    });
    Box::new(RawButton::new(tile_style(), move || on_open()).child(content))
}

fn status_tile(
    icon: &str,
    kicker: &str,
    value: String,
    caption: &str,
    on_open: Rc<dyn Fn()>,
) -> BoxedWidget {
    let content = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE_W, TILE_H)} padding={10.0} gap={6.0} justify={creamui_widgets::layout::Justify::Center}>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                {pixel_icon(icon, 16.0, TEXT)}
                <RawText color={MUTED} font_size={11.0} align={TextAlign::Start}>{kicker}</RawText>
            </Flex>
            <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
                <RawText color={TEXT} font_size={18.0} align={TextAlign::Start}>{value}</RawText>
                <RawText color={MUTED} font_size={11.0} align={TextAlign::Start}>{caption}</RawText>
            </Flex>
        </Flex>
    });
    Box::new(RawButton::new(tile_style(), move || on_open()).child(content))
}

fn tile_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(TILE_W, TILE_H),
            ..Default::default()
        })
        .background(PANEL)
        .border(BORDER, 1.0)
        .corner_radius(ISLAND_RADIUS)
        .hover(StateStyle::new().background(CONTROL_HOVER))
        .pressed(StateStyle::new().background(SELECTED))
}

fn back_to_main(view: Signal<PanelView>) -> Rc<dyn Fn()> {
    Rc::new(move || view.set(PanelView::Main))
}

fn awake_tile(awake: bool, toggle_awake: Rc<dyn Fn()>) -> BoxedWidget {
    let caption = if awake { "Holding" } else { "Idle" };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE_W, TILE_H)} padding={10.0} gap={6.0} background={PANEL} border={(BORDER, 1.0)} corner_radius={ISLAND_RADIUS} justify={creamui_widgets::layout::Justify::Center}>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                <RawText color={MUTED} font_size={11.0} align={TextAlign::Start}>"Keep awake"</RawText>
                <Flex grow={1.0} />
                {compact_switch(awake, toggle_awake)}
            </Flex>
            <RawText color={TEXT} font_size={15.0} align={TextAlign::Start}>{caption}</RawText>
        </Flex>
    })
}
