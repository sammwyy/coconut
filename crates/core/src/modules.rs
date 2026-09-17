use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::PathBuf;

/// Loads a plugin's own typed config from `<config dir>/coconut/modules/<id>.toml`
/// (e.g. the clock's strftime format, or the tray's mode/visibility). Lazily
/// created: unlike `shell.toml` there's no shipped template, since most
/// plugins are never touched by hand — mirrors `UserProfile::load`.
pub fn load_module<T: Default + DeserializeOwned>(id: &str) -> T {
    std::fs::read_to_string(module_path(id))
        .ok()
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn save_module<T: Serialize>(id: &str, value: &T) -> std::io::Result<()> {
    let path = module_path(id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents =
        toml::to_string_pretty(value).map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(path, contents)
}

/// Schema-free variant used by Settings' generic per-module editor, which
/// has no compiled-in Rust type for every plugin's config shape.
pub fn load_module_value(id: &str) -> toml::Value {
    std::fs::read_to_string(module_path(id))
        .ok()
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_else(|| toml::Value::Table(toml::value::Table::new()))
}

pub fn save_module_value(id: &str, value: &toml::Value) -> std::io::Result<()> {
    let path = module_path(id);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents =
        toml::to_string_pretty(value).map_err(|error| std::io::Error::other(error.to_string()))?;
    std::fs::write(path, contents)
}

fn module_path(id: &str) -> PathBuf {
    modules_dir().join(format!("{id}.toml"))
}

fn modules_dir() -> PathBuf {
    super::config_dir().join("coconut").join("modules")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct Example {
        format: String,
        enabled: bool,
    }

    #[test]
    fn missing_module_file_falls_back_to_default() {
        let value: Example = load_module("coconut-core-tests-nonexistent-module");
        assert_eq!(value, Example::default());
    }
}
