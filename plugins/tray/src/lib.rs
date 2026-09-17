//! `coconut-plugin-tray`: the dock's tray islands (a single grouped
//! "control_center" button, or up to 5 independent per-device islands) and
//! the control center, network, bluetooth, energy, brightness and volume
//! panels they open. The most complex plugin in the dock/island/panel
//! refactor (`~/.claude/plans/luminous-prancing-wombat.md`, Phase 4c).
//!
//! See [`shared`] for the exact `SharedState` contract `apps/shell/src/lib.rs`
//! (Phase 4d) must provide.

mod config;
mod islands;
mod panels;
mod shared;

pub use config::{TrayConfig, TrayMode, TrayVisibility};
pub use islands::grouped::ControlCenterIsland;
pub use panels::bluetooth::BluetoothPanel;
pub use panels::brightness::BrightnessPanel;
pub use panels::control_center::ControlCenterPanel;
pub use panels::energy::EnergyPanel;
pub use panels::network::NetworkPanel;
pub use panels::volume::VolumePanel;
pub use shared::{
    BatteryRevision, BluetoothDetailView, BluetoothPowered, BluetoothRevision, BluetoothScroll,
    BrightnessLevel, KeepAwakeState, NetworkDetailView, NetworkPasswordReveal, NetworkRevision,
    NetworkScroll, PowerProfileRevision, ToggleKeepAwake, VolumeLevel, WifiEnabled,
};

use coconut_plugin_kit::{Island, Panel, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct TrayPlugin;

impl Plugin for TrayPlugin {
    fn id(&self) -> &'static str {
        "tray"
    }

    /// `TrayMode::Grouped` (the default) registers a single `"control_center"`
    /// island; `TrayMode::Individual` registers 0-5 `"tray.<device>"`
    /// islands, one per device whose `TrayVisibility::shows_in_bar()` is
    /// true.
    fn islands(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        let tray: TrayConfig = ctx.module_config("tray");
        match tray.mode {
            TrayMode::Grouped => vec![Rc::new(ControlCenterIsland::new(tray)) as Rc<dyn Island>],
            TrayMode::Individual => islands::individual::build_islands(&tray),
        }
    }

    /// All 6 panels are always registered, regardless of `TrayConfig::mode`:
    /// the 5 device panels are reachable both directly (individual tray
    /// mode's icons, or `PanelHost::open` from anywhere else) and via the
    /// control center's own internal navigation, so they must exist whether
    /// or not any individual-mode island currently points at them.
    fn panels(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        let tray: TrayConfig = ctx.module_config("tray");
        vec![
            Rc::new(ControlCenterPanel::new(tray)) as Rc<dyn Panel>,
            Rc::new(NetworkPanel),
            Rc::new(BluetoothPanel),
            Rc::new(EnergyPanel),
            Rc::new(BrightnessPanel),
            Rc::new(VolumePanel),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_id_matches_module_config_file_name() {
        assert_eq!(TrayPlugin.id(), "tray");
    }

    #[test]
    fn panel_ids_match_the_ids_islands_open() {
        assert_eq!(ControlCenterPanel::new(TrayConfig::default()).id(), "control_center");
        assert_eq!(NetworkPanel.id(), "network");
        assert_eq!(BluetoothPanel.id(), "bluetooth");
        assert_eq!(EnergyPanel.id(), "energy");
        assert_eq!(BrightnessPanel.id(), "brightness");
        assert_eq!(VolumePanel.id(), "volume");
    }

    #[test]
    fn grouped_island_id_is_control_center() {
        assert_eq!(
            ControlCenterIsland::new(TrayConfig::default()).id(),
            "control_center"
        );
    }

    #[test]
    fn individual_islands_use_stable_per_device_ids_and_respect_visibility() {
        let mut tray = TrayConfig {
            mode: TrayMode::Individual,
            ..TrayConfig::default()
        };
        tray.bluetooth = TrayVisibility::Hidden;
        let islands = islands::individual::build_islands(&tray);
        let ids: Vec<&str> = islands.iter().map(|island| island.id()).collect();
        assert_eq!(ids, vec!["tray.wifi", "tray.brightness", "tray.volume", "tray.battery"]);
    }
}
