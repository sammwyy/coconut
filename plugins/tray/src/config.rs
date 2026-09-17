use serde::{Deserialize, Serialize};

/// How a tray/status icon is exposed to the user.
///
/// Moved verbatim from the old `crates/core/src/tray.rs` (deleted in Phase 1
/// of the dock/island/panel refactor, `~/.claude/plans/luminous-prancing-wombat.md`)
/// into this plugin, which is now the only consumer of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayVisibility {
    /// Always shown in the collapsed bar strip.
    Always,
    /// Not shown in the bar, but still available in the control center panel.
    Hidden,
    /// Never shown anywhere.
    Off,
}

impl Default for TrayVisibility {
    fn default() -> Self {
        TrayVisibility::Always
    }
}

impl TrayVisibility {
    pub fn shows_in_bar(self) -> bool {
        matches!(self, TrayVisibility::Always)
    }

    pub fn shows_in_panel(self) -> bool {
        !matches!(self, TrayVisibility::Off)
    }
}

/// How the tray icons in the dock open their controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrayMode {
    /// A single button shows every enabled icon and opens the grouped
    /// control center; its device tiles link to their dedicated panels.
    Grouped,
    /// Each enabled icon is its own island and opens its device's dedicated
    /// panel directly.
    Individual,
}

impl Default for TrayMode {
    fn default() -> Self {
        TrayMode::Grouped
    }
}

/// The tray plugin's own `modules/tray.toml` config (loaded via
/// [`coconut_plugin_kit::PluginInitContext::module_config`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrayConfig {
    pub mode: TrayMode,
    pub wifi: TrayVisibility,
    pub bluetooth: TrayVisibility,
    pub battery: TrayVisibility,
    pub volume: TrayVisibility,
    pub brightness: TrayVisibility,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            mode: TrayMode::default(),
            wifi: TrayVisibility::default(),
            bluetooth: TrayVisibility::default(),
            battery: TrayVisibility::default(),
            volume: TrayVisibility::default(),
            brightness: TrayVisibility::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TrayVisibility;

    #[test]
    fn always_shows_everywhere() {
        assert!(TrayVisibility::Always.shows_in_bar());
        assert!(TrayVisibility::Always.shows_in_panel());
    }

    #[test]
    fn hidden_skips_the_bar_but_keeps_the_panel() {
        assert!(!TrayVisibility::Hidden.shows_in_bar());
        assert!(TrayVisibility::Hidden.shows_in_panel());
    }

    #[test]
    fn off_skips_everywhere() {
        assert!(!TrayVisibility::Off.shows_in_bar());
        assert!(!TrayVisibility::Off.shows_in_panel());
    }
}
