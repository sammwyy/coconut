//! `coconut-plugin-app-launcher`: the dock's app-launcher island — the
//! button that opens the app drawer, plus the open-window task strip drawn
//! alongside it in both the horizontal and vertical/side dock orientations.
//!
//! Paired with `coconut-plugin-app-drawer` (Phase 4b of the dock/island/
//! panel refactor, `~/.claude/plans/luminous-prancing-wombat.md`): this
//! crate contributes only an `Island`, no `Panel`, and opens the drawer by
//! calling `(ctx.open_panel)("app_drawer", at)`.

mod island;

pub use island::{AppLauncherIsland, LauncherOpenSignal, WindowListState};

use coconut_plugin_kit::{Island, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct AppLauncherPlugin;

impl Plugin for AppLauncherPlugin {
    fn id(&self) -> &'static str {
        "app_launcher"
    }

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![Rc::new(AppLauncherIsland)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_island_ids_match_shell_toml() {
        assert_eq!(AppLauncherPlugin.id(), "app_launcher");
        assert_eq!(AppLauncherIsland.id(), "app_launcher");
    }
}
