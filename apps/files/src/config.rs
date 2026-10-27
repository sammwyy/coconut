//! Persistent preferences for File Explorer.

use delib::xdg::XdgPaths;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::PathBuf};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    pub explorer: ExplorerConfig,
    pub shortcuts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ExplorerConfig {
    /// Include dot-prefixed entries when listing a directory.
    pub show_hidden: bool,
    /// Open directory tabs using the grid presentation.
    pub default_grid_view: bool,
    /// Put the tab strip above or below the folder contents.
    pub tab_position: TabPosition,
    /// Display the current selection and item count in the status area.
    pub show_statusbar: bool,
    /// Columns shown in the details presentation.
    pub columns: DetailColumns,
    /// Scale used for file and folder tiles/rows.
    pub item_size: ItemSize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct DetailColumns {
    pub name: bool,
    pub size: bool,
    pub kind: bool,
    pub modified: bool,
    pub created: bool,
    pub permissions: bool,
}

impl Default for DetailColumns {
    fn default() -> Self {
        Self {
            name: true,
            size: true,
            kind: true,
            modified: true,
            created: false,
            permissions: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemSize {
    Compact,
    #[default]
    Default,
    Large,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TabPosition {
    #[default]
    Top,
    Bottom,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            explorer: ExplorerConfig::default(),
            shortcuts: [("search".into(), "Ctrl+Shift+F".into())]
                .into_iter()
                .collect(),
        }
    }
}

impl Default for ExplorerConfig {
    fn default() -> Self {
        Self {
            show_hidden: false,
            default_grid_view: false,
            tab_position: TabPosition::Top,
            show_statusbar: true,
            columns: DetailColumns::default(),
            item_size: ItemSize::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        XdgPaths::default().user_config_file("coconut", "cfiles.toml")
    }

    /// Reads the user's preferences. A malformed or unreadable file never
    /// prevents the file manager from starting; settings can repair it later.
    pub fn load() -> Self {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("cfiles: failed to parse {}: {error}", path.display());
                    Self::default()
                }
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                eprintln!("cfiles: failed to read {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self) -> io::Result<()> {
        let path = Self::path();
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("configuration path has no parent"))?;
        fs::create_dir_all(parent)?;
        let contents =
            toml::to_string_pretty(self).map_err(|error| io::Error::other(error.to_string()))?;
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&temporary, contents)?;
        fs::rename(temporary, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preferences_are_stable() {
        let config = Config::default();
        assert!(!config.explorer.show_hidden);
        assert!(!config.explorer.default_grid_view);
        assert_eq!(config.explorer.tab_position, TabPosition::Top);
        assert!(config.explorer.show_statusbar);
        assert!(config.explorer.columns.name);
        assert!(config.explorer.columns.modified);
        assert_eq!(config.explorer.item_size, ItemSize::Default);
        assert_eq!(config.shortcuts.get("search"), Some(&"Ctrl+Shift+F".into()));
        assert!(toml::to_string(&config).unwrap().contains("[explorer]"));
    }

    #[test]
    fn layout_preferences_round_trip() {
        let config: Config =
            toml::from_str("[explorer]\ntab_position = \"bottom\"\nshow_statusbar = false\n")
                .unwrap();
        assert_eq!(config.explorer.tab_position, TabPosition::Bottom);
        assert!(!config.explorer.show_statusbar);
    }
}
