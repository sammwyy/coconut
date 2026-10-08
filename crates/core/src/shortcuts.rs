use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShortcutAction {
    OpenLauncher,
    MinimizeFocused,
    NextWindow,
    PreviousWindow,
    CloseFocused,
    VolumeUp,
    VolumeDown,
    BrightnessUp,
    BrightnessDown,
}

impl ShortcutAction {
    pub const ALL: [Self; 9] = [
        Self::OpenLauncher,
        Self::MinimizeFocused,
        Self::NextWindow,
        Self::PreviousWindow,
        Self::CloseFocused,
        Self::VolumeUp,
        Self::VolumeDown,
        Self::BrightnessUp,
        Self::BrightnessDown,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::OpenLauncher => "Open app launcher",
            Self::MinimizeFocused => "Minimize focused window",
            Self::NextWindow => "Next window",
            Self::PreviousWindow => "Previous window",
            Self::CloseFocused => "Close focused window",
            Self::VolumeUp => "Increase volume",
            Self::VolumeDown => "Decrease volume",
            Self::BrightnessUp => "Increase brightness",
            Self::BrightnessDown => "Decrease brightness",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShellShortcut {
    pub accelerator: String,
    pub action: ShortcutAction,
}

/// A user-defined shortcut launches one program with an optional single
/// argument. Keeping it separate from `ShellShortcut` makes the desktop's
/// built-in actions impossible to delete from the settings UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomShortcut {
    pub accelerator: String,
    pub command: String,
    #[serde(default)]
    pub argument: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShortcutConfig {
    pub shortcuts: Vec<ShellShortcut>,
    pub custom_shortcuts: Vec<CustomShortcut>,
}

impl Default for ShortcutConfig {
    fn default() -> Self {
        Self {
            shortcuts: ShellShortcut::defaults(),
            custom_shortcuts: Vec::new(),
        }
    }
}

impl ShortcutConfig {
    pub fn load() -> Self {
        let path = path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_else(|error| {
                eprintln!("shortcuts: failed to parse {}: {error}", path.display());
                Self::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Self::default();
                let _ = config.save();
                config
            }
            Err(error) => {
                eprintln!("shortcuts: failed to read {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let target = path();
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            target,
            toml::to_string_pretty(self).map_err(std::io::Error::other)?,
        )
    }
}

fn path() -> std::path::PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("coconut")
        .join("shortcuts.toml")
}

impl ShellShortcut {
    pub fn defaults() -> Vec<Self> {
        use ShortcutAction::*;
        [
            ("Super", OpenLauncher),
            ("Super+D", MinimizeFocused),
            ("Alt+Tab", NextWindow),
            ("Alt+Shift+Tab", PreviousWindow),
            ("Alt+F4", CloseFocused),
            ("XF86AudioRaiseVolume", VolumeUp),
            ("XF86AudioLowerVolume", VolumeDown),
            ("XF86MonBrightnessUp", BrightnessUp),
            ("XF86MonBrightnessDown", BrightnessDown),
        ]
        .into_iter()
        .map(|(accelerator, action)| Self {
            accelerator: accelerator.into(),
            action,
        })
        .collect()
    }
}
