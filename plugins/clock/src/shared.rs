use creamui_reactive::Signal;

/// The live, pre-formatted clock text the dock island paints.
///
/// A plain `Signal<String>` is deliberately not stored directly:
/// [`coconut_plugin_kit::SharedState`] is keyed by `TypeId` alone, so two
/// unrelated plugins each storing a bare `Signal<String>` for their own
/// purposes would silently clobber one another. This newtype gives the
/// clock's entry its own type identity.
///
/// **Contract for `apps/shell/src/lib.rs` (Phase 4d):** insert one of these
/// into the app-wide `SharedState` at startup, seeded with
/// `Local::now().format(&format).to_string()` (using the same
/// [`crate::ClockConfig::format`] this plugin was constructed with — obtain
/// it via `ctx.module_config::<ClockConfig>("clock")`), then update the
/// signal roughly once a second (mirroring the old
/// `apps/shell/src/lib.rs::schedule_clock_refresh`). If nothing is ever
/// inserted, [`crate::ClockIsland`] falls back to a one-shot, non-reactive
/// `Local::now()` formatted at build time (correct on first render, but it
/// will not tick).
#[derive(Clone)]
pub struct ClockText(pub Signal<String>);
