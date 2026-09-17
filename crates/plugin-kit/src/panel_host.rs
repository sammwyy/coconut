use crate::{PanelRenderContext, PluginRegistry, SharedState};
use coconut_core::DockPosition;
use creamui_core::{Point, Rect};
use creamui_render::platform::PopupPlacement;
use creamui_render::{AppHandle, PopupOptions, WindowHandle, WindowOptions};
use creamui_theme::{Color, Theme};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

/// Owns every open panel popup and knows how to open/toggle-close one by
/// id. Replaces the ~10x duplicated `append_popup` boilerplate that used to
/// live inline in `apps/shell/src/lib.rs` (one block per panel: a
/// hand-created per-open `Signal`, a toggle-close-if-already-open check, an
/// anchor computation, and an `AppHandle::append_popup` call) with one
/// generic implementation shared by every panel.
///
/// Must be constructed behind an `Rc` (via [`PanelHost::new`]) since
/// [`PanelHost::dispatcher`] and [`PanelHost::open`] need to hand out and
/// recursively re-invoke a handle to themselves from popup callbacks that
/// outlive the call to `open`.
pub struct PanelHost {
    app: AppHandle,
    registry: Rc<PluginRegistry>,
    shared: SharedState,
    /// The dock window panels are anchored against. A `Cell`/`RefCell` pair
    /// (rather than requiring it up front in `new`) because the dock window
    /// is itself recreated whenever its `DockPosition` changes (a
    /// layer-shell role can't be changed in place) — set with
    /// [`PanelHost::set_dock_window`] whenever that happens.
    dock_window: Rc<RefCell<Option<WindowHandle>>>,
    /// The current dock's position and physical thickness in logical
    /// pixels (`DOCK_HEIGHT`/`DOCK_WIDTH` in the shell engine), needed by
    /// [`popup_for`]'s anchor computation. Unlike the old `lib.rs`, which
    /// hardcoded `DOCK_HEIGHT`/`DOCK_WIDTH` constants owned by the bar
    /// module, this is a parameter set via [`PanelHost::set_dock_window`]
    /// so a future multi-dock setup isn't locked into one fixed thickness.
    position: Cell<DockPosition>,
    dock_thickness: Cell<f32>,
    /// Applied to every popup's `WindowOptions`, kept current via
    /// [`PanelHost::set_theme`] the same way `apps/shell`'s
    /// `RuntimeEvent::ReloadTheme` handler pushes a new theme to every
    /// live window today.
    theme: Cell<Theme>,
    open_panels: RefCell<HashMap<&'static str, WindowHandle>>,
}

impl PanelHost {
    pub fn new(app: AppHandle, registry: Rc<PluginRegistry>, shared: SharedState) -> Rc<Self> {
        Rc::new(Self {
            app,
            registry,
            shared,
            dock_window: Rc::new(RefCell::new(None)),
            position: Cell::new(DockPosition::default()),
            dock_thickness: Cell::new(44.0),
            theme: Cell::new(Theme::default()),
            open_panels: RefCell::new(HashMap::new()),
        })
    }

    /// Updates the dock window panels anchor against. Call whenever the
    /// dock's own window is (re)created, e.g. after a `DockPosition` change
    /// forces the layer-shell surface to be recreated.
    pub fn set_dock_window(&self, window: Option<WindowHandle>) {
        *self.dock_window.borrow_mut() = window;
    }

    /// Updates the dock geometry used to compute popup anchors/placement.
    pub fn set_dock_geometry(&self, position: DockPosition, dock_thickness: f32) {
        self.position.set(position);
        self.dock_thickness.set(dock_thickness);
    }

    /// Updates the theme applied to newly opened popups.
    pub fn set_theme(&self, theme: Theme) {
        self.theme.set(theme);
    }

    /// Returns the generic `open_panel(id, at)` closure every
    /// [`crate::IslandRenderContext`]/[`PanelRenderContext`] is handed,
    /// replacing `BarActions`'s 16 named `open_*: Rc<dyn Fn(Point)>` fields
    /// with one dispatcher shared by every island and panel.
    pub fn dispatcher(self: &Rc<Self>) -> Rc<dyn Fn(&'static str, Point)> {
        let host = Rc::clone(self);
        Rc::new(move |id, at| host.open(id, at))
    }

