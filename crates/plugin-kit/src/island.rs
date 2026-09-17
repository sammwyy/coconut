use crate::{IslandConfig, SharedState};
use coconut_core::DockPosition;
use creamui_core::{BoxedWidget, Point};
use std::rc::Rc;

/// Everything an [`Island`] needs to build (or decide whether to build)
/// itself for one dock rebuild.
pub struct IslandRenderContext<'a> {
    pub shared: &'a SharedState,
    /// `modules/<id>.toml` defaults with the instance's inline
    /// `[[dock.section.island]].config` overrides layered on top, already
    /// merged by the engine before this context is built.
    pub config: &'a IslandConfig,
    pub position: DockPosition,
    /// Opens the panel registered under `id` anchored at `at` (in the
    /// island's own window's logical-pixel coordinates). Replaces
    /// `BarActions`'s 16 named `open_*` fields with one dispatcher shared by
    /// every island — see [`crate::PanelHost::dispatcher`].
    pub open_panel: Rc<dyn Fn(&'static str, Point)>,
}

/// A single clickable module placed in a dock section (weather, clock, the
/// tray, ...). `id` must match a `[[dock.section.island]].id` entry in
/// `shell.toml` for this island to ever be rendered.
pub trait Island {
    /// Must match the `id` islands are addressed by in `shell.toml` and by
    /// [`crate::PluginRegistry::island`].
    fn id(&self) -> &'static str;

    fn build(&self, ctx: &IslandRenderContext) -> BoxedWidget;

    /// Whether this island should render at all right now, e.g.
    /// `current_playing` hiding itself with nothing playing. Re-checked on
    /// every dock rebuild; defaults to always visible.
    fn is_visible(&self, _ctx: &IslandRenderContext) -> bool {
        true
    }
}
