use crate::DesktopColor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How an image is placed on an output.
///
/// `Full` preserves the image aspect ratio and fills the output, cropping the
/// excess when necessary. `Centered` keeps its native size, `Stretched` fills
/// both axes, and `Mosaic` repeats the image without scaling it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BackgroundMode {
    #[default]
    Full,
    Centered,
    Stretched,
    #[serde(alias = "tiled")]
    Mosaic,
}

/// Chooses what Coconut Background draws on each output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum WallpaperSource {
    #[default]
    Image,
    SolidColor,
    Gradient,
    Shader,
}

/// A two-color linear gradient used when [`WallpaperSource::Gradient`] is
/// selected. `angle_degrees` is the direction the gradient travels: `0` runs
/// left to right and larger values rotate it clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GradientConfig {
    pub from: DesktopColor,
    pub to: DesktopColor,
    pub angle_degrees: f32,
}

impl Default for GradientConfig {
    fn default() -> Self {
        Self {
            from: DesktopColor {
                r: 26,
                g: 32,
                b: 48,
            },
            to: DesktopColor {
                r: 74,
                g: 56,
                b: 118,
            },
            angle_degrees: 135.0,
        }
    }
}

/// Built-in animated shaders shipped with `coconut-bg`, selectable through
/// [`BackgroundConfig::shader`]. [`Self::ALL`] lists every variant for
/// selectors in Settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum BuiltinShader {
    #[default]
    Swirl,
    Waves,
}

/// How many palette slots a shader uniform can carry. Shaders that use fewer
/// slots simply ignore the rest.
pub const MAX_SHADER_COLORS: usize = 4;

impl BuiltinShader {
    pub const ALL: &'static [Self] = &[Self::Swirl, Self::Waves];

    pub fn name(self) -> &'static str {
        match self {
            Self::Swirl => "Swirl",
            Self::Waves => "Waves",
        }
    }

    /// The palette this shader consumes: a Settings label and the default
    /// color for each slot, in shader order.
    pub fn palette(self) -> &'static [(&'static str, DesktopColor)] {
        match self {
            Self::Swirl => &[
                ("Base", DesktopColor { r: 8, g: 10, b: 26 }),
                (
                    "Accent A",
                    DesktopColor {
                        r: 218,
                        g: 84,
                        b: 138,
                    },
                ),
                (
                    "Accent B",
                    DesktopColor {
                        r: 60,
                        g: 170,
                        b: 200,
                    },
                ),
            ],
            Self::Waves => &[
                ("Deep water", DesktopColor { r: 3, g: 41, b: 97 }),
                (
                    "Lagoon",
                    DesktopColor {
                        r: 13,
                        g: 184,
                        b: 184,
                    },
                ),
                (
                    "Shallows",
                    DesktopColor {
                        r: 140,
                        g: 235,
                        b: 224,
                    },
                ),
            ],
        }
    }
}

/// Per-shader tuning stored under `[shaders.<name>]` in `bg.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShaderConfig {
    /// Animation speed multiplier: `1.0` is the shader's tuned pace.
    pub speed: f32,
    /// Target frame rate for presentation, clamped to `1..=60`. This gates how
    /// often a frame is presented, not the shader's own animation speed.
    pub fps: u32,
    /// Palette overrides aligned with [`BuiltinShader::palette`]. Missing or
    /// extra entries fall back to that shader's defaults.
    pub colors: Vec<DesktopColor>,
}

impl ShaderConfig {
    /// The color a shader slot renders with: the stored override when present,
    /// otherwise the palette default.
    pub fn color(&self, shader: BuiltinShader, slot: usize) -> DesktopColor {
        self.colors
            .get(slot)
            .copied()
            .or_else(|| shader.palette().get(slot).map(|(_, default)| *default))
            .unwrap_or_default()
    }
}