    /// Opens the panel registered under `id`, anchored at `at`. Looks the
    /// panel up in the registry (warns and no-ops if `id` is unknown),
    /// toggles it closed if it's already open, then computes the popup's
    /// placement and appends it via the real
    /// [`creamui_render::AppHandle::append_popup`] — the same call every
    /// `open_X` block in the old `apps/shell/src/lib.rs` made by hand.
    pub fn open(self: &Rc<Self>, id: &'static str, at: Point) {
        let Some(panel) = self.registry.panel(id).cloned() else {
            eprintln!("plugin-kit: unknown panel id '{id}'; ignoring open request");
            return;
        };
        if let Some(handle) = self.open_panels.borrow_mut().remove(id) {
            if handle.is_open() {
                handle.close();
                return;
            }
        }
        let Some(popup) = popup_for(
            &self.dock_window,
            at,
            self.position.get(),
            self.dock_thickness.get(),
        ) else {
            return;
        };
        let size = panel.size();
        let host_for_ready = Rc::clone(self);
        let host_for_close = Rc::clone(self);
        let scratch = SharedState::default();
        let shared = self.shared.clone();
        let dispatcher = self.dispatcher();
        self.app.append_popup(
            popup_options(
                panel.title(),
                size.width as u32,
                size.height as u32,
                self.theme.get(),
            ),
            popup,
            Color::rgba(0, 0, 0, 0),
            move |window| {
                window.set_always_on_top(true);
                close_on_focus_lost(&window);
                host_for_ready.open_panels.borrow_mut().insert(id, window);
            },
            move |resolved_size| {
                let close_host = Rc::clone(&host_for_close);
                let close: Rc<dyn Fn()> = Rc::new(move || {
                    if let Some(window) = close_host.open_panels.borrow_mut().remove(id) {
                        window.close();
                    }
                });
                panel.build(&PanelRenderContext {
                    size: resolved_size,
                    shared: shared.clone(),
                    scratch: scratch.clone(),
                    open_panel: dispatcher.clone(),
                    close,
                })
            },
        );
    }
}

/// Registers a callback that closes `window` as soon as it loses focus —
/// the standard dismiss-on-click-outside behavior for every popup. Moved
/// verbatim from `apps/shell/src/lib.rs::close_on_focus_lost`.
fn close_on_focus_lost(window: &WindowHandle) {
    let handle = window.clone();
    window.on_focus_lost(move || handle.close());
}

/// Computes the anchored, gravity-aware [`PopupOptions`] for a panel opened
/// at `anchor` against a dock at `position`, `dock_thickness` logical
/// pixels thick. Adapted from `apps/shell/src/lib.rs::popup_for`
/// (`BarPosition` -> `DockPosition`; the dock's physical thickness — a
/// hardcoded `DOCK_HEIGHT`/`DOCK_WIDTH` constant owned by the bar module in
/// the original — is now an explicit parameter via
/// [`PanelHost::set_dock_geometry`], since `PanelHost` itself has no
/// dependency on the shell engine that defines those constants).
fn popup_for(
    dock_window: &Rc<RefCell<Option<WindowHandle>>>,
    anchor: Point,
    position: DockPosition,
    dock_thickness: f32,
) -> Option<PopupOptions> {
    let dock_ref = dock_window.borrow();
    dock_ref.as_ref().map(|dock| {
        let anchor_rect = match position {
            // Horizontal panels keep the old edge-spanning anchor, which
            // places their popup outside the dock.
            DockPosition::Top => Rect {
                x: anchor.x,
                y: anchor.y,
                width: 1.0,
                height: (dock_thickness - anchor.y).max(1.0),
            },
            DockPosition::Bottom => Rect {
                x: anchor.x,
                y: 0.0,
                width: 1.0,
                height: (anchor.y + 1.0).max(1.0),
            },
            // For a side dock, Top/Center/Bottom gravity must be relative
            // to the button itself, never the whole section above it.
            DockPosition::Left | DockPosition::Right => Rect {
                x: anchor.x,
                y: anchor.y,
                width: 1.0,
                height: 1.0,
            },
        };
        let vertical = dock
            .monitor_size()
            .map(|(_, height)| height as f32)
            .unwrap_or(800.0);
        // The center section of a side dock is deliberately generous. It
        // covers the middle half of the dock; only controls clearly near an
        // edge choose an edge-aligned popup.
        let top_region = vertical * 0.25;
        let bottom_region = vertical * 0.75;
        let placement = match position {
            DockPosition::Top => PopupPlacement::Below,
            DockPosition::Bottom => PopupPlacement::Above,
            DockPosition::Left if anchor.y < top_region => PopupPlacement::RightTop,
            DockPosition::Left if anchor.y > bottom_region => PopupPlacement::RightBottom,
            DockPosition::Left => PopupPlacement::RightCenter,
            DockPosition::Right if anchor.y < top_region => PopupPlacement::LeftTop,
            DockPosition::Right if anchor.y > bottom_region => PopupPlacement::LeftBottom,
            DockPosition::Right => PopupPlacement::LeftCenter,
        };
        PopupOptions::new(dock.clone(), anchor_rect).placed(placement)
    })
}

/// Builds the `WindowOptions` shared by every panel popup. Moved verbatim
/// (parameterized on `theme` instead of reading `apps/shell`'s own
/// process-global `system_theme()`, since `PanelHost` has no such static)
/// from `apps/shell/src/lib.rs::popup_options`.
fn popup_options(title: &str, width: u32, height: u32, theme: Theme) -> WindowOptions {
    WindowOptions {
        title: title.into(),
        width,
        height,
        decorations: false,
        resizable: false,
        transparent: true,
        theme,
        ..Default::default()
    }
}
