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
    pub fn validate(&self) -> Result<(), String> {
        if self
            .accelerators()
            .filter(|accelerator| !accelerator.is_empty())
            .count()
            > 256
        {
            return Err("At most 256 active shell shortcuts are supported".into());
        }
        let mut parsed = Vec::new();
        for (index, accelerator) in self.accelerators().enumerate() {
            if accelerator.is_empty() {
                continue;
            }
            let mut binding = blair_protocol::ShortcutAccelerator::parse(accelerator)
                .map_err(|error| format!("Shortcut {}: {error}", index + 1))?;
            binding.keys.sort_unstable();
            if parsed.contains(&binding) {
                return Err(format!(
                    "Shortcut {} duplicates an earlier binding",
                    index + 1
                ));
            }
            parsed.push(binding);
        }
        for (index, shortcut) in self.custom_shortcuts.iter().enumerate() {
            if !shortcut.accelerator.is_empty() && shortcut.command.trim().is_empty() {
                return Err(format!("Custom shortcut {} has no command", index + 1));
            }
        }
        Ok(())
    }

    pub fn accelerators(&self) -> impl Iterator<Item = &str> {
        self.shortcuts
            .iter()
            .map(|binding| binding.accelerator.as_str())
            .chain(
                self.custom_shortcuts
                    .iter()
                    .map(|binding| binding.accelerator.as_str()),
            )
    }

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
        self.validate().map_err(std::io::Error::other)?;
        let temporary = target.with_extension(format!("toml.{}.tmp", std::process::id()));
        std::fs::write(
            &temporary,
            toml::to_string_pretty(self).map_err(std::io::Error::other)?,
        )?;
        std::fs::rename(temporary, target)
    }
}

fn path() -> std::path::PathBuf {
    super::config_dir().join("coconut").join("shortcuts.toml")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_defaults_and_rejects_duplicates_unknown_keys_and_empty_commands() {
        let mut config = ShortcutConfig::default();
        assert!(config.validate().is_ok());
        config.custom_shortcuts.push(CustomShortcut {
            accelerator: "Logo".into(),
            command: "terminal".into(),
            argument: String::new(),
        });
        assert!(config.validate().unwrap_err().contains("duplicates"));
        config.custom_shortcuts[0].accelerator = "Super+F35".into();
        assert!(config.validate().unwrap_err().contains("unsupported"));
        config.custom_shortcuts[0].accelerator = "Ctrl+Alt+T".into();
        config.custom_shortcuts[0].command.clear();
        assert!(config.validate().unwrap_err().contains("no command"));
        config.custom_shortcuts[0].accelerator.clear();
        assert!(config.validate().is_ok());
    }
}
