use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperMode {
    Image,
    SolidColor,
}

impl Default for WallpaperMode {
    fn default() -> Self {
        Self::Image
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for DesktopColor {
    fn default() -> Self {
        Self {
            r: 29,
            g: 37,
            b: 48,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DesktopConfig {
    pub wallpaper: Option<PathBuf>,
    pub wallpaper_mode: WallpaperMode,
    pub solid_color: DesktopColor,
    pub recent_wallpapers: Vec<PathBuf>,
}
