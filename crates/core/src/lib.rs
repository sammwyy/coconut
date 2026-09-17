mod appearance;
mod desktop;
mod dock;
pub mod geo;
pub mod ipc;
pub mod modules;
mod profile;

pub use appearance::AppearanceConfig;
pub use desktop::{
    ClickAction, DesktopColor, DesktopConfig, DesktopIconsConfig, IconShape, WallpaperMode,
};
pub use dock::{
    ordered_islands, DockAlign, DockConfig, DockDirection, DockPosition, IslandEntry, SectionConfig,
};
pub use profile::UserProfile;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub appearance: AppearanceConfig,
    #[serde(rename = "dock")]
    pub docks: Vec<DockConfig>,
    pub desktop: DesktopConfig,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig::default(),
            docks: vec![DockConfig::default()],
            desktop: DesktopConfig::default(),
        }
    }
}

/// The commented template written to disk the first time Coconut runs,
/// so the file is comfortable to open and edit by hand right away. Keep this
/// in sync with the `Default` impls above when a field's default changes.
const DEFAULT_SHELL_TOML: &str = r#"# Coconut configuration.
# Edit this file before launching the desktop session, or use Settings for
# changes that apply immediately. Missing keys fall back to their defaults,
# so you only need to list what you want to change.
#
# Per-island settings (the clock's time format, tray visibility, etc.) live
# in their own files under the "modules" directory next to this one, e.g.
# modules/clock.toml — this file only describes which docks exist and what
# sits in them.

[appearance]
# Freedesktop application icon and desktop sound themes used by Coconut.
icon_theme = "hicolor"
sound_theme = "freedesktop"

[[dock]]
# Where this dock sits on screen: "top", "bottom", "left" or "right". You
# can declare more than one [[dock]] block to have several docks at once.
position = "bottom"
# When sections do not fill the whole dock, place their group at the
# "start", "center", or "end" of the dock.
align = "start"
# Insets at each end of the dock and spacing between its sections, in pixels.
edge_gap = 16.0
section_gap = 0.0
# Set false to keep a compact section group and use `align` above.
fill_available_space = true
# Optional maximum panel length in pixels; omit for the compositor default.
# max_length = 600.0
# Offset the dock away from its anchored screen edge.
margin = 0.0
# Dock and island chrome can be independently disabled for a transparent,
# borderless presentation.
show_background = true
show_border = true
show_island_background = true
show_island_border = true

[[dock.section]]
# Spacing between the islands in this section: a fixed length ("10px"), a
# percentage of the section's length ("30%"), or "between"/"evenly".
gap = "10px"
[[dock.section.island]]
id = "logo"
[[dock.section.island]]
id = "weather"
[[dock.section.island]]
id = "current_playing"

[[dock.section]]
gap = "10px"
[[dock.section.island]]
id = "app_launcher"

[[dock.section]]
gap = "10px"
[[dock.section.island]]
id = "control_center"
[[dock.section.island]]
id = "clock"

[desktop]
# Path to a wallpaper image. Unset uses the built-in background.
# wallpaper = "/home/you/Pictures/wallpaper.jpg"

[desktop.icons]
# Background square size in pixels. The grid slot, label text and
# spacing all scale with this.
# size = 58.0
# Extra gap added between grid slots, on top of "size".
# spacing = 16.0
# Whether icons show a colored background square.
# background = true
# Background square shape: "square", "rounded", or "circle".
# shape = "rounded"
# Gap between the background and the icon glyph. 0 makes the glyph
# fill the background and clip to its shape.
# padding = 12.0
# background_color = { r = 59, g = 126, b = 193 }
# What a single click does: "select" it, or "open" it right away.
# Double-clicking always opens it either way.
# click = "select"
"#;

impl ShellConfig {
    /// Loads the shell configuration from disk, creating the default file on
    /// first run and falling back to in-memory defaults if the file cannot
    /// be read or contains invalid TOML (the file itself is left untouched
    /// in that case, so no user edits are lost).
    pub fn load() -> Self {
        let path = config_file_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!(
                        "config: failed to parse {}: {error}; using defaults",
                        path.display()
                    );
                    ShellConfig::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_default_config(&path);
                ShellConfig::default()
            }
            Err(error) => {
                eprintln!(
                    "config: failed to read {}: {error}; using defaults",
                    path.display()
                );
                ShellConfig::default()
            }
        }
    }

    /// Writes this configuration to `<config dir>/coconut/shell.toml`,
    /// creating the directory if needed.
    pub fn save(&self) -> std::io::Result<()> {
        let path = config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        std::fs::write(path, contents)
    }
}

fn write_default_config(path: &std::path::Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        eprintln!("config: failed to create {}: {error}", parent.display());
        return;
    }
    if let Err(error) = std::fs::write(path, DEFAULT_SHELL_TOML) {
        eprintln!(
            "config: failed to write default config to {}: {error}",
            path.display()
        );
    }
}

/// Resolves `<config dir>/coconut/shell.toml`: `$XDG_CONFIG_HOME` or
/// `~/.config` on Linux/macOS, `%APPDATA%` on Windows. Namespaced to Coconut
/// itself (not to any compositor/distro bundling it) so this file lands in
/// the same place whether Coconut runs standalone or as part of one.
fn config_file_path() -> PathBuf {
    config_dir().join("coconut").join("shell.toml")
}

#[cfg(target_os = "windows")]
fn config_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(not(target_os = "windows"))]
fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_template_matches_in_memory_defaults() {
        let parsed: ShellConfig = toml::from_str(DEFAULT_SHELL_TOML).expect("valid TOML");
        assert_eq!(parsed, ShellConfig::default());
    }

    #[test]
    fn missing_keys_fall_back_to_defaults() {
        let parsed: ShellConfig = toml::from_str("[[dock]]\nposition = \"top\"\n").unwrap();
        assert_eq!(parsed.docks.len(), 1);
        assert_eq!(parsed.docks[0].position, DockPosition::Top);
        assert_eq!(parsed.docks[0].sections, DockConfig::default().sections);
    }

    #[test]
    fn invalid_toml_is_rejected_rather_than_silently_accepted() {
        let result: Result<ShellConfig, _> = toml::from_str("dock = \"not an array\"");
        assert!(result.is_err());
    }
}
