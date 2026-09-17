use crate::SharedState;
use creamui_core::{BoxedWidget, Point, Size};
use std::rc::Rc;

/// Everything a [`Panel`] needs to build itself for one popup open.
pub struct PanelRenderContext {
    /// The size the platform actually negotiated for the popup (subject to
    /// compositor placement, same as every `build_ui` callback in
    /// `creamui-render`), not necessarily [`Panel::size`] verbatim.
    pub size: Size,
    /// The same bag every island and panel shares, for state that must
    /// survive across opens (integrations, persistent signals).
    pub shared: SharedState,
    /// A fresh, empty [`SharedState`] created for this open only — replaces
    /// the "hand-created `Signal`/`ScrollController` per open" locals that
    /// used to live inline in each `open_X` block in
    /// `apps/shell/src/lib.rs`. Use [`SharedState::get_or_insert_with`] to
    /// lazily seed per-open state (a navigation `Signal`, a
    /// `ScrollController`) the first time this panel's `build` runs for
    /// this open.
    pub scratch: SharedState,
    /// Opens another panel by id, e.g. control center drilling into its
    /// network/bluetooth/energy sub-panels.
    pub open_panel: Rc<dyn Fn(&'static str, Point)>,
    /// Closes this panel's own popup window.
    pub close: Rc<dyn Fn()>,
}

/// A dropdown surface opened by clicking an island (or another panel).
pub trait Panel {
    /// Must match the id islands call `open_panel` with, and the id
    /// [`crate::PluginRegistry::panel`] looks it up by.
    fn id(&self) -> &'static str;

    /// The popup window's title.
    fn title(&self) -> &'static str;

    /// The popup window's requested size, replacing each panel module's own
    /// `WIDTH`/`HEIGHT` consts.
    fn size(&self) -> Size;

    fn build(&self, ctx: &PanelRenderContext) -> BoxedWidget;
}
