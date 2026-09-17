//! `coconut-plugin-app-drawer`: the searchable/categorized app grid popup
//! opened by the paired `coconut-plugin-app-launcher` crate's
//! `AppLauncherIsland`. Contributes only a `Panel`, no `Island` (Phase 4b of
//! the dock/island/panel refactor,
//! `~/.claude/plans/luminous-prancing-wombat.md`).
//!
//! See `panel.rs`'s module doc comment for how `AppCatalog` (the
//! background app scan/icon cache, which needs a live `AppHandle` and the
//! configured icon theme) is built once here at plugin-init time and handed
//! to `Panel::build` via `SharedState`.

mod panel;
mod process;

pub use panel::{AppCatalog, AppDrawerPanel, DrawerState};

use coconut_plugin_kit::{Panel, Plugin, PluginInitContext};
use std::rc::Rc;

pub struct AppDrawerPlugin;

impl Plugin for AppDrawerPlugin {
    fn id(&self) -> &'static str {
        "app_drawer"
    }

    fn panels(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        let catalog = AppCatalog::new();
        // The icon theme lives in `shell.toml`'s `[appearance]` table, not a
        // per-plugin `modules/<id>.toml` (so `ctx.module_config` doesn't
        // apply here) — read it the same way `apps/shell/src/lib.rs` used to
        // read `initial_config.appearance.icon_theme` before `AppBuilder`
        // even started.
        let icon_theme = coconut_core::ShellConfig::load().appearance.icon_theme;
        catalog.start_loading(&ctx.app, icon_theme);
        // Stashed so `Panel::build` (and, later, Phase 4d's config-reload
        // handling) can fetch this exact instance back out — see panel.rs's
        // module doc comment.
        ctx.shared.insert(catalog);
        vec![Rc::new(AppDrawerPanel)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_panel_ids_match_the_launcher_s_open_panel_call() {
        assert_eq!(AppDrawerPlugin.id(), "app_drawer");
        assert_eq!(AppDrawerPanel.id(), "app_drawer");
    }
}
