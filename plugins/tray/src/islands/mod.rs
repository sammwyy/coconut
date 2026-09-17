//! Shared tray-icon mapping helpers and dock-button styling used by both
//! [`grouped::ControlCenterIsland`] (one fused button) and
//! [`individual::build_islands`] (one button per device) — ported from the
//! free functions in `apps/shell/src/bar/mod.rs`
//! (`network_icon`/`battery_icon`/`volume_icon`/`island_button_style`; the
//! inline `if status.bluetooth_connected {...}` branch there is lifted out
//! into `bluetooth_icon` here since it's now needed in two places instead of
//! one).

pub mod grouped;
pub mod individual;

use coconut_plugin_kit::chrome::{shell_control_hover, shell_panel, shell_selected};
use creamui_core::layout::Style as LayoutStyle;
use creamui_core::{StateStyle, Style};
use creamui_widgets::layout::fixed;

pub(crate) fn network_icon(connected: bool, strength: Option<u8>) -> &'static str {
    if !connected {
        return "wifi-slash";
    }
    match strength.unwrap_or(100) {
        75.. => "wifi-excellent",
        50..=74 => "wifi-good",
        25..=49 => "wifi-fair",
        _ => "wifi-weak",
    }
}

pub(crate) fn battery_icon(percentage: Option<u8>, charging: bool) -> &'static str {
    if charging {
        return "battery-bolt";
    }
    match percentage.unwrap_or(0) {
        80.. => "battery-full",
        50..=79 => "battery-mid",
        20..=49 => "battery-low",
        _ => "battery-empty",
    }
}

pub(crate) fn volume_icon(level: f32, muted: bool) -> &'static str {
    if muted || level <= 0.01 {
        "volume-mute"
    } else if level < 0.34 {
        "volume-low"
    } else if level < 0.67 {
        "volume-mid"
    } else {
        "volume-high"
    }
}

pub(crate) fn bluetooth_icon(connected: bool, powered: bool) -> &'static str {
    if connected {
        "bluetooth-connected"
    } else if powered {
        "bluetooth-on"
    } else {
        "bluetooth-off"
    }
}

/// A fixed-size dock button matching every other island's look — ported
/// verbatim from `apps/shell/src/bar/mod.rs::island_button_style` (its
/// `island_color`/`control_hover`/`selected_color` theme helpers are the
/// exact same colors as `coconut_plugin_kit::chrome`'s `shell_panel`/
/// `shell_control_hover`/`shell_selected`, just renamed during the Phase 3
/// chrome unification).
pub(crate) fn island_button_style(width: f32, height: f32) -> Style {
    Style::new()
        .layout(LayoutStyle {
            size: fixed(width, height),
            ..Default::default()
        })
        .background(shell_panel())
        .corner_radius(9.0)
        .hover(StateStyle::new().background(shell_control_hover()))
        .pressed(StateStyle::new().background(shell_selected()))
}
