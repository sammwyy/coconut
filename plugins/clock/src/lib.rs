//! `coconut-plugin-clock`: the dock's clock island and its expanded panel.
//! Phase 4a of the dock/island/panel refactor
//! (`~/.claude/plans/luminous-prancing-wombat.md`).

mod config;
mod island;
mod panel;
mod shared;

pub use config::ClockConfig;
pub use island::ClockIsland;
pub use panel::ClockPanel;
pub use shared::ClockText;

use coconut_plugin_kit::{Island, Panel, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct ClockPlugin;

impl Plugin for ClockPlugin {
    fn id(&self) -> &'static str {
        "clock"
    }

    fn islands(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        let config: ClockConfig = ctx.module_config("clock");
        vec![Rc::new(ClockIsland {
            format: config.format,
        })]
    }

    fn panels(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        let config: ClockConfig = ctx.module_config("clock");
        vec![Rc::new(ClockPanel {
            format: config.format,
        })]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_island_and_panel_ids_match_shell_toml() {
        assert_eq!(ClockPlugin.id(), "clock");
        assert_eq!(
            ClockIsland {
                format: "%H:%M".to_owned()
            }
            .id(),
            "clock"
        );
        assert_eq!(
            ClockPanel {
                format: "%H:%M".to_owned()
            }
            .id(),
            "clock"
        );
    }
}
