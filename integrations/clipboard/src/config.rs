use delib::xdg::XdgPaths;
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub max_entries: usize,
    pub max_item_bytes: usize,
    pub max_total_bytes: usize,
    pub max_age_seconds: u64,
    pub seat: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 50,
            max_item_bytes: 1024 * 1024,
            max_total_bytes: 8 * 1024 * 1024,
            max_age_seconds: 24 * 60 * 60,
            seat: None,
        }
    }
}

impl Config {
    fn path() -> PathBuf {
        XdgPaths::from_env()
            .config_home
            .join("coconut/clipboard.toml")
    }

    pub fn load() -> io::Result<Self> {
        Self::load_path(&Self::path())
    }

    pub fn save(&self, previous: &Self) -> io::Result<()> {
        self.validate()?;
        if &Self::load()? != previous {
            return Err(io::Error::other("Clipboard configuration changed on disk; restart the service before changing preferences"));
        }
        let path = Self::path();
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("Missing configuration directory"))?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(
            toml::to_string_pretty(self)
                .map_err(io::Error::other)?
                .as_bytes(),
        )?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|error| error.error)?;
        Ok(())
    }

    pub fn load_path(path: &Path) -> io::Result<Self> {
        let config = match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(io::Error::other)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Self::default(),
            Err(error) => return Err(error),
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> io::Result<()> {
        if !(1..=1000).contains(&self.max_entries)
            || !(1..=4 * 1024 * 1024).contains(&self.max_item_bytes)
            || !(self.max_item_bytes..=64 * 1024 * 1024).contains(&self.max_total_bytes)
            || !(1..=30 * 24 * 60 * 60).contains(&self.max_age_seconds)
        {
            return Err(io::Error::other("Invalid clipboard retention limits"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_rejects_unbounded_or_misspelled_limits() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("clipboard.toml");
        assert_eq!(Config::load_path(&path).unwrap().max_entries, 50);
        for invalid in [
            "max_entries = 0",
            "max_item_bytes = 999999999",
            "max_age_seconds = 0",
            "max_entires = 5",
        ] {
            std::fs::write(&path, invalid).unwrap();
            assert!(Config::load_path(&path).is_err(), "{invalid}");
        }
    }
}
