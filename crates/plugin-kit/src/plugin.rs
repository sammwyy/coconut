use crate::{Island, Panel, SharedState};
use creamui_render::AppHandle;
use serde::de::DeserializeOwned;
use std::rc::Rc;

/// Passed to every [`Plugin`] method once, at startup, when
/// [`crate::PluginRegistry::build`] runs.
pub struct PluginInitContext {
    pub shared: SharedState,
    pub app: AppHandle,
}

impl PluginInitContext {
    /// Loads this plugin's own `modules/<id>.toml` config, falling back to
    /// `T::default()` if the file is missing or fails to parse. Thin
    /// wrapper over [`coconut_core::modules::load_module`] so a plugin
    /// doesn't need its own dependency wiring for it.
    pub fn module_config<T: Default + DeserializeOwned>(&self, id: &str) -> T {
        coconut_core::modules::load_module(id)
    }
}

/// One domain's contribution to the shell: zero or more islands and/or
/// panels. A plugin with only islands (e.g. `logo`) leaves `panels` at its
/// default; a plugin with only panels (e.g. `app_drawer`) leaves `islands`
/// at its default.
pub trait Plugin {
    /// A stable identifier for this plugin, used only for diagnostics (it
    /// does not need to match any island or panel id).
    fn id(&self) -> &'static str;

    fn islands(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Island>> {
        Vec::new()
    }

    fn panels(&self, _ctx: &PluginInitContext) -> Vec<Rc<dyn Panel>> {
        Vec::new()
    }
}
