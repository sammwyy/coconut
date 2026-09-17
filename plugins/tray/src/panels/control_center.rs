use crate::config::TrayConfig;
use crate::panels::{bluetooth, brightness, energy, network, volume};
use crate::shared::{
    BatteryRevision, BluetoothPowered, BluetoothRevision, BrightnessLevel, KeepAwakeState,
    NetworkRevision, PowerProfileRevision, ToggleKeepAwake, VolumeLevel, WifiEnabled,
};
use coconut_api::battery::{BatteryIntegration, Fallback as BatteryFallback};
use coconut_api::bluetooth::{BluetoothIntegration, Fallback as BluetoothFallback};
use coconut_api::brightness::{BrightnessIntegration, Fallback as BrightnessFallback};
use coconut_api::network::{Fallback as NetworkFallback, NetworkIntegration};
use coconut_api::volume::{Fallback as VolumeFallback, VolumeIntegration};
use coconut_plugin_kit::chrome::{
    compact_switch, fat_slider, shell_border, shell_card, shell_control_hover, shell_muted,
    shell_panel, shell_selected, shell_text, wifi_strength_icon, CARD_RADIUS, ISLAND_RADIUS,
};
use coconut_plugin_kit::{Panel, PanelRenderContext};
use creamui_core::layout::{FlexDirection, Style as LayoutStyle};
use creamui_core::{Border, BoxedWidget, Size, StateStyle, Style, StyleProp, Styled, TextAlign};
use creamui_macros::jsx;
use creamui_reactive::Signal;
use creamui_widgets::{
    layout::{fixed, Align, Flex},
    RawButton, RawMarquee,
};
use std::rc::Rc;

pub const WIDTH: u32 = 380;
pub const HEIGHT: u32 = 620;

const TILE_W: f32 = 169.0;
const TILE_H: f32 = 86.0;
const SLIDER_W: f32 = 324.0;

/// Which content the control center popup currently shows, stored per-open
/// via `ctx.scratch.get_or_insert_with(|| Signal::new(PanelView::Main))` so
/// drilling into a device and back is a pure signal flip — the popup window
/// itself is never closed and reopened. Ported from
/// `apps/shell/src/panels/control_center/mod.rs::PanelView`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PanelView {
    Main,
    Network,
    Bluetooth,
    Energy,
    Brightness,
    Volume,
}

/// The `"control_center"` panel: opened either directly (grouped tray mode's
/// single fused button) or from any device panel's own entry point. Its
/// device tiles navigate *within this same popup* by flipping the
/// `PanelView` scratch signal (mirroring the old hand-created
/// `Signal::new(control_center::PanelView::Main)` in
/// `apps/shell/src/lib.rs`) rather than opening a separate popup — unlike
/// the individual-mode tray icons (and this panel's own back button), which
/// reach the exact same 5 device panels as freestanding popups via
/// `PanelHost`'s normal `open_panel("network"|...)` dispatch. Both paths
/// call the same `build_content` function in each device panel's module
/// (`network::build_content`, `bluetooth::build_content`, etc.), so there is
/// exactly one implementation of each device's UI; only whether an
/// `on_back` closure is threaded through differs.
///
/// # `SharedState` this panel expects pre-populated
/// - `Rc<dyn coconut_api::network::NetworkIntegration>`
/// - `Rc<dyn coconut_api::bluetooth::BluetoothIntegration>`
/// - `Rc<dyn coconut_api::battery::BatteryIntegration>`
/// - `Rc<dyn coconut_api::volume::VolumeIntegration>`
/// - `Rc<dyn coconut_api::brightness::BrightnessIntegration>`
/// - `Rc<dyn coconut_api::power_profile::PowerProfileIntegration>`
/// - [`crate::WifiEnabled`] (`Signal<bool>`)
/// - [`crate::BluetoothPowered`] (`Signal<bool>`)
/// - [`crate::BrightnessLevel`] (`Signal<f32>`)
/// - [`crate::VolumeLevel`] (`Signal<f32>`)
/// - [`crate::KeepAwakeState`] (`Signal<bool>`)
/// - [`crate::ToggleKeepAwake`] (`Rc<dyn Fn()>`)
/// - [`crate::NetworkRevision`], [`crate::BluetoothRevision`],
///   [`crate::BatteryRevision`], [`crate::PowerProfileRevision`]
///   (`Signal<()>` each) — read unconditionally on every rebuild, regardless
///   of which `PanelView` is showing (matching `apps/shell/src/lib.rs`'s old
///   `open_control_center` popup builder, which subscribed to all four
///   before ever dispatching on view), purely for their reactive
///   subscription.
///
/// Drilling into `Network`/`Bluetooth` additionally relies on everything
/// documented on [`network::NetworkPanel`]/[`bluetooth::BluetoothPanel`]
/// being pre-populated too, since `build_content` is shared verbatim
/// (including their own `scratch`-only state).
pub struct ControlCenterPanel {
    tray: TrayConfig,
}

