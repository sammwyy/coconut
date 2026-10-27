use crate::shortcuts::{Action, Binding};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "FileConfig")]
pub struct Config {
    pub shell: String,
    pub font: String,
    pub profile: String,
    pub profiles: BTreeMap<String, Profile>,
    pub appearance: Appearance,
    pub scrollback: usize,
    pub confirm_multiline_paste: bool,
    pub restore_session: bool,
    pub notify_on_command_completion: bool,
    pub command_completion_threshold_seconds: u16,
    pub island: crate::island::Config,
    pub shortcuts: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Profile {
    pub executable: String,
    pub arguments: Vec<String>,
    pub initial_directory: Option<PathBuf>,
    pub environment: BTreeMap<String, String>,
    pub theme: TerminalTheme,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            executable: default_shell().into(),
            arguments: Vec::new(),
            initial_directory: None,
            environment: BTreeMap::new(),
            theme: TerminalTheme::Default,
        }
    }
}
impl Profile {
    pub fn command(&self) -> Vec<String> {
        let mut command = Vec::with_capacity(self.arguments.len() + 1);
        command.push(self.executable.clone());
        command.extend(self.arguments.clone());
        command
    }

    fn validate(&self, name: &str) -> Result<(), String> {
        if self.executable.trim().is_empty() {
            return Err(format!("Profile {name:?} needs an executable"));
        }
        if self.executable.contains('\0')
            || self
                .arguments
                .iter()
                .any(|argument| argument.contains('\0'))
        {
            return Err(format!("Profile {name:?} contains an invalid command"));
        }
        for (key, value) in &self.environment {
            let valid_name = !key.is_empty()
                && key.bytes().enumerate().all(|(index, byte)| {
                    byte == b'_' || byte.is_ascii_alphabetic() || index > 0 && byte.is_ascii_digit()
                });
            if !valid_name || key.contains('=') || value.contains('\0') {
                return Err(format!(
                    "Profile {name:?} has an invalid environment variable {key:?}"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalTheme {
    #[default]
    Default,
    Dracula,
    SolarizedDark,
    SolarizedLight,
}
impl TerminalTheme {
    pub const ALL: [Self; 4] = [
        Self::Default,
        Self::Dracula,
        Self::SolarizedDark,
        Self::SolarizedLight,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Dracula => "Dracula",
            Self::SolarizedDark => "Solarized Dark",
            Self::SolarizedLight => "Solarized Light",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub font_size: u8,
    pub ligatures: bool,
    pub opacity: u8,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: 14,
            ligatures: true,
            opacity: 100,
        }
    }
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    shell: Option<String>,
    font: Option<String>,
    profile: Option<String>,
    profiles: BTreeMap<String, Profile>,
    appearance: Appearance,
    scrollback: Option<usize>,
    confirm_multiline_paste: Option<bool>,
    restore_session: Option<bool>,
    notify_on_command_completion: Option<bool>,
    command_completion_threshold_seconds: Option<u16>,
    island: crate::island::Config,
    shortcuts: BTreeMap<String, String>,
}
impl From<FileConfig> for Config {
    fn from(file: FileConfig) -> Self {
        let mut config = Self::default();
        let has_default_profile = file.profiles.contains_key("default");
        if let Some(shell) = file.shell {
            config.shell = shell;
        }
        if let Some(font) = file.font {
            config.font = font.trim().to_string();
        }
        if let Some(profile) = file.profile {
            config.profile = profile;
        }
        config.profiles.extend(file.profiles);
        if !has_default_profile {
            config.profiles.get_mut("default").unwrap().executable = config.shell.clone();
        }
        config.appearance = file.appearance;
        if let Some(scrollback) = file.scrollback {
            config.scrollback = scrollback;
        }
        if let Some(confirm_multiline_paste) = file.confirm_multiline_paste {
            config.confirm_multiline_paste = confirm_multiline_paste;
        }
        if let Some(restore_session) = file.restore_session {
            config.restore_session = restore_session;
        }
        if let Some(notify_on_command_completion) = file.notify_on_command_completion {
            config.notify_on_command_completion = notify_on_command_completion;
        }
        if let Some(command_completion_threshold_seconds) =
            file.command_completion_threshold_seconds
        {
            config.command_completion_threshold_seconds = command_completion_threshold_seconds;
        }
        config.shortcuts.extend(file.shortcuts);
        config.island = file.island;
        config
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            shell: default_shell().into(),
            font: default_font(),
            profile: "default".into(),
            profiles: BTreeMap::from([("default".into(), Profile::default())]),
            appearance: Appearance::default(),
            scrollback: crate::terminal::SCROLLBACK,
            confirm_multiline_paste: true,
            restore_session: true,
            notify_on_command_completion: true,
            command_completion_threshold_seconds: 10,
            island: crate::island::Config::default(),
            shortcuts: Action::all()
                .into_iter()
                .map(|a| (a.id(), a.default_binding()))
                .collect(),
        }
    }
}
impl Config {
    pub fn load() -> Result<Self, String> {
        Self::load_paths(&paths())
    }
    fn load_paths(paths: &[PathBuf]) -> Result<Self, String> {
        for path in paths {
            match fs::read_to_string(path) {
                Ok(text) => {
                    return Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
        Ok(Self::default())
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        config.validate()?;
        Ok(config)
    }
    pub fn bindings(&self) -> Result<Vec<(Action, Binding)>, String> {
        let mut result: Vec<(Action, Binding)> = Vec::new();
        for (id, text) in &self.shortcuts {
            if !Action::all().iter().any(|a| a.id() == *id) {
                return Err(format!("Unknown shortcut action {id:?}"));
            }
            if text.is_empty() {
                continue;
            }
        }
        for action in Action::all() {
            let text = self
                .shortcuts
                .get(&action.id())
                .map(String::as_str)
                .unwrap_or("");
            if text.is_empty() {
                continue;
            }
            let binding = Binding::parse(text).map_err(|e| format!("{}: {e}", action.label()))?;
            if let Some((other, _)) = result.iter().find(|(_, b)| *b == binding) {
                return Err(format!(
                    "{} is assigned to both {} and {}",
                    binding.display(),
                    other.label(),
                    action.label()
                ));
            }
            result.push((action, binding));
        }
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.shell.trim().is_empty() {
            return Err(format!(
                "Enter a shell executable, such as {}",
                default_shell()
            ));
        }
        if self.font.trim().is_empty() {
            return Err("Choose a terminal font".into());
        }
        if self.scrollback > 1_000_000 {
            return Err("Scrollback must not exceed 1,000,000 lines".into());
        }
        if !(8..=72).contains(&self.appearance.font_size) {
            return Err("Font size must be between 8 and 72".into());
        }
        if !(30..=100).contains(&self.appearance.opacity) {
            return Err("Window opacity must be between 30 and 100".into());
        }
        if !(1..=3_600).contains(&self.command_completion_threshold_seconds) {
            return Err("Command completion threshold must be between 1 and 3600 seconds".into());
        }
        if self.profile.trim().is_empty() {
            return Err("Choose a terminal profile".into());
        }
        if !self.profiles.contains_key(&self.profile) {
            return Err(format!("Unknown terminal profile {:?}", self.profile));
        }
        for (name, profile) in &self.profiles {
            if name.trim().is_empty() {
                return Err("Terminal profile names cannot be empty".into());
            }
            profile.validate(name)?;
        }
        self.bindings().map(|_| ())
    }
    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let parent = path.parent().ok_or("Invalid configuration directory")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = parent.join(format!(".cterm-{}.tmp", std::process::id()));
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(|e| e.to_string())
    }
    pub fn import_from(path: &Path) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        Self::parse(&text).map_err(|error| format!("{}: {error}", path.display()))
    }
    pub fn shell_command(&self) -> Result<Vec<String>, String> {
        resolve_shell(&self.shell, std::env::var_os("PATH").as_deref())
            .map(|path| vec![path.to_string_lossy().into_owned()])
    }
    pub fn active_profile(&self) -> Result<&Profile, String> {
        self.profiles
            .get(&self.profile)
            .ok_or_else(|| format!("Unknown terminal profile {:?}", self.profile))
    }
    pub fn profile_command(&self) -> Result<Vec<String>, String> {
        self.active_profile().map(Profile::command)
    }
}
pub fn path() -> PathBuf {
    delib::xdg::XdgPaths::default().user_config_file("coconut", "cterm.toml")
}
pub fn paths() -> Vec<PathBuf> {
    delib::xdg::XdgPaths::default().config_files("coconut", "cterm.toml")
}
pub fn fingerprint() -> Vec<Option<Vec<u8>>> {
    paths().iter().map(|p| fs::read(p).ok()).collect()
}
pub fn default_shell() -> &'static str {
    crate::backends::default_shell()
}
pub fn default_font() -> String {
    crate::backends::default_font()
}
fn resolve_shell(shell: &str, search: Option<&std::ffi::OsStr>) -> Result<PathBuf, String> {
    let mut names = vec![shell];
    for &fallback in crate::backends::fallback_shells() {
        if !names.contains(&fallback) {
            names.push(fallback);
        }
    }
    for name in names {
        let p = Path::new(name);
        let candidates = if p.components().count() > 1 {
            crate::backends::executable_candidates(p)
        } else {
            search
                .map(std::env::split_paths)
                .into_iter()
                .flatten()
                .flat_map(|dir| crate::backends::executable_candidates(&dir.join(p)))
                .collect()
        };
        if let Some(path) = candidates.into_iter().find(|path| {
            fs::metadata(path).is_ok_and(|meta| crate::backends::is_executable_file(&meta))
        }) {
            return if path.is_absolute() {
                Ok(path)
            } else {
                std::env::current_dir()
                    .map(|cwd| cwd.join(path))
                    .map_err(|e| e.to_string())
            };
        }
    }
    Err(crate::backends::shell_missing_message(shell))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "cterm-config-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn partial_config_merges_defaults_and_rejects_conflicts() {
        let c = Config::parse("shell='zsh'\n[shortcuts]\nnew_tab='Ctrl+Alt+T'\ncopy=''\n").unwrap();
        assert_eq!(c.shell, "zsh");
        assert_eq!(c.font, Config::default().font);
        assert_eq!(c.shortcuts["new_window"], "Ctrl+Shift+N");
        assert_eq!(c.shortcuts["copy"], "");
        assert!(Config::parse("[shortcuts]\nnew_tab='Ctrl+Shift+N'").is_err());
        assert!(Config::parse("shell=''").is_err());
        assert!(Config::parse("font=''").is_err());
        assert_eq!(Config::parse("font='Consolas'").unwrap().font, "Consolas");
        assert!(Config::parse("[shortcuts]\nunknown='Ctrl+K'").is_err());
    }
    #[test]
    fn island_config_defaults_roundtrip_and_validation() {
        use crate::island::{Horizontal, Orientation, Vertical};
        assert_eq!(
            Config::parse("shell='bash'").unwrap().island,
            crate::island::Config::default()
        );
        let config = Config::parse("[island]\nenabled=false\norientation='vertical'\nhorizontal='right'\nvertical='center'").unwrap();
        assert!(!config.island.enabled);
        assert_eq!(config.island.orientation, Orientation::Vertical);
        assert_eq!(config.island.horizontal, Horizontal::Right);
        assert_eq!(config.island.vertical, Vertical::Center);
        assert_eq!(
            Config::parse(&toml::to_string(&config).unwrap()).unwrap(),
            config
        );
        let partial = Config::parse("[island]\nhorizontal='left'").unwrap();
        assert!(partial.island.enabled);
        assert_eq!(partial.island.vertical, Vertical::Bottom);
        assert!(Config::parse("[island]\norientation='diagonal'").is_err());
        assert!(Config::parse("[island]\nvertical='left'").is_err());
        assert!(Config::parse("[island]\nunknown=true").is_err());
    }
    #[test]
    fn profiles_appearance_and_safety_preferences_merge_and_validate() {
        let config = Config::parse(
            "profile = 'work'\nscrollback = 0\nconfirm_multiline_paste = false\nrestore_session = false\nnotify_on_command_completion = false\ncommand_completion_threshold_seconds = 42\n\n[appearance]\nfont_size = 17\nligatures = false\nopacity = 87\n\n[profiles.work]\nexecutable = 'ssh'\narguments = ['dev.example']\ninitial_directory = '/tmp'\ntheme = 'dracula'\n\n[profiles.work.environment]\nLANG = 'C.UTF-8'\n",
        )
        .unwrap();
        assert_eq!(config.profile, "work");
        assert_eq!(config.profile_command().unwrap(), ["ssh", "dev.example"]);
        assert_eq!(
            config.active_profile().unwrap().theme,
            TerminalTheme::Dracula
        );
        assert_eq!(config.scrollback, 0);
        assert!(!config.confirm_multiline_paste);
        assert!(!config.restore_session);
        assert!(!config.notify_on_command_completion);
        assert_eq!(config.command_completion_threshold_seconds, 42);
        assert_eq!(config.appearance.font_size, 17);
        assert!(!config.appearance.ligatures);
        assert_eq!(config.appearance.opacity, 87);
        assert_eq!(
            Config::parse("shell='zsh'")
                .unwrap()
                .profile_command()
                .unwrap(),
            ["zsh"]
        );
        assert!(Config::parse("profile='missing'").is_err());
        assert!(Config::parse("[appearance]\nfont_size=7").is_err());
        assert!(Config::parse("[appearance]\nopacity=29").is_err());
        assert!(Config::parse("command_completion_threshold_seconds=0").is_err());
        assert!(Config::parse("[profiles.bad]\nexecutable='' ").is_err());
        assert!(Config::parse("[profiles.bad.environment]\n'1BAD'='x'").is_err());
        assert!(Config::parse("scrollback=1000001").is_err());
    }
    #[test]
    fn config_roundtrip_and_system_fallback() {
        let root = fixture();
        let user = root.join("user/cterm.toml");
        let system = root.join("system/cterm.toml");
        let c = Config {
            shell: "zsh".into(),
            ..Default::default()
        };
        c.save_to(&system).unwrap();
        assert_eq!(Config::load_paths(&[user.clone(), system]).unwrap(), c);
        Config::default().save_to(&user).unwrap();
        assert_eq!(Config::load_paths(&[user]).unwrap(), Config::default());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn imported_configuration_is_validated_before_it_is_saved() {
        let root = fixture();
        let file = root.join("import.toml");
        fs::write(&file, "[appearance]\nfont_size=18\nopacity=90\n").unwrap();
        let imported = Config::import_from(&file).unwrap();
        assert_eq!(imported.appearance.font_size, 18);
        assert_eq!(imported.appearance.opacity, 90);
        fs::write(&file, "[appearance]\nfont_size=3\n").unwrap();
        assert!(Config::import_from(&file).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn shell_falls_back_in_order_and_ignores_non_executables() {
        use std::os::unix::fs::PermissionsExt;
        let root = fixture();
        let path = std::ffi::OsString::from(&root);
        for name in ["sh", "bash", "custom"] {
            fs::write(root.join(name), "#!/bin/sh\n").unwrap();
            fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o755)).unwrap();
        }
        assert_eq!(
            resolve_shell("custom", Some(&path)).unwrap(),
            root.join("custom")
        );
        assert_eq!(
            resolve_shell("missing", Some(&path)).unwrap(),
            root.join("sh")
        );
        fs::remove_file(root.join("sh")).unwrap();
        assert_eq!(resolve_shell("sh", Some(&path)).unwrap(), root.join("bash"));
        fs::set_permissions(root.join("bash"), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(resolve_shell("missing", Some(&path)).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(windows)]
    #[test]
    fn shell_falls_back_to_windows_executables() {
        let root = fixture();
        let path = std::ffi::OsString::from(&root);
        fs::write(root.join("custom"), "not executable").unwrap();
        for name in ["powershell.exe", "pwsh.exe", "cmd.exe", "custom.exe"] {
            fs::write(root.join(name), "").unwrap();
        }
        assert_eq!(
            resolve_shell("custom", Some(&path))
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase(),
            root.join("custom.exe")
                .to_string_lossy()
                .to_ascii_lowercase()
        );
        assert_eq!(
            resolve_shell("missing", Some(&path))
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase(),
            root.join("powershell.exe")
                .to_string_lossy()
                .to_ascii_lowercase()
        );
        fs::remove_file(root.join("powershell.exe")).unwrap();
        assert_eq!(
            resolve_shell("missing", Some(&path))
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase(),
            root.join("pwsh.exe").to_string_lossy().to_ascii_lowercase()
        );
        fs::remove_file(root.join("pwsh.exe")).unwrap();
        assert_eq!(
            resolve_shell("missing", Some(&path))
                .unwrap()
                .to_string_lossy()
                .to_ascii_lowercase(),
            root.join("cmd.exe").to_string_lossy().to_ascii_lowercase()
        );
        fs::remove_file(root.join("cmd.exe")).unwrap();
        assert!(resolve_shell("missing", Some(&path)).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
