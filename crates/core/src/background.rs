use crate::DesktopColor;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// How an image is placed on an output.
///
/// `Full` preserves the image aspect ratio and fills the output, cropping the
/// excess when necessary. `Centered` keeps its native size, `Stretched` fills
/// both axes, and `Mosaic` repeats the image without scaling it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    Full,
    Centered,
    Stretched,
    #[serde(alias = "tiled")]
    Mosaic,
}

impl Default for BackgroundMode {
    fn default() -> Self {
        Self::Full
    }
}

/// Chooses whether Coconut Background draws an image or an opaque color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperSource {
    Image,
    SolidColor,
}

impl Default for WallpaperSource {
    fn default() -> Self {
        Self::Image
    }
}

/// Configuration owned by the standalone `coconut-bg` process.
///
/// This deliberately lives apart from [`crate::ShellConfig`]: the background
/// keeps rendering while the shell is restarted, and the shell no longer owns
/// an opaque desktop surface just to paint a wallpaper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BackgroundConfig {
    /// Image to show when [`Self::source`] is [`WallpaperSource::Image`].
    pub wallpaper: Option<PathBuf>,
    pub source: WallpaperSource,
    pub mode: BackgroundMode,
    pub solid_color: DesktopColor,
    /// A short history used by Coconut Settings' wallpaper picker.
    pub recent_wallpapers: Vec<PathBuf>,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            wallpaper: None,
            source: WallpaperSource::Image,
            mode: BackgroundMode::Full,
            solid_color: DesktopColor::default(),
            recent_wallpapers: Vec::new(),
        }
    }
}

const DEFAULT_BACKGROUND_TOML: &str = r#"# Coconut Background configuration.
# This file is owned by the standalone coconut-bg process. It sits beside
# shell.toml so Settings and manual edits use one Coconut configuration folder.

# Path to an image. PNG, JPEG, WebP and the other formats supported by the
# image decoder are accepted. Unset falls back to the color below.
# wallpaper = "/home/you/Pictures/wallpaper.jpg"

# "image" or "solid_color".
source = "image"
# "full" (crop to fill), "centered", "stretched", or "mosaic".
mode = "full"
# Used when source = "solid_color", and as the fallback behind transparent
# images or an image that cannot be decoded.
solid_color = { r = 29, g = 37, b = 48 }
"#;

impl BackgroundConfig {
    /// Reads `bg.toml`, creating it on first run. A missing `bg.toml` imports
    /// the wallpaper fields from pre-split `shell.toml` once, so existing
    /// desktops retain their selected background after upgrading.
    pub fn load() -> Self {
        let path = background_config_file_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|error| {
                eprintln!(
                    "background: failed to parse {}: {error}; using defaults",
                    path.display()
                );
                Self::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(config) = legacy_background_config() {
                    if let Err(error) = config.save() {
                        eprintln!(
                            "background: failed to migrate legacy wallpaper to {}: {error}",
                            path.display()
                        );
                    }
                    config
                } else {
                    write_default_background_config(&path);
                    Self::default()
                }
            }
            Err(error) => {
                eprintln!(
                    "background: failed to read {}: {error}; using defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Writes this configuration to `<config dir>/coconut/bg.toml`.
    pub fn save(&self) -> std::io::Result<()> {
        let path = background_config_file_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        std::fs::write(path, contents)
    }

    pub fn config_path() -> PathBuf {
        background_config_file_path()
    }
}

fn background_config_file_path() -> PathBuf {
    super::config_dir().join("coconut").join("bg.toml")
}

fn write_default_background_config(path: &std::path::Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        eprintln!(
            "background: failed to create configuration directory {}: {error}",
            parent.display()
        );
        return;
    }
    if let Err(error) = std::fs::write(path, DEFAULT_BACKGROUND_TOML) {
        eprintln!(
            "background: failed to write default config to {}: {error}",
            path.display()
        );
    }
}

#[derive(Debug, Deserialize)]
struct LegacyShellConfig {
    #[serde(default)]
    desktop: LegacyDesktopConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyDesktopConfig {
    wallpaper: Option<PathBuf>,
    wallpaper_mode: WallpaperSource,
    solid_color: DesktopColor,
    recent_wallpapers: Vec<PathBuf>,
}

fn legacy_background_config() -> Option<BackgroundConfig> {
    let contents = std::fs::read_to_string(super::config_file_path()).ok()?;
    let legacy: LegacyShellConfig = toml::from_str(&contents).ok()?;
    let desktop = legacy.desktop;
    if desktop.wallpaper.is_none()
        && desktop.source_is_default()
        && desktop.solid_color == DesktopColor::default()
        && desktop.recent_wallpapers.is_empty()
    {
        return None;
    }
    Some(BackgroundConfig {
        wallpaper: desktop.wallpaper,
        source: desktop.wallpaper_mode,
        mode: BackgroundMode::Full,
        solid_color: desktop.solid_color,
        recent_wallpapers: desktop.recent_wallpapers,
    })
}

impl LegacyDesktopConfig {
    fn source_is_default(&self) -> bool {
        self.wallpaper_mode == WallpaperSource::Image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_shipped_template() {
        let parsed: BackgroundConfig = toml::from_str(DEFAULT_BACKGROUND_TOML).unwrap();
        assert_eq!(parsed, BackgroundConfig::default());
    }

    #[test]
    fn background_modes_use_stable_toml_names() {
        let config: BackgroundConfig = toml::from_str("mode = \"mosaic\"").unwrap();
        assert_eq!(config.mode, BackgroundMode::Mosaic);
    }
}