impl Default for ShaderConfig {
    fn default() -> Self {
        Self {
            speed: 1.0,
            fps: 60,
            colors: Vec::new(),
        }
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
    /// Gradient used when [`Self::source`] is [`WallpaperSource::Gradient`].
    pub gradient: GradientConfig,
    /// Shader used when [`Self::source`] is [`WallpaperSource::Shader`].
    pub shader: BuiltinShader,
    /// Per-shader tuning, keyed by the shader's TOML name.
    pub shaders: BTreeMap<BuiltinShader, ShaderConfig>,
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
            gradient: GradientConfig::default(),
            shader: BuiltinShader::default(),
            shaders: BTreeMap::new(),
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

# "image", "solid_color", "gradient" or "shader".
source = "image"
# "full" (crop to fill), "centered", "stretched", or "mosaic".
mode = "full"
# Used when source = "solid_color", and as the fallback behind transparent
# images or an image that cannot be decoded.
solid_color = { r = 29, g = 37, b = 48 }
# Used when source = "gradient". angle_degrees is the direction the gradient
# travels: 0 runs left to right, and larger values rotate clockwise.
# gradient = { from = { r = 26, g = 32, b = 48 }, to = { r = 74, g = 56, b = 118 }, angle_degrees = 135.0 }
# Used when source = "shader". Available shaders: "swirl", "waves".
# shader = "swirl"

# Per-shader tuning lives under [shaders.<name>]: speed is an animation
# multiplier (1.0 is the tuned pace), fps caps presentation at 1-60 frames
# per second, and colors override the built-in palette slots in order.
# [shaders.swirl]
# speed = 1.0
# fps = 60
# colors = [{ r = 8, g = 10, b = 26 }, { r = 218, g = 84, b = 138 }, { r = 60, g = 170, b = 200 }]
"#;

impl BackgroundConfig {
    /// The stored tuning for `shader`, or its defaults when none was saved.
    pub fn shader_config(&self, shader: BuiltinShader) -> ShaderConfig {
        self.shaders.get(&shader).cloned().unwrap_or_default()
    }

    /// The mutable tuning entry for `shader`, inserting defaults on first use.
    pub fn shader_settings(&mut self, shader: BuiltinShader) -> &mut ShaderConfig {
        self.shaders.entry(shader).or_default()
    }

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
        ..BackgroundConfig::default()
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

    #[test]
    fn wallpaper_sources_use_stable_toml_names() {
        for (name, source) in [
            ("image", WallpaperSource::Image),
            ("solid_color", WallpaperSource::SolidColor),
            ("gradient", WallpaperSource::Gradient),
            ("shader", WallpaperSource::Shader),
        ] {
            let config: BackgroundConfig = toml::from_str(&format!("source = \"{name}\"")).unwrap();
            assert_eq!(config.source, source);
        }
    }

    #[test]
    fn builtin_shaders_use_stable_toml_names() {
        for (name, shader) in [
            ("swirl", BuiltinShader::Swirl),
            ("waves", BuiltinShader::Waves),
        ] {
            let config: BackgroundConfig = toml::from_str(&format!("shader = \"{name}\"")).unwrap();
            assert_eq!(config.shader, shader);
            assert!(BuiltinShader::ALL.contains(&shader));
        }
    }

    #[test]
    fn shader_settings_round_trip() {
        let config: BackgroundConfig = toml::from_str(
            "[shaders.waves]\nspeed = 0.5\nfps = 30\ncolors = [{ r = 1, g = 2, b = 3 }]",
        )
        .unwrap();
        let waves = config.shader_config(BuiltinShader::Waves);
        assert_eq!(waves.speed, 0.5);
        assert_eq!(waves.fps, 30);
        assert_eq!(waves.colors.len(), 1);
        assert_eq!(
            waves.color(BuiltinShader::Waves, 0),
            DesktopColor { r: 1, g: 2, b: 3 }
        );
        assert_eq!(
            waves.color(BuiltinShader::Waves, 1),
            BuiltinShader::Waves.palette()[1].1
        );
        assert_eq!(
            config.shader_config(BuiltinShader::Swirl),
            ShaderConfig::default()
        );
        let serialized = toml::to_string(&config).unwrap();
        let reparsed: BackgroundConfig = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed, config);
    }

    #[test]
    fn gradient_settings_round_trip() {
        let config: BackgroundConfig = toml::from_str(
            "gradient = { from = { r = 1, g = 2, b = 3 }, to = { r = 4, g = 5, b = 6 }, angle_degrees = 90.0 }",
        )
        .unwrap();
        assert_eq!(config.gradient.from, DesktopColor { r: 1, g: 2, b: 3 });
        assert_eq!(config.gradient.to, DesktopColor { r: 4, g: 5, b: 6 });
        assert_eq!(config.gradient.angle_degrees, 90.0);
        let roundtrip: BackgroundConfig =
            toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(config, roundtrip);
    }
}
