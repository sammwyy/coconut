//! `coconut-plugin-logo`: the dock's brand-mark island. No panel, no shared
//! state, no per-instance config — the simplest possible plugin, migrated
//! first in Phase 4a of the dock/island/panel refactor
//! (`~/.claude/plans/luminous-prancing-wombat.md`).

mod island;

pub use island::LogoIsland;

use coconut_plugin_kit::{Island, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct LogoPlugin;

impl Plugin for LogoPlugin {
    fn id(&self) -> &'static str {
        "logo"
    }

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![Rc::new(LogoIsland)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_island_ids_match_shell_toml() {
        assert_eq!(LogoPlugin.id(), "logo");
        assert_eq!(LogoIsland.id(), "logo");
    }
}
