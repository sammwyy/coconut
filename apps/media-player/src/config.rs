//! Persistent preferences shared by the player and `cplayer --settings`.

use creamui_theme::{Color, Theme};
use serde::{Deserialize, Serialize};
use std::{fs, io, path::PathBuf, time::SystemTime};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub playback: PlaybackConfig,
    pub interface: InterfaceConfig,
    pub appearance: AppearanceConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct PlaybackConfig {
    pub backend: PlaybackBackend,
    /// Volume used when the player starts, from 0.0 to 1.0.
    pub volume: f32,
    /// Playback rate applied when the player starts.
    pub speed: f32,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    /// Start playing as soon as a file finishes loading.
    pub autoplay: bool,
    /// Seconds skipped by ← and →.
    pub seek_step: u32,
    /// Seconds skipped by Shift+← and Shift+→.
    pub seek_step_long: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackBackend {
    #[default]
    Auto,
    Mpv,
    Ffmpeg,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct InterfaceConfig {
    /// Seconds without input before video controls hide.
    pub autohide_delay: u32,
    /// Slide and fade transitions for the playback controls.
    pub animations: bool,
    pub background_playback: bool,
    /// Share the current track as a Discord rich presence.
    pub discord_presence: bool,
    /// Only publish a presence for files with artist metadata; untagged
    /// files otherwise show their filename without the extension.
    pub discord_metadata_only: bool,
    /// Upload embedded cover art so the presence can display it.
    pub discord_covers: bool,
    /// Which text leads the Discord presence headline.
    pub discord_title: DiscordTitle,
    pub controls_width: ControlsWidth,
    pub time_display: TimeDisplay,
    /// Which backend draws the windows. Applies to newly opened windows.
    pub renderer: Renderer,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct AppearanceConfig {
    pub accent: Accent,
    pub audio_background: AudioBackground,
    pub cover_shape: CoverShape,
    pub bar_style: BarStyle,
    pub bar_opacity: u8,
    pub bar_blur: bool,
}

impl AppearanceConfig {
    pub fn bar_background(&self, elevated: Color, surface: Color) -> Color {
        let base = match self.bar_style {
            BarStyle::Floating => elevated,
            BarStyle::Solid => surface,
        };
        Color::rgba(
            base.r,
            base.g,
            base.b,
            ((base.a as u16 * self.bar_opacity.min(100) as u16 + 50) / 100) as u8,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ControlsWidth {
    Compact,
    #[default]
    Default,
    Wide,
}

impl ControlsWidth {
    pub fn max_width(self) -> f32 {
        match self {
            Self::Compact => 560.0,
            Self::Default => 720.0,
            Self::Wide => 960.0,
        }
    }
}

/// What the Discord presence shows in its headline (`details`); the second
/// line (`state`) carries the complementary field.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiscordTitle {
    Artist,
    Title,
    #[default]
    ArtistTitle,
    TitleArtist,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimeDisplay {
    #[default]
    Total,
    Remaining,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Renderer {
    /// Draws with `wgpu`, falling back to CPU when no adapter is usable.
    Gpu,
    /// Software rasterizer; skips GPU init cost and memory.
    #[default]
    Cpu,
}

impl Renderer {
    pub fn backend(self) -> creamui_render::RenderBackend {
        match self {
            Self::Gpu => creamui_render::RenderBackend::Gpu,
            Self::Cpu => creamui_render::RenderBackend::Cpu,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioBackground {
    /// A soft, darkened enlargement of the cover art.
    #[default]
    Blurred,
    /// A gradient tinted with the accent colour.
    Gradient,
    Black,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverShape {
    #[default]
    Rounded,
    Square,
    Circle,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BarStyle {
    #[default]
    Floating,
    Solid,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Accent {
    /// Follows the dominant color of the cover art or video.
    #[default]
    Auto,
    System,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Green,
    Teal,
}

impl Accent {
    pub const ALL: [Self; 9] = [
        Self::Auto,
        Self::System,
        Self::Blue,
        Self::Purple,
        Self::Pink,
        Self::Red,
        Self::Orange,
        Self::Green,
        Self::Teal,
    ];

    pub fn color(self) -> Option<Color> {
        Some(match self {
            Self::Auto | Self::System => return None,
            Self::Blue => Color::rgb(59, 130, 246),
            Self::Purple => Color::rgb(139, 92, 246),
            Self::Pink => Color::rgb(236, 72, 153),
            Self::Red => Color::rgb(239, 68, 68),
            Self::Orange => Color::rgb(249, 115, 22),
            Self::Green => Color::rgb(34, 197, 94),
            Self::Teal => Color::rgb(20, 184, 166),
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::System => "System",
            Self::Blue => "Blue",
            Self::Purple => "Purple",
            Self::Pink => "Pink",
            Self::Red => "Red",
            Self::Orange => "Orange",
            Self::Green => "Green",
            Self::Teal => "Teal",
        }
    }

    /// Replaces the system accent while keeping the rest of the palette.
    /// `media` is the color picked from the current artwork, used by `Auto`.
    pub fn apply(self, theme: Theme, media: Option<Color>) -> Theme {
        match self
            .color()
            .or(if self == Self::Auto { media } else { None })
        {
            Some(accent) => with_accent(theme, accent),
            None => theme,
        }
    }
}

fn with_accent(mut theme: Theme, accent: Color) -> Theme {
    theme.colors.accent = accent;
    theme.colors.selection_background = theme.colors.surface.mix(accent, 0.22);
    theme
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            backend: PlaybackBackend::Auto,
            volume: 1.0,
            speed: 1.0,
            repeat: RepeatMode::Off,
            shuffle: false,
            autoplay: true,
            seek_step: 5,
            seek_step_long: 30,
        }
    }
}

impl Default for InterfaceConfig {
    fn default() -> Self {
        Self {
            autohide_delay: 3,
            animations: true,
            background_playback: false,
            discord_presence: false,
            discord_metadata_only: true,
            discord_covers: true,
            discord_title: DiscordTitle::ArtistTitle,
            controls_width: ControlsWidth::Default,
            time_display: TimeDisplay::Total,
            renderer: Renderer::Cpu,
        }
    }
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            accent: Accent::Auto,
            audio_background: AudioBackground::Blurred,
            cover_shape: CoverShape::Rounded,
            bar_style: BarStyle::Floating,
            bar_opacity: 100,
            bar_blur: false,
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        delib::xdg::XdgPaths::default().user_config_file("coconut", "cplayer.toml")
    }

    /// Last modification of the preferences file, used to pick up changes
    /// written by the settings window while the player is running.
    pub fn modified() -> Option<SystemTime> {
        fs::metadata(Self::path()).and_then(|m| m.modified()).ok()
    }

    /// A malformed or unreadable file never prevents the player from starting.
    pub fn load() -> Self {
        let path = Self::path();
        match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|error| {
                eprintln!("cplayer: failed to parse {}: {error}", path.display());
                Self::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                eprintln!("cplayer: failed to read {}: {error}", path.display());
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
    fn defaults_round_trip_through_toml() {
        let config = Config::default();
        let text = toml::to_string_pretty(&config).unwrap();
        assert!(text.contains("[playback]"));
        assert_eq!(toml::from_str::<Config>(&text).unwrap(), config);
    }

    #[test]
    fn partial_files_keep_remaining_defaults() {
        let config: Config =
            toml::from_str("[appearance]\naccent = \"pink\"\n[playback]\nseek_step = 10\n")
                .unwrap();
        assert_eq!(config.appearance.accent, Accent::Pink);
        assert_eq!(config.playback.seek_step, 10);
        assert_eq!(config.playback.seek_step_long, 30);
        assert!(config.interface.animations);
        assert!(!config.interface.background_playback);
        assert_eq!(config.playback.backend, PlaybackBackend::Auto);
    }

    #[test]
    fn playback_backend_round_trips_and_preserves_old_config() {
        let config: Config = toml::from_str("[playback]\nbackend = \"mpv\"\n").unwrap();
        assert_eq!(config.playback.backend, PlaybackBackend::Mpv);
        let saved = toml::to_string(&config).unwrap();
        assert_eq!(toml::from_str::<Config>(&saved).unwrap(), config);
    }

    #[test]
    fn discord_presence_defaults_off_and_round_trips() {
        let config: Config =
            toml::from_str("[interface]\ndiscord_presence = true\ndiscord_metadata_only = false\n")
                .unwrap();
        assert!(config.interface.discord_presence);
        assert!(!config.interface.discord_metadata_only);
        assert!(!Config::default().interface.discord_presence);
        assert!(Config::default().interface.discord_metadata_only);
        assert_eq!(
            toml::from_str::<Config>(&toml::to_string(&config).unwrap()).unwrap(),
            config
        );
    }

    #[test]
    fn background_playback_round_trips() {
        let config: Config = toml::from_str("[interface]\nbackground_playback = true\n").unwrap();
        assert!(config.interface.background_playback);
        assert_eq!(
            toml::from_str::<Config>(&toml::to_string(&config).unwrap()).unwrap(),
            config
        );
    }

    #[test]
    fn bar_background_preferences_keep_old_defaults_and_round_trip() {
        let old: Config = toml::from_str("[appearance]\nbar_style = \"solid\"\n").unwrap();
        assert_eq!(old.appearance.bar_opacity, 100);
        assert!(!old.appearance.bar_blur);
        let config: Config =
            toml::from_str("[appearance]\nbar_opacity = 35\nbar_blur = true\n").unwrap();
        assert_eq!(config.appearance.bar_opacity, 35);
        assert!(config.appearance.bar_blur);
        assert_eq!(
            config
                .appearance
                .bar_background(Color::rgb(20, 40, 60), Color::rgb(80, 100, 120)),
            Color::rgba(20, 40, 60, 89)
        );
        assert_eq!(
            toml::from_str::<Config>(&toml::to_string(&config).unwrap()).unwrap(),
            config
        );
    }
}
