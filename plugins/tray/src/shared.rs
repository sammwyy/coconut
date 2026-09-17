use creamui_reactive::Signal;
use creamui_widgets::ScrollController;
use std::rc::Rc;

/// The `SharedState` entries every tray island/panel in this crate expects
/// `apps/shell/src/lib.rs` (Phase 4d) to `.insert(...)` at startup, replacing
/// the individually-cloned `Rc<dyn ...Integration>` bindings and
/// hand-created `Signal`s that used to live as separate locals in that file
/// (one per integration, one per "fresh state on open").
///
/// The 6 integration handles themselves
/// (`Rc<dyn coconut_api::network::NetworkIntegration>`,
/// `Rc<dyn coconut_api::bluetooth::BluetoothIntegration>`,
/// `Rc<dyn coconut_api::battery::BatteryIntegration>`,
/// `Rc<dyn coconut_api::volume::VolumeIntegration>`,
/// `Rc<dyn coconut_api::brightness::BrightnessIntegration>`,
/// `Rc<dyn coconut_api::power_profile::PowerProfileIntegration>`) are stored
/// bare — each trait is already its own distinct type, so there is no
/// `TypeId` collision risk storing them directly, unlike the newtypes below.
///
/// Every other piece of state here gets its own newtype instead of a bare
/// `Signal<bool>`/`Signal<f32>`/`Signal<()>`/`Rc<dyn Fn()>`/
/// `ScrollController`: [`coconut_plugin_kit::SharedState`] is keyed by
/// `TypeId` alone, so e.g. `WifiEnabled` and `BluetoothPowered` (both morally
/// a `Signal<bool>`) would otherwise silently clobber one another if stored
/// bare — the same reasoning `coconut-plugin-clock`'s `ClockText` and
/// `coconut-plugin-current-playing`'s `shared.rs` newtypes document.
///
/// If any of these is absent, every island/panel in this crate falls back to
/// a value from `coconut_api::*::Fallback` (for the integration handles) or
/// an inert default (for the signals/callbacks) rather than panicking —
/// consistent with how `coconut-plugin-current-playing`'s island/panel
/// degrade to a "nothing playing" state when its own shared callbacks are
/// missing.
///
/// # Long-lived state (insert once at startup)
/// - [`WifiEnabled`] — mirrors `NetworkIntegration::enabled()`; toggled by
///   both the control center's Wi-Fi tile and the network panel's own
///   switch, so it must be the single shared instance both read and write.
/// - [`BluetoothPowered`] — the Bluetooth analog of `WifiEnabled`.
/// - [`BrightnessLevel`], [`VolumeLevel`] — written directly by a background
///   thread relaying each integration's native change hook (or a poll
///   fallback), mirroring `apps/shell/src/lib.rs`'s old
///   `schedule_brightness_events`/`schedule_volume_events`. Reading these
///   inside a panel/island's `build` is already enough for live reactivity;
///   no separate revision signal is used for either.
/// - [`KeepAwakeState`], [`ToggleKeepAwake`] — the `systemd-inhibit`
///   keep-awake toggle shown on both the control center and the energy
///   panel. The tray plugin only reads/toggles this state; the shell itself
///   still owns spawning/killing the actual child process (mirrors the old
///   `apps/shell/src/lib.rs::keep_awake` closure and `awake` signal).
/// - [`NetworkRevision`], [`BluetoothRevision`], [`BatteryRevision`],
///   [`PowerProfileRevision`] — pulsed by a background thread whenever the
///   corresponding integration's native change hook (or poll fallback)
///   fires; read purely for their reactive subscription (the `()` payload
///   carries no data) so a panel/island re-reads the integration's plain
///   (non-reactive) getter methods live. Mirrors
///   `apps/shell/src/lib.rs::schedule_change_events` and its four
///   `*_revision` signals.
///
/// # Per-open scratch state (do **not** pre-populate; each panel creates its
/// own lazily via `SharedState::get_or_insert_with` on `ctx.scratch`)
/// - [`NetworkScroll`], [`NetworkDetailView`], [`NetworkPasswordReveal`] —
///   used by the network panel's content, whether opened standalone or
///   embedded inside the control center.
/// - [`BluetoothScroll`], [`BluetoothDetailView`] — the Bluetooth analogs.
#[derive(Clone)]
pub struct WifiEnabled(pub Signal<bool>);

#[derive(Clone)]
pub struct BluetoothPowered(pub Signal<bool>);

#[derive(Clone)]
pub struct BrightnessLevel(pub Signal<f32>);

#[derive(Clone)]
pub struct VolumeLevel(pub Signal<f32>);

#[derive(Clone)]
pub struct KeepAwakeState(pub Signal<bool>);

#[derive(Clone)]
pub struct ToggleKeepAwake(pub Rc<dyn Fn()>);

#[derive(Clone)]
pub struct NetworkRevision(pub Signal<()>);

#[derive(Clone)]
pub struct BluetoothRevision(pub Signal<()>);

#[derive(Clone)]
pub struct BatteryRevision(pub Signal<()>);

#[derive(Clone)]
pub struct PowerProfileRevision(pub Signal<()>);

/// Per-open scroll position for the network panel's Wi-Fi/connections list.
/// A newtype rather than a bare `ScrollController` so it can't collide with
/// [`BluetoothScroll`] when both live in the control center's own `scratch`
/// bag (only one is visible at a time, but each keeps its own position
/// across `PanelView` navigation for the lifetime of one popup open).
#[derive(Clone)]
pub struct NetworkScroll(pub ScrollController);

/// Per-open scroll position for the bluetooth panel's device list. See
/// [`NetworkScroll`].
#[derive(Clone)]
pub struct BluetoothScroll(pub ScrollController);

/// Per-open "which interface's detail view is showing" state for the
/// network panel — `None` shows the list. A newtype so it never collides
/// with [`BluetoothDetailView`] or [`NetworkPasswordReveal`] (all three are
/// otherwise a bare `Signal<Option<String>>`).
#[derive(Clone)]
pub struct NetworkDetailView(pub Signal<Option<String>>);

/// Per-open "which device's detail view is showing" state for the bluetooth
/// panel. See [`NetworkDetailView`].
#[derive(Clone)]
pub struct BluetoothDetailView(pub Signal<Option<String>>);

/// Per-open "revealed Wi-Fi password" state for the network panel's detail
/// view — `None` shows the masked placeholder. See [`NetworkDetailView`].
#[derive(Clone)]
pub struct NetworkPasswordReveal(pub Signal<Option<String>>);
