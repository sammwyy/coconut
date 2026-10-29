//! `coconut-plugin-workspaces`: the statusbar's workspace switcher. It has
//! no panel or configuration; the compositor drives the chip set.

mod island;

pub use island::{WorkspaceListState, WorkspacesIsland};

use coconut_plugin_kit::{Island, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct WorkspacesPlugin;

impl Plugin for WorkspacesPlugin {
    fn id(&self) -> &'static str {
        "workspaces"
    }

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        vec![
            Rc::new(WorkspacesIsland("workspaces")),
            // "logo" was this island's original id; keep resolving it so
            // existing shell.toml placements survive the rename.
            Rc::new(WorkspacesIsland("logo")),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_island_ids_match_shell_toml() {
        assert_eq!(WorkspacesPlugin.id(), "workspaces");
        assert_eq!(WorkspacesIsland("workspaces").id(), "workspaces");
        assert_eq!(WorkspacesIsland("logo").id(), "logo");
    }
}
