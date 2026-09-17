use serde::{Deserialize, Serialize};

/// The clock plugin's own `modules/clock.toml` config (loaded via
/// [`coconut_plugin_kit::PluginInitContext::module_config`]). Mirrors the
/// old, now-deleted `crates/core/src/widgets.rs::ClockWidgetConfig` minus its
/// `enabled` field — presence/absence in `shell.toml`'s
/// `[[dock.section.island]]` list now controls whether the clock shows at
/// all, so only the format pattern remains plugin-owned config.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClockConfig {
    /// `chrono::format::strftime` pattern, shared by both the dock island
    /// and the expanded clock panel (previously the panel ignored this and
    /// hardcoded its own "%H:%M" — fixed here while rewriting both anyway).
    pub format: String,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            format: "%H:%M".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_24_hour_hour_minute() {
        assert_eq!(ClockConfig::default().format, "%H:%M");
    }
}
