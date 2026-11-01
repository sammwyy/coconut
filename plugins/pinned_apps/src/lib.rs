mod view;

use coconut_plugin_kit::{
    pins::Pins, AppCatalog, Island, IslandRenderContext, Panel, PanelRenderContext, Plugin,
    PluginEvent, PluginEventHandler, PluginInitContext,
};
use creamui_core::{BoxedWidget, Size};
use std::rc::Rc;

pub struct PinnedAppsPlugin;
pub struct PinnedAppsIsland;
pub struct PinApplicationPanel;

impl Plugin for PinnedAppsPlugin {
    fn id(&self) -> &'static str {
        "pinned_apps"
    }

    fn islands(&self, ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        Pins::ensure(ctx);
        AppCatalog::ensure(ctx);
        vec![Rc::new(PinnedAppsIsland)]
    }

    fn panels(&self, _: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        vec![Rc::new(PinApplicationPanel)]
    }

    fn event_handler(&self, ctx: &PluginInitContext) -> Option<PluginEventHandler> {
        let pins = Pins::ensure(ctx);
        let catalog = AppCatalog::ensure(ctx);
        let app = ctx.app.clone();
        Some(Rc::new(move |event| match event {
            PluginEvent::ModuleConfigChanged => pins.refresh(),
            PluginEvent::IconThemeChanged(theme) => {
                catalog.set_icon_theme(&app, theme.clone());
                pins.set_icon_theme(theme.clone());
            }
        }))
    }
}

impl Island for PinnedAppsIsland {
    fn id(&self) -> &'static str {
        "pinned_apps"
    }
    fn build(&self, ctx: &IslandRenderContext<'_>) -> BoxedWidget {
        view::island(ctx)
    }
}

impl Panel for PinApplicationPanel {
    fn id(&self) -> &'static str {
        "pin_application"
    }
    fn title(&self) -> &'static str {
        "Pin application"
    }
    fn size(&self) -> Size {
        Size {
            width: 360.0,
            height: 480.0,
        }
    }
    fn autofocus(&self) -> bool {
        true
    }
    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget {
        view::picker(ctx)
    }
}
