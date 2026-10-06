//! `coconut-plugin-logo`: the statusbar's compact workspace selector. It has
//! no panel, shared state, or configuration yet.

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
