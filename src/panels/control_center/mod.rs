use crate::integrations::{
    battery::BatteryIntegration, bluetooth::BluetoothIntegration,
    brightness::BrightnessIntegration, network::NetworkIntegration, volume::VolumeIntegration,
};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{BoxedWidget, Size, Style};
use creamui_macros::jsx;
use creamui_theme::Color;
use creamui_widgets::{
    layout::{fixed, Align},
    RawSlider,
};
use std::rc::Rc;

const CARD: Color = Color::rgba(27, 28, 30, 252);
const PANEL: Color = Color::rgba(39, 40, 42, 245);
const TEXT: Color = Color::rgb(244, 244, 245);
const MUTED: Color = Color::rgb(166, 168, 171);
const ACCENT: Color = Color::rgba(255, 255, 255, 28);
const BAR_TRACK: Color = Color::rgba(255, 255, 255, 38);
const BAR_FILL: Color = Color::rgb(215, 222, 255);

pub const WIDTH: u32 = 360;
pub const HEIGHT: u32 = 360;

pub fn build_with_integrations(
    _: Size,
    network: Rc<dyn NetworkIntegration>,
    brightness: Rc<dyn BrightnessIntegration>,
    battery: Rc<dyn BatteryIntegration>,
    volume: Rc<dyn VolumeIntegration>,
    bluetooth: Rc<dyn BluetoothIntegration>,
    toggle_awake: Rc<dyn Fn()>,
) -> BoxedWidget {
    let connected = network.connected();
    let network_text = if connected {
        format!(
            "Wi-Fi · {}",
            network.network_name().unwrap_or_else(|| "Connected".into())
        )
    } else {
        "Wi-Fi · Disconnected".into()
    };
    let brightness_value = brightness.level();
    let volume_value = volume.level();
    let brightness_text = format!("{}%", (brightness_value * 100.0).round());
    let volume_text = format!(
        "{}%{}",
        (volume_value * 100.0).round(),
        if volume.muted() { " · Muted" } else { "" }
    );
    let battery_percent = battery
        .percentage()
        .map(|percent| format!("{percent}%"))
        .unwrap_or_else(|| "Unavailable".into());
    let battery_text = format!(
        "Battery      {battery_percent} · {}",
        if battery.charging() {
            "Charging"
        } else {
            "Discharging"
        }
    );
    let wifi_icon = if !connected {
        "wifi-off"
    } else {
        match network.strength().unwrap_or(100) {
            75.. => "wifi-high",
            50..=74 => "wifi-mid",
            25..=49 => "wifi-low",
            _ => "wifi-none",
        }
    };
    let battery_icon = if battery.charging() {
        "battery-bolt"
    } else {
        match battery.percentage().unwrap_or(0) {
            80.. => "battery-full",
            50..=79 => "battery-mid",
            20..=49 => "battery-low",
            _ => "battery-empty",
        }
    };
    let bluetooth_connected = bluetooth.connected();
    let bluetooth_icon = if bluetooth_connected {
        "bluetooth-connected"
    } else if bluetooth.powered() {
        "bluetooth-on"
    } else {
        "bluetooth-off"
    };
    let bluetooth_text = if bluetooth_connected {
        format!(
            "Bluetooth · {}",
            bluetooth
                .device_name()
                .unwrap_or_else(|| "Connected".into())
        )
    } else if bluetooth.powered() {
        "Bluetooth · On".into()
    } else {
        "Bluetooth · Off".into()
    };
    let brightness_backend = brightness.clone();
    let brightness_slider: BoxedWidget = Box::new(
        RawSlider::new(
            slider_style(),
            brightness_value,
            BAR_TRACK,
            BAR_FILL,
            TEXT,
            move |level| brightness_backend.set_level(level),
        )
        .track(5.0, 2.5)
        .handle(13.0, 6.5),
    );
    let volume_backend = volume.clone();
    let volume_slider: BoxedWidget = Box::new(
        RawSlider::new(
            slider_style(),
            volume_value,
            BAR_TRACK,
            BAR_FILL,
            TEXT,
            move |level| volume_backend.set_level(level),
        )
        .track(5.0, 2.5)
        .handle(13.0, 6.5),
    );
    let volume_icon = if volume.muted() || volume_value <= 0.01 {
        "volume-mute"
    } else if volume_value < 0.34 {
        "volume-low"
    } else if volume_value < 0.67 {
        "volume-mid"
    } else {
        "volume-max"
    };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={18.0} gap={12.0} background={CARD} corner_radius={16.0}>
            <RawText color={TEXT} font_size={16.0}>"Control Center"</RawText>
            <Flex direction={FlexDirection::Column} gap={8.0} padding={12.0} background={PANEL} corner_radius={10.0}>
                <RawText color={TEXT} font_size={12.0}>"Connections"</RawText>
                <Flex direction={FlexDirection::Row} gap={6.0} align={Align::Center}>{pixel_icon(wifi_icon, 14.0)}<RawText color={MUTED} font_size={11.0}>{network_text}</RawText></Flex>
                <Flex direction={FlexDirection::Row} gap={6.0} align={Align::Center}>{pixel_icon(bluetooth_icon, 14.0)}<RawText color={MUTED} font_size={11.0}>{bluetooth_text}</RawText></Flex>
            </Flex>
            <Flex direction={FlexDirection::Column} gap={7.0} padding={12.0} background={PANEL} corner_radius={10.0}>
                <RawText color={TEXT} font_size={12.0}>"Quick Settings"</RawText>
                <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                    {pixel_icon("brightness", 15.0)}
                    <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                        <Flex direction={FlexDirection::Row}><RawText color={MUTED} font_size={11.0}>"Brightness"</RawText><Flex grow={1.0}/><RawText color={MUTED} font_size={10.0}>{brightness_text}</RawText></Flex>
                        {brightness_slider}
                    </Flex>
                </Flex>
                <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                    {pixel_icon(volume_icon, 15.0)}
                    <Flex direction={FlexDirection::Column} gap={3.0} grow={1.0}>
                        <Flex direction={FlexDirection::Row}><RawText color={MUTED} font_size={11.0}>"Volume"</RawText><Flex grow={1.0}/><RawText color={MUTED} font_size={10.0}>{volume_text}</RawText></Flex>
                        {volume_slider}
                    </Flex>
                </Flex>
                <Flex direction={FlexDirection::Row} gap={6.0} align={Align::Center}>{pixel_icon(battery_icon, 14.0)}<RawText color={MUTED} font_size={11.0}>{battery_text}</RawText></Flex>
            </Flex>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center} padding={10.0} background={PANEL} corner_radius={10.0}>
                <RawText color={TEXT} font_size={12.0}>"Profile: Balanced"</RawText>
                <Flex grow={1.0} />
                <RawButton style={small_button_style()} on_click={move || toggle_awake()}><RawText color={TEXT} font_size={11.0}>"ON"</RawText></RawButton>
            </Flex>
            <RawText color={MUTED} font_size={10.0}>"Keep awake · Prevents sleep and screen blanking"</RawText>
        </Flex>
    })
}

fn small_button_style() -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(30.0, 28.0),
            ..Default::default()
        })
        .background(ACCENT)
        .corner_radius(7.0)
}

fn slider_style() -> Style {
    Style::new().layout(LayoutStyle {
        size: fixed(250.0, 16.0),
        ..Default::default()
    })
}
use crate::icons::pixel_icon;
