use crate::{Panel, PanelRenderContext, PluginRegistry, SharedState};
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
/// [`PanelHost::dispatcher_for`] and [`PanelHost::open`] need to hand out and
/// recursively re-invoke a handle to themselves from popup callbacks that
/// outlive the call to `open`.
pub struct PanelHost {
    app: AppHandle,
    registry: Rc<PluginRegistry>,
    shared: SharedState,
    /// Per-dock anchor geometry, keyed by dock index (see `append_docks`),
    /// so each dock's popups anchor against that dock, not always the first.
    docks: RefCell<HashMap<usize, DockAnchor>>,
    /// Applied to every popup's `WindowOptions`, kept current via
    /// [`PanelHost::set_theme`] the same way `apps/shell`'s
    /// `RuntimeEvent::ReloadTheme` handler pushes a new theme to every
    /// live window today.
    theme: Cell<Theme>,
    open_panels: RefCell<HashMap<&'static str, WindowHandle>>,
}

/// A dock's popup-anchor window plus the position/thickness [`popup_for`]
/// needs for gravity and edge-spanning math.
#[derive(Clone)]
struct DockAnchor {
    window: WindowHandle,
    position: DockPosition,
    thickness: f32,
}

impl PanelHost {
    pub fn new(app: AppHandle, registry: Rc<PluginRegistry>, shared: SharedState) -> Rc<Self> {
        Rc::new(Self {
            app,
            registry,
            shared,
            docks: RefCell::new(HashMap::new()),
            theme: Cell::new(Theme::default()),
            open_panels: RefCell::new(HashMap::new()),
        })
    }

    /// Registers/refreshes `dock_index`'s anchor window and geometry. Call
    /// whenever that dock's window is (re)created.
    pub fn set_dock(&self, dock_index: usize, window: WindowHandle, position: DockPosition, thickness: f32) {
        self.docks.borrow_mut().insert(
            dock_index,
            DockAnchor {
                window,
                position,
                thickness,
            },
        );
    }

    /// Drops every registered dock anchor, before a dock list rebuild.
    pub fn clear_docks(&self) {
        self.docks.borrow_mut().clear();
    }

    /// Updates the theme applied to newly opened popups.
    pub fn set_theme(&self, theme: Theme) {
        self.theme.set(theme);
    }

    /// Returns the `open_panel(id, at)` closure bound to `dock_index`.
    pub fn dispatcher_for(self: &Rc<Self>, dock_index: usize) -> Rc<dyn Fn(&'static str, Point)> {
        let host = Rc::clone(self);
        Rc::new(move |id, at| host.open(dock_index, id, at))
    }

    /// Opens the panel `id`, anchored at `at` against `dock_index`'s dock.
    /// Toggles closed if already open.
    pub fn open(self: &Rc<Self>, dock_index: usize, id: &'static str, at: Point) {
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
        // A new xdg_popup must be the topmost popup; give a just-closed
        // one time to actually reach the compositor before opening another.
        let others: Vec<WindowHandle> = self
            .open_panels
            .borrow_mut()
            .drain()
            .map(|(_, window)| window)
            .filter(|window| window.is_open())
            .collect();
        if others.is_empty() {
            self.do_open(dock_index, id, at, panel);
            return;
        }
        for window in others {
            window.close();
        }
        let host = Rc::clone(self);
        self.app.spawn_background(
            || std::thread::sleep(std::time::Duration::from_millis(100)),
            move |()| host.do_open(dock_index, id, at, panel),
        );
    }

    /// Builds and appends the popup for `panel`, once any previously open
    /// panel is confirmed closed. See [`Self::open`].
    fn do_open(self: &Rc<Self>, dock_index: usize, id: &'static str, at: Point, panel: Rc<dyn Panel>) {
        let Some(anchor) = self.docks.borrow().get(&dock_index).cloned() else {
            eprintln!(
                "plugin-kit: no dock registered at index {dock_index}; ignoring open request"
            );
            return;
        };
        let popup = popup_for(&anchor.window, at, anchor.position, anchor.thickness);
        let size = panel.size();
        let host_for_ready = Rc::clone(self);
        let host_for_close = Rc::clone(self);
        let scratch = SharedState::default();
        let shared = self.shared.clone();
        let dispatcher = self.dispatcher_for(dock_index);
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
/// at `anchor` against `dock` (that dock's own window) at `position`,
/// `dock_thickness` logical pixels thick. Adapted from
/// `apps/shell/src/lib.rs::popup_for` (`BarPosition` -> `DockPosition`; the
/// dock's physical thickness — a hardcoded `DOCK_HEIGHT`/`DOCK_WIDTH`
/// constant owned by the bar module in the original — is now an explicit
/// parameter via [`PanelHost::set_dock`], since `PanelHost` itself has no
/// dependency on the shell engine that defines those constants).
fn popup_for(
    dock: &WindowHandle,
    anchor: Point,
    position: DockPosition,
    dock_thickness: f32,
) -> PopupOptions {
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
