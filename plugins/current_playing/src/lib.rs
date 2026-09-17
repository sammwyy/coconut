//! `coconut-plugin-current-playing`: the dock's now-playing island and
//! panel. Phase 4a of the dock/island/panel refactor
//! (`~/.claude/plans/luminous-prancing-wombat.md`).

mod island;
mod panel;
mod shared;

pub use island::CurrentPlayingIsland;
pub use panel::CurrentPlayingPanel;
pub use shared::{CurrentPlayback, NextPlayback, PreviousPlayback, SeekPlayback, TogglePlayback};

use coconut_plugin_kit::{Island, Panel, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct CurrentPlayingPlugin;

impl Plugin for CurrentPlayingPlugin {
    fn id(&self) -> &'static str {
        "current_playing"
    }

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![Rc::new(CurrentPlayingIsland)]
    }

    fn panels(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        vec![Rc::new(CurrentPlayingPanel)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_island_and_panel_ids_match_shell_toml() {
        assert_eq!(CurrentPlayingPlugin.id(), "current_playing");
        assert_eq!(CurrentPlayingIsland.id(), "current_playing");
        assert_eq!(CurrentPlayingPanel.id(), "current_playing");
    }
}
