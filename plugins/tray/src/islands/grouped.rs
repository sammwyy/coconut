use crate::config::TrayConfig;
use crate::islands::{battery_icon, bluetooth_icon, island_button_style, network_icon, volume_icon};
use crate::shared::{BatteryRevision, BluetoothRevision, NetworkRevision, VolumeLevel};
use coconut_api::battery::{BatteryIntegration, Fallback as BatteryFallback};
use coconut_api::bluetooth::{BluetoothIntegration, Fallback as BluetoothFallback};
use coconut_api::network::{Fallback as NetworkFallback, NetworkIntegration};
use coconut_api::volume::{Fallback as VolumeFallback, VolumeIntegration};
use coconut_plugin_kit::chrome::shell_text;
use coconut_plugin_kit::{pixel_icon, Island, IslandRenderContext};
use creamui_core::BoxedWidget;
use creamui_widgets::layout::{Align, Flex, Justify};
use creamui_widgets::RawButton;
use std::rc::Rc;

/// The single fused tray button shown when `[modules/tray.toml] mode =
/// "grouped"` (the default): every enabled device gets one icon inside this
/// button, which opens the `"control_center"` panel. Ported from
/// `apps/shell/src/bar/mod.rs::control_button_widget`.
///
/// See [`crate::shared`] for the exact `SharedState` entries this island
/// expects `apps/shell/src/lib.rs` (Phase 4d) to provide.
pub struct ControlCenterIsland {
    tray: TrayConfig,
}

impl ControlCenterIsland {
    pub fn new(tray: TrayConfig) -> Self {
        Self { tray }
    }
}

impl Island for ControlCenterIsland {
    fn id(&self) -> &'static str {
        "control_center"
    }

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget {
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

        // Subscribing to these revisions here (mirroring the old dock
        // rebuild closure's `network_revision.get(); ...` calls in
        // `apps/shell/src/lib.rs`) makes this island's own rebuild reactive
        // to device changes even if the engine's dock-rebuild trigger set
        // doesn't already cover them.
        if let Some(revision) = ctx.shared.get::<NetworkRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<BluetoothRevision>() {
            revision.0.get();
        }
        if let Some(revision) = ctx.shared.get::<BatteryRevision>() {
            revision.0.get();
        }
        let volume_level = ctx
            .shared
            .get::<VolumeLevel>()
            .map(|level| level.0.get())
            .unwrap_or(0.0);

        let network_icon_name = network_icon(network.connected(), network.strength());
        let battery_icon_name = battery_icon(battery.percentage(), battery.charging());
        let bluetooth_icon_name = bluetooth_icon(bluetooth.connected(), bluetooth.powered());
        let volume_icon_name = volume_icon(volume_level, volume.muted());

        let mut icon_count = 0;
        let mut row = Flex::row()
            .padding(4.0)
            .gap(7.0)
            .justify(Justify::Center)
            .align(Align::Center);
        if self.tray.wifi.shows_in_bar() {
            row = row.child(pixel_icon(network_icon_name, 16.0, shell_text()));
            icon_count += 1;
        }
        if self.tray.brightness.shows_in_bar() {
            row = row.child(pixel_icon("brightness", 16.0, shell_text()));
            icon_count += 1;
        }
        if self.tray.volume.shows_in_bar() {
            row = row.child(pixel_icon(volume_icon_name, 16.0, shell_text()));
            icon_count += 1;
        }
        if self.tray.bluetooth.shows_in_bar() {
            row = row.child(pixel_icon(bluetooth_icon_name, 16.0, shell_text()));
            icon_count += 1;
        }
        if self.tray.battery.shows_in_bar() {
            row = row.child(pixel_icon(battery_icon_name, 16.0, shell_text()));
            icon_count += 1;
        }
        row = row.child(pixel_icon("chevron-up", 13.0, shell_text()));
        let width = control_button_width(icon_count);
        row = row.size(width, 32.0);

        let open_panel = ctx.open_panel.clone();
        Box::new(
            RawButton::new(island_button_style(width, 32.0), || {})
                .with_click_position(move |point| open_panel("control_center", point))
                .child(Box::new(row)),
        )
    }
}

/// Grows with the number of visible device icons, plus the trailing
/// chevron. Ported verbatim from
/// `apps/shell/src/bar/mod.rs::control_button_width`.
fn control_button_width(icon_count: usize) -> f32 {
    const ICON_WIDTH: f32 = 16.0;
    const CHEVRON_WIDTH: f32 = 13.0;
    const GAP: f32 = 7.0;
    const PADDING: f32 = 8.0;
    let icons = icon_count as f32;
    PADDING + icons * ICON_WIDTH + (icons + 1.0) * GAP + CHEVRON_WIDTH
}

#[cfg(test)]
mod tests {
    use super::control_button_width;

    /// Moved from `apps/shell/src/bar/mod.rs::control_button_grows_with_visible_icons`.
    #[test]
    fn control_button_grows_with_visible_icons() {
        assert!(control_button_width(0) < control_button_width(3));
        assert!(control_button_width(3) < control_button_width(5));
    }
}
