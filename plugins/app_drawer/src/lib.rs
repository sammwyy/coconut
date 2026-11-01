mod panel;

pub use coconut_plugin_kit::AppCatalog;
pub use panel::{AppDrawerPanel, DrawerState};

use coconut_plugin_kit::{Panel, Plugin, PluginEvent, PluginEventHandler, PluginInitContext};
use std::rc::Rc;

pub struct AppDrawerPlugin;

impl Plugin for AppDrawerPlugin {
    fn id(&self) -> &'static str {
        "app_drawer"
    }

    fn panels(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        AppCatalog::ensure(ctx);
        coconut_plugin_kit::pins::Pins::ensure(ctx);
        vec![Rc::new(AppDrawerPanel)]
    }

    fn event_handler(&self, ctx: &PluginInitContext) -> Option<PluginEventHandler> {
        let catalog = AppCatalog::ensure(ctx);
        let app = ctx.app.clone();
        Some(Rc::new(move |event| {
            if let PluginEvent::IconThemeChanged(theme) = event {
                catalog.set_icon_theme(&app, theme.clone());
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_and_panel_ids_match_the_launcher_s_open_panel_call() {
        assert_eq!(AppDrawerPlugin.id(), "app_drawer");
        assert_eq!(AppDrawerPanel.id(), "app_drawer");
        assert!(AppDrawerPanel.autofocus());
    }
}
