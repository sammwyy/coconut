//! `coconut-plugin-app-launcher`: two islands — [`AppLauncherIsland`] (opens
//! the app drawer) and [`OpenWindowsIsland`] (the open-window task strip) —
//! sharing this crate for their icon-decoding/caching helpers.
//!
//! Paired with `coconut-plugin-app-drawer`: this crate contributes only
//! `Island`s, no `Panel`, and opens the drawer by calling
//! `(ctx.open_panel)("app_drawer", at)`.

mod island;

pub use island::{AppLauncherIsland, LauncherOpenSignal, OpenWindowsIsland, WindowListState};

use coconut_plugin_kit::{Island, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct AppLauncherPlugin;

impl Plugin for AppLauncherPlugin {
    fn id(&self) -> &'static str {
        "app_launcher"
    }

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![Rc::new(AppLauncherIsland), Rc::new(OpenWindowsIsland)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_island_ids_match_shell_toml() {
        assert_eq!(AppLauncherPlugin.id(), "app_launcher");
        assert_eq!(AppLauncherIsland.id(), "app_launcher");
        assert_eq!(OpenWindowsIsland.id(), "open_windows");
    }
}
