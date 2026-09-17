use coconut_api::audio::Playback;
use std::rc::Rc;

/// The `SharedState` entries [`crate::CurrentPlayingIsland`] and
/// [`crate::CurrentPlayingPanel`] expect `apps/shell/src/lib.rs` (Phase 4d)
/// to `.insert(...)` at startup, replacing the dedicated
/// `current_playback`/`toggle_playback`/`previous_playback`/`next_playback`
/// fields the old `BarActions` struct carried (plus `seek`, previously
/// threaded directly into `current_playing::build` by the one `open_X`
/// block that opened this panel).
///
/// Each callback is wrapped in its own newtype rather than stored as a bare
/// `Rc<dyn Fn()>`/`Rc<dyn Fn(f64)>`: `coconut_plugin_kit::SharedState` is
/// keyed by `TypeId` alone, so several unrelated `Rc<dyn Fn()>` values
/// (e.g. a future plugin's own toggle callback) would otherwise collide
/// under the same erased type and silently overwrite one another.
///
/// `apps/shell/src/lib.rs` should insert:
/// - [`CurrentPlayback`] — reads the current playback state (the old
///   `BarActions::current_playback`).
/// - [`TogglePlayback`], [`PreviousPlayback`], [`NextPlayback`] — the three
///   transport controls (the old `BarActions::toggle_playback` /
///   `previous_playback` / `next_playback`).
/// - [`SeekPlayback`] — only consumed by the panel's progress slider (the
///   old inline `seek` closure built in the `open_current_playing`
///   `append_popup` block), takes the target position in seconds.
///
/// If [`CurrentPlayback`] is absent, [`crate::CurrentPlayingIsland::is_visible`]
/// returns `false` (nothing to show) and the panel renders its
/// nothing-playing state.
#[derive(Clone)]
pub struct CurrentPlayback(pub Rc<dyn Fn() -> Option<Playback>>);

#[derive(Clone)]
pub struct TogglePlayback(pub Rc<dyn Fn()>);

#[derive(Clone)]
pub struct PreviousPlayback(pub Rc<dyn Fn()>);

#[derive(Clone)]
pub struct NextPlayback(pub Rc<dyn Fn()>);

#[derive(Clone)]
pub struct SeekPlayback(pub Rc<dyn Fn(f64)>);
