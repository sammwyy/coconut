mod appearance;
mod desktop;
mod dock;
pub mod geo;
pub mod ipc;
pub mod modules;
mod popups;
mod profile;
mod shortcuts;

pub use appearance::AppearanceConfig;
pub use desktop::{
    ClickAction, DesktopColor, DesktopConfig, DesktopIconsConfig, IconShape, WallpaperMode,
};
pub use dock::{
    ordered_islands, BackgroundSource, DockAlign, DockConfig, DockDirection, DockPosition,
    IslandEntry, SectionConfig, DEFAULT_THICKNESS,
};
pub use popups::{PopupAlign, PopupConfig, PopupEdge};
pub use profile::UserProfile;
pub use shortcuts::{CustomShortcut, ShellShortcut, ShortcutAction, ShortcutConfig};

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Battery levels at which the shell warns while running on battery power.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct BatteryAlertsConfig {
    pub low_percent: u8,
    pub critical_percent: u8,
}

impl Default for BatteryAlertsConfig {
    fn default() -> Self {
        Self {
            low_percent: 10,
            critical_percent: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShellConfig {
    pub appearance: AppearanceConfig,
    pub statusbar: DockConfig,
    pub dockbar: DockConfig,
    pub desktop: DesktopConfig,
    pub status_updates: PopupConfig,
    pub battery_alerts: BatteryAlertsConfig,
    #[serde(
        default = "PopupConfig::notifications",
        deserialize_with = "popups::deserialize_notifications"
    )]
    pub notifications: PopupConfig,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self {
            appearance: AppearanceConfig::default(),
            statusbar: DockConfig {
                position: DockPosition::Top,
                edge_gap: 20.0,
                section_gap: 8.0,
                show_background: false,
                show_border: false,
                island_background_opacity: 0.72,
                sections: vec![
                    SectionConfig {
                        enabled: true,
                        gap: "10px".to_owned(),
                        islands: vec![
                            IslandEntry::with_id("logo"),
                            IslandEntry::with_id("open_windows"),
                        ],
                    },
                    SectionConfig {
                        enabled: true,
                        gap: "10px".to_owned(),
                        islands: vec![
                            IslandEntry::with_id("weather"),
                            IslandEntry::with_id("current_playing"),
                            IslandEntry::with_id("clock"),
                        ],
                    },
                    SectionConfig {
                        enabled: true,
                        gap: "10px".to_owned(),
                        islands: vec![IslandEntry::with_id("control_center")],
                    },
                ],
                ..DockConfig::default()
            },
            dockbar: DockConfig {
                align: DockAlign::Center,
                fill_available_space: false,
                max_length: Some(800.0),
                margin: 20.0,
                thickness: 76.0,
                background_opacity: 1.0,
                ..DockConfig::default()
            },
            desktop: DesktopConfig::default(),
            status_updates: PopupConfig::default(),
            battery_alerts: BatteryAlertsConfig::default(),
            notifications: PopupConfig::notifications(),
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
# modules/clock.toml.

[appearance]
# Freedesktop application icon and desktop sound themes used by Coconut.
icon_theme = "hicolor"
sound_theme = "freedesktop"
# Mouse cursor theme, picked from installed themes with a "cursors" folder.
cursor_theme = "Adwaita"

# Both bars accept the same options. There are always exactly two: statusbar
# and dockbar. Each can sit on any screen edge.
[statusbar]
position = "top"
# When sections do not fill the whole dock, place their group at the
# "start", "center", or "end" of the dock.
align = "start"
# Insets at each end of the dock and spacing between its sections, in pixels.
edge_gap = 20.0
section_gap = 8.0
# Set false to keep a compact section group and use `align` above.
fill_available_space = true
# Optional maximum panel length in pixels; omit for the compositor default.
# max_length = 600.0
# Offset the dock away from its anchored screen edge.
margin = 0.0
# Dock thickness in pixels: height for "top"/"bottom", width for "left"/
# "right". Islands (icons, text, padding) scale to match.
thickness = 44.0
# Dock and island chrome can be independently disabled for a transparent,
# borderless presentation.
show_background = false
show_border = false
show_island_background = true
show_island_border = true
# When true, a section's islands share one background/border instead of
# each drawing its own.
unify_island_background = false
# Background color source for the dock ("theme" or "custom") and, when
# "custom", the color and opacity applied to it. Opacity (0.0-1.0) applies
# either way.
# background_source = "theme"
# background_color = { r = 24, g = 24, b = 24 }
background_opacity = 1.0
# Same, but for each island's own background.
# island_background_source = "theme"
# island_background_color = { r = 24, g = 24, b = 24 }
island_background_opacity = 0.72

[[statusbar.section]]
# Spacing between the islands in this section: a fixed length ("10px"), a
# percentage of the section's length ("30%"), or "between"/"evenly".
gap = "10px"
[[statusbar.section.island]]
id = "logo"
[[statusbar.section.island]]
id = "open_windows"

[[statusbar.section]]
gap = "10px"
[[statusbar.section.island]]
id = "weather"
[[statusbar.section.island]]
id = "current_playing"
[[statusbar.section.island]]
id = "clock"

[[statusbar.section]]
gap = "10px"
[[statusbar.section.island]]
id = "control_center"

[dockbar]
position = "bottom"
align = "center"
edge_gap = 16.0
section_gap = 0.0
fill_available_space = false
max_length = 800.0
margin = 20.0
thickness = 76.0
show_background = true
show_border = true
show_island_background = true
show_island_border = true
unify_island_background = false
background_opacity = 1.0
island_background_opacity = 1.0

[[dockbar.section]]
gap = "10px"
[[dockbar.section.island]]
id = "app_launcher"
[[dockbar.section.island]]
id = "open_windows"

[status_updates]
enabled = true
edge = "bottom"
align = "center"
offset = 120.0
duration_ms = 1800

# The shell warns once when charge falls through either level while unplugged.
[battery_alerts]
low_percent = 10
critical_percent = 5

[notifications]
enabled = true
edge = "top"
align = "right"
offset = 64.0
duration_ms = 5000

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
    pub fn bars(&self) -> [&DockConfig; 2] {
        [&self.statusbar, &self.dockbar]
    }

    pub fn bar_configs(&self) -> [DockConfig; 2] {
        [self.statusbar.clone(), self.dockbar.clone()]
    }

    /// Loads the shell configuration from disk, creating the default file on
    /// first run and falling back to in-memory defaults if the file cannot
    /// be read or contains invalid TOML (the file itself is left untouched
    /// in that case, so no user edits are lost).
    pub fn load() -> Self {
        let path = config_file_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<ShellConfig>(&contents) {
                Ok(config) => {
                    if legacy_dock_config(&contents) {
                        if let Err(error) = config.save() {
                            eprintln!(
                                "config: failed to replace legacy docks in {}: {error}",
                                path.display()
                            );
                        }
                    }
                    config
                }
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

    /// Reset only Coconut's shell configuration, preserving an exact backup.
    /// Called only after the user opens and confirms the reset page.
    pub fn reset_with_backup(&self) -> std::io::Result<PathBuf> {
        use std::io::Write;
        let target = config_file_path();
        let directory = target
            .parent()
            .ok_or_else(|| std::io::Error::other("No configuration directory"))?;
        std::fs::create_dir_all(directory)?;
        if std::fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(std::io::Error::other("Refusing to replace a symbolic link"));
        }
        let previous = match std::fs::read(&target) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                toml::to_string_pretty(self)
                    .map_err(std::io::Error::other)?
                    .into_bytes()
            }
            Err(error) => return Err(error),
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let backup = directory.join(format!("shell-backup-{stamp}.toml"));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&backup)?;
        file.write_all(&previous)?;
        file.sync_all()?;
        let temporary = directory.join(format!("shell-reset-{stamp}.toml"));
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(
            toml::to_string_pretty(&Self::default())
                .map_err(std::io::Error::other)?
                .as_bytes(),
        )?;
        file.sync_all()?;
        std::fs::rename(temporary, target)?;
        Ok(backup)
    }
}

fn legacy_dock_config(contents: &str) -> bool {
    let Ok(toml::Value::Table(table)) = toml::from_str(contents) else {
        return false;
    };
    table.contains_key("dock") && !table.contains_key("statusbar") && !table.contains_key("dockbar")
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
        let parsed: ShellConfig = toml::from_str("[statusbar]\nposition = \"bottom\"\n").unwrap();
        assert_eq!(parsed.statusbar.position, DockPosition::Bottom);
        assert_eq!(parsed.dockbar, ShellConfig::default().dockbar);
    }

    #[test]
    fn partial_popup_configuration_preserves_each_default_position() {
        let parsed: ShellConfig = toml::from_str(
            "[notifications]\nenabled = false\n[status_updates]\nduration_ms = 2500\n",
        )
        .unwrap();
        assert_eq!(
            parsed.notifications,
            PopupConfig {
                enabled: false,
                ..PopupConfig::notifications()
            }
        );
        assert_eq!(
            parsed.status_updates,
            PopupConfig {
                duration_ms: 2500,
                ..PopupConfig::default()
            }
        );
        let roundtrip: ShellConfig = toml::from_str(&toml::to_string(&parsed).unwrap()).unwrap();
        assert_eq!(parsed, roundtrip);
    }

    #[test]
    fn invalid_toml_is_rejected_rather_than_silently_accepted() {
        let result: Result<ShellConfig, _> = toml::from_str("statusbar = \"not a table\"");
        assert!(result.is_err());
    }

    #[test]
    fn detects_a_legacy_dynamic_dock_file() {
        assert!(legacy_dock_config("[[dock]]\nposition = \"bottom\"\n"));
        assert!(!legacy_dock_config("[statusbar]\nposition = \"top\"\n"));
        assert!(!legacy_dock_config("not valid toml"));
    }
}
