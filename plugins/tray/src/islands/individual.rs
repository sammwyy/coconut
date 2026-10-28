use crate::config::TrayConfig;
use crate::islands::{
    battery_icon, bluetooth_icon, island_button_style, network_icon, volume_icon,
};
use crate::shared::{BatteryRevision, BluetoothRevision, NetworkRevision, VolumeLevel};
use coconut_api::battery::{BatteryIntegration, Fallback as BatteryFallback};
use coconut_api::bluetooth::{BluetoothIntegration, Fallback as BluetoothFallback};
use coconut_api::network::{Fallback as NetworkFallback, NetworkIntegration};
use coconut_api::volume::{Fallback as VolumeFallback, VolumeIntegration};
use coconut_plugin_kit::chrome::shell_text;
use coconut_plugin_kit::{pixel_icon, Island, IslandRenderContext};
use creamui_core::{BoxedWidget, Point};
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::RawButton;
use std::rc::Rc;

/// Which device an individual-mode tray icon represents, and which panel it
/// opens. Ported from `desktop/shell/src/bar/mod.rs::individual_tray_row`,
/// which fused every enabled icon into one row/dock-slot; here each becomes
/// its own [`Island`] with a stable id (`"tray.<device>"`) so `shell.toml`
/// can place and reorder them independently — a deliberate improvement over
/// the old single-fused-row behavior, called out in the refactor plan's
/// Phase 4c section.
#[derive(Clone, Copy)]
enum Device {
    Wifi,
    Bluetooth,
    Battery,
    Volume,
    Brightness,
}

impl Device {
    fn id(self) -> &'static str {
        match self {
            Self::Wifi => "tray.wifi",
            Self::Bluetooth => "tray.bluetooth",
            Self::Battery => "tray.battery",
            Self::Volume => "tray.volume",
            Self::Brightness => "tray.brightness",
        }
    }

    /// The panel this device's icon opens. Battery opens `"energy"`, not a
    /// `"battery"` panel (there isn't one) — matches
    /// `desktop/shell/src/bar/mod.rs::individual_tray_row`'s
    /// `tray_icon_button(battery_icon, open_energy)` wiring exactly.
    fn panel_id(self) -> &'static str {
        match self {
            Self::Wifi => "network",
            Self::Bluetooth => "bluetooth",
            Self::Battery => "energy",
            Self::Volume => "volume",
            Self::Brightness => "brightness",
        }
    }

    fn shows_in_bar(self, tray: &TrayConfig) -> bool {
        match self {
            Self::Wifi => tray.wifi.shows_in_bar(),
            Self::Bluetooth => tray.bluetooth.shows_in_bar(),
            Self::Battery => tray.battery.shows_in_bar(),
            Self::Volume => tray.volume.shows_in_bar(),
            Self::Brightness => tray.brightness.shows_in_bar(),
        }
    }
}

/// Builds one [`Island`] per device whose [`crate::config::TrayVisibility`]
/// is `shows_in_bar()`, in the same wifi/brightness/volume/bluetooth/battery
/// order `individual_tray_row` used — 0 to 5 islands depending on
/// `modules/tray.toml`.
pub fn build_islands(tray: &TrayConfig) -> Vec<Rc<dyn Island>> {
    [
        Device::Wifi,
        Device::Brightness,
        Device::Volume,
        Device::Bluetooth,
        Device::Battery,
    ]
    .into_iter()
    .filter(|device| device.shows_in_bar(tray))
    .map(|device| Rc::new(DeviceIsland { device }) as Rc<dyn Island>)
    .collect()
}

struct DeviceIsland {
    device: Device,
}

impl Island for DeviceIsland {
    fn id(&self) -> &'static str {
        self.device.id()
    }

    fn build(&self, ctx: &IslandRenderContext<'_>) -> BoxedWidget {
        let icon = self.icon_name(ctx);
        let panel_id = self.device.panel_id();
        let open_panel = ctx.open_panel.clone();
        if matches!(self.device, Device::Battery)
            && coconut_core::modules::load_module_value("tray")
                .get("show_battery_percentage")
                .and_then(|value| value.as_bool())
                .unwrap_or(false)
        {
            if let Some(percent) = ctx
                .shared
                .get::<Rc<dyn BatteryIntegration>>()
                .and_then(|battery| battery.percentage())
            {
                use creamui_core::Styled;
                let s = ctx.scale;
                let row = Flex::row()
                    .size(74.0 * s, 32.0 * s)
                    .gap(6.0 * s)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .child(pixel_icon(icon, 16.0 * s, shell_text()))
                    .child(Box::new(
                        creamui_widgets::Text::new(format!("{percent}%"))
                            .font_size(11.0 * s)
                            .bold(true),
                    ));
                return Box::new(
                    RawButton::new(island_button_style(74.0 * s, 32.0 * s, s), || {})
                        .with_click_position(move |point| open_panel(panel_id, point))
                        .child(Box::new(row)),
                );
            }
        }
        tray_icon_button(icon, move |point| open_panel(panel_id, point), ctx.scale)
    }
}

impl DeviceIsland {
    fn icon_name(&self, ctx: &IslandRenderContext<'_>) -> &'static str {
        match self.device {
            Device::Wifi => {
                if let Some(revision) = ctx.shared.get::<NetworkRevision>() {
                    revision.0.get();
                }
                let network = ctx
                    .shared
                    .get::<Rc<dyn NetworkIntegration>>()
                    .unwrap_or_else(|| Rc::new(NetworkFallback));
                network_icon(network.connected(), network.strength())
            }
            Device::Bluetooth => {
                if let Some(revision) = ctx.shared.get::<BluetoothRevision>() {
                    revision.0.get();
                }
                let bluetooth = ctx
                    .shared
                    .get::<Rc<dyn BluetoothIntegration>>()
                    .unwrap_or_else(|| Rc::new(BluetoothFallback));
                bluetooth_icon(bluetooth.connected(), bluetooth.powered())
            }
            Device::Battery => {
                if let Some(revision) = ctx.shared.get::<BatteryRevision>() {
                    revision.0.get();
                }
                let battery = ctx
                    .shared
                    .get::<Rc<dyn BatteryIntegration>>()
                    .unwrap_or_else(|| Rc::new(BatteryFallback));
                battery_icon(battery.percentage(), battery.charging())
            }
            Device::Volume => {
                let volume = ctx
                    .shared
                    .get::<Rc<dyn VolumeIntegration>>()
                    .unwrap_or_else(|| Rc::new(VolumeFallback));
                let level = ctx
                    .shared
                    .get::<VolumeLevel>()
                    .map(|level| level.0.get())
                    .unwrap_or(0.0);
                volume_icon(level, volume.muted())
            }
            // No per-level brightness icon exists in the bundled icon set —
            // matches `individual_tray_row`'s static `"brightness"` icon.
            Device::Brightness => "brightness",
        }
    }
}

fn tray_icon_button(
    icon: &'static str,
    on_click: impl Fn(Point) + 'static,
    scale: f32,
) -> BoxedWidget {
    let size = 32.0 * scale;
    Box::new(
        RawButton::new(island_button_style(size, size, scale), || {})
            .with_click_position(on_click)
            .child(Box::new(
                Flex::row()
                    .size(size, size)
                    .align(Align::Center)
                    .justify(Justify::Center)
                    .child(pixel_icon(icon, 16.0 * scale, shell_text())),
            )),
    )
}