impl ControlCenterPanel {
    pub fn new(tray: TrayConfig) -> Self {
        Self { tray }
    }
}

impl Panel for ControlCenterPanel {
    fn id(&self) -> &'static str {
        "control_center"
    }

    fn title(&self) -> &'static str {
        "Coconut Control Center"
    }

    fn size(&self) -> Size {
        Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        }
    }

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        // Read unconditionally, regardless of which `PanelView` is showing —
        // see the doc comment above.
        if let Some(revision) = ctx.shared.get::<NetworkRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<BluetoothRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<BatteryRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<PowerProfileRevision>() {
            revision.0.get();
        }

        let view = ctx
            .scratch
            .get_or_insert_with(|| Signal::new(PanelView::Main));

        match view.get() {
            PanelView::Network => {
                return network::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    Some(back_to_main(view.clone())),
                );
            }
            PanelView::Bluetooth => {
                return bluetooth::build_content(
                    &ctx.shared,
                    &ctx.scratch,
                    Some(back_to_main(view.clone())),
                );
            }
            PanelView::Energy => {
                return energy::build_content(&ctx.shared, Some(back_to_main(view.clone())));
            }
            PanelView::Brightness => {
                return brightness::build_content(&ctx.shared, Some(back_to_main(view.clone())));
            }
            PanelView::Volume => {
                return volume::build_content(&ctx.shared, Some(back_to_main(view.clone())));
            }
            PanelView::Main => {}
        }

        let network = ctx
            .shared
            .get::<Rc<dyn NetworkIntegration>>()
            .unwrap_or_else(|| Rc::new(NetworkFallback));
        let bluetooth = ctx
            .shared
            .get::<Rc<dyn BluetoothIntegration>>()
            .unwrap_or_else(|| Rc::new(BluetoothFallback));
        let battery = ctx
            .shared
            .get::<Rc<dyn BatteryIntegration>>()
            .unwrap_or_else(|| Rc::new(BatteryFallback));
        let volume = ctx
            .shared
            .get::<Rc<dyn VolumeIntegration>>()
            .unwrap_or_else(|| Rc::new(VolumeFallback));
        let brightness = ctx
            .shared
            .get::<Rc<dyn BrightnessIntegration>>()
            .unwrap_or_else(|| Rc::new(BrightnessFallback));
        let wifi_enabled = ctx
            .shared
            .get::<WifiEnabled>()
            .unwrap_or_else(|| WifiEnabled(Signal::new(network.enabled())))
            .0;
        let bluetooth_powered = ctx
            .shared
            .get::<BluetoothPowered>()
            .unwrap_or_else(|| BluetoothPowered(Signal::new(bluetooth.powered())))
            .0;
        let brightness_level = ctx
            .shared
            .get::<BrightnessLevel>()
            .unwrap_or_else(|| BrightnessLevel(Signal::new(brightness.level())))
            .0;
        let volume_level = ctx
            .shared
            .get::<VolumeLevel>()
            .unwrap_or_else(|| VolumeLevel(Signal::new(volume.level())))
            .0;
        let awake = ctx
            .shared
            .get::<KeepAwakeState>()
            .unwrap_or_else(|| KeepAwakeState(Signal::new(false)))
            .0
            .get();
        let toggle_awake = ctx
            .shared
            .get::<ToggleKeepAwake>()
            .unwrap_or_else(|| ToggleKeepAwake(Rc::new(|| {})))
            .0;

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
        if self.tray.wifi.shows_in_panel() {
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
        if self.tray.bluetooth.shows_in_panel() {
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
        if self.tray.battery.shows_in_panel() {
            status_row = status_row.child(status_tile(
                battery_icon,
                "Battery",
                battery_value,
                battery_caption,
                open_energy_view,
            ));
        }
        status_row = status_row.child(awake_tile(awake, toggle_awake));

        let brightness_card: BoxedWidget = if self.tray.brightness.shows_in_panel() {
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
            // Keep the same grown, styled island even with nothing to show —
            // an unstyled `<Flex/>` collapses to zero height, leaving a gap
            // of bare (transparent) window below the wifi/battery cards
            // instead.
            Box::new(slider_container())
        };
        let volume_card: BoxedWidget = if self.tray.volume.shows_in_panel() {
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
            Box::new(slider_container())
        };

        Box::new(jsx! {
            <Flex direction={FlexDirection::Column} size={(WIDTH as f32, HEIGHT as f32)} padding={16.0} gap={12.0} background={shell_card()} border={(shell_border(), 1.0)} corner_radius={CARD_RADIUS}>
                <RawText color={shell_muted()} font_size={14.0}>"CONTROL"</RawText>
                {Box::new(device_row) as BoxedWidget}
                {Box::new(status_row) as BoxedWidget}
                {brightness_card}
                {volume_card}
            </Flex>
        })
    }
}

