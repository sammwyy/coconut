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

use creamui_core::Style;

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

/// Tray buttons share the same glass chrome as the other dock islands.
pub(crate) fn island_button_style(width: f32, height: f32, scale: f32) -> Style {
    coconut_plugin_kit::chrome::island_style(width, height, scale)
}
