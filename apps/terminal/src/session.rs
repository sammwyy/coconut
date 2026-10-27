use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SavedSession {
    pub active: usize,
    pub tabs: Vec<SavedTab>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedTab {
    pub cwd: PathBuf,
    pub manual_title: Option<String>,
}

pub fn path() -> PathBuf {
    crate::config::path().with_file_name("cterm-session.toml")
}

pub fn load(path: &Path) -> Result<Option<SavedSession>, String> {
    match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text)
            .map(Some)
            .map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

pub fn save(path: &Path, session: &SavedSession) -> Result<(), String> {
    let text = toml::to_string_pretty(session).map_err(|error| error.to_string())?;
    let parent = path.parent().ok_or("Invalid session directory")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(".cterm-session-{}.tmp", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.map_err(|error| error.to_string())
}

pub fn clear(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_loads_and_clears_sessions_atomically() {
        let root = std::env::temp_dir().join(format!(
            "cterm-session-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let file = root.join("session.toml");
        let session = SavedSession {
            active: 1,
            tabs: vec![
                SavedTab {
                    cwd: PathBuf::from("/tmp/one"),
                    manual_title: Some("One".into()),
                },
                SavedTab {
                    cwd: PathBuf::from("/tmp/two"),
                    manual_title: None,
                },
            ],
        };
        assert_eq!(load(&file).unwrap(), None);
        save(&file, &session).unwrap();
        assert_eq!(load(&file).unwrap(), Some(session));
        clear(&file).unwrap();
        assert_eq!(load(&file).unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }
}
