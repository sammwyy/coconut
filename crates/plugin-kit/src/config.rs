use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single scalar value in an [`IslandConfig`]. Deliberately closed to
/// these four shapes rather than an open `toml::Value`/`Box<dyn Any>`: this
/// is the exact surface a future Lua bridge plugin needs to marshal to and
/// from Lua tables (see the refactor plan's "Deferred" section) — widening
/// it later would be a breaking change to that seam, so it stays narrow
/// from the start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

/// A resolved per-island configuration: `modules/<id>.toml` defaults
/// layered under the inline `[[dock.section.island]].config` overrides from
/// `shell.toml`, merged before an [`crate::Island`] ever sees it.
pub type IslandConfig = HashMap<String, ConfigValue>;