/// A single slider's own full-width card — brightness and volume each get
/// one, rather than sharing a single box, but both still grow to fill
/// whatever vertical space is left, split evenly between them.
fn slider_container() -> Flex {
    Flex::column()
        .grow(1.0)
        .padding(12.0)
        .justify(creamui_widgets::layout::Justify::Center)
        .property(StyleProp::Background(shell_panel().into()))
        .property(StyleProp::Border(Border::new(shell_border(), 1.0)))
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
    let mut name = RawMarquee::new(name, shell_text(), 15.0, TILE_W - 20.0);
    name.style.layout.size.height = creamui_core::layout::Dimension::Length(18.0);
    let name = Box::new(name) as BoxedWidget;
    let content = Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE_W, TILE_H)} padding={10.0} gap={7.0}>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                {coconut_plugin_kit::pixel_icon(icon, 16.0, shell_text())}
                <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>{kicker}</RawText>
                <Flex grow={1.0} />
                {compact_switch(enabled, on_toggle)}
            </Flex>
            {name}
            <RawText color={shell_muted()} font_size={10.0} align={TextAlign::Start}>{caption}</RawText>
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
                {coconut_plugin_kit::pixel_icon(icon, 16.0, shell_text())}
                <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>{kicker}</RawText>
            </Flex>
            <Flex direction={FlexDirection::Row} align={Align::Center} gap={8.0}>
                <RawText color={shell_text()} font_size={18.0} align={TextAlign::Start}>{value}</RawText>
                <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>{caption}</RawText>
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
        .background(shell_panel())
        .border(shell_border(), 1.0)
        .corner_radius(ISLAND_RADIUS)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}

fn back_to_main(view: Signal<PanelView>) -> Rc<dyn Fn()> {
    Rc::new(move || view.set(PanelView::Main))
}

fn awake_tile(awake: bool, toggle_awake: Rc<dyn Fn()>) -> BoxedWidget {
    let caption = if awake { "Holding" } else { "Idle" };
    Box::new(jsx! {
        <Flex direction={FlexDirection::Column} size={(TILE_W, TILE_H)} padding={10.0} gap={6.0} background={shell_panel()} border={(shell_border(), 1.0)} corner_radius={ISLAND_RADIUS} justify={creamui_widgets::layout::Justify::Center}>
            <Flex direction={FlexDirection::Row} gap={8.0} align={Align::Center}>
                <RawText color={shell_muted()} font_size={11.0} align={TextAlign::Start}>"Keep awake"</RawText>
                <Flex grow={1.0} />
                {compact_switch(awake, toggle_awake)}
            </Flex>
            <RawText color={shell_text()} font_size={15.0} align={TextAlign::Start}>{caption}</RawText>
        </Flex>
    })
}
