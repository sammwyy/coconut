//! `coconut-plugin-kit`: the `Island`/`Panel`/`Plugin` traits, the
//! `PluginRegistry` that indexes them, `SharedState` (the type-keyed bag
//! that replaces `BarActions`'s enumerated closures and the shell's many
//! individually-cloned `Rc` integration bindings), `PanelHost` (the generic
//! popup-panel opener that replaces ~10 duplicated `append_popup` blocks),
//! and the chrome/icon helpers every plugin's UI is built from.
//!
//! Phase 3 of the dock/island/panel refactor
//! (`~/.claude/plans/luminous-prancing-wombat.md`). Nothing in the
//! workspace depends on this crate yet — `plugins/<id>` crates and
//! `apps/shell`'s engine rewrite (Phase 4) are the first consumers.

pub mod chrome;
mod config;
mod gap;
mod icon_theme;
mod icons;
mod island;
mod panel;
mod panel_host;
mod plugin;
mod registry;
mod state;

pub use config::{ConfigValue, IslandConfig};
pub use gap::{parse_gap, Gap};
pub use icon_theme::{build_icon_index, load_icon, resolve_icon, xdg_data_directories};
pub use icons::pixel_icon;
pub use island::{Island, IslandRenderContext};
pub use panel::{Panel, PanelRenderContext};
pub use panel_host::PanelHost;
pub use plugin::{Plugin, PluginInitContext};
pub use registry::{warn_unknown_islands, PluginRegistry};
pub use state::SharedState;
