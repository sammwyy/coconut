use delib::{desktop_entry::DesktopEntry, xdg::XdgPaths};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinnedApplication {
    pub source: PathBuf,
    pub desktop_file: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PinnedApplications {
    pub applications: Vec<PinnedApplication>,
}

#[derive(Clone)]
pub struct PinnedAppsStore {
    config: PathBuf,
    launchers: PathBuf,
}

impl Default for PinnedAppsStore {
    fn default() -> Self {
        Self::new(
            super::config_dir().join("coconut/pinned-apps.toml"),
            XdgPaths::default().data_home.join("coconut/pinned-apps"),
        )
    }
}

impl PinnedAppsStore {
    pub fn new(config: PathBuf, launchers: PathBuf) -> Self {
        Self { config, launchers }
    }

    pub fn load(&self) -> io::Result<PinnedApplications> {
        match fs::read_to_string(&self.config) {
            Ok(contents) => toml::from_str(&contents).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Default::default()),
            Err(error) => Err(error),
        }
    }

    pub fn pin(&self, paths: &[PathBuf]) -> io::Result<PinnedApplications> {
        self.update(|pins| {
            let mut additions = Vec::new();
            for path in paths {
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_none_or(|ext| !ext.eq_ignore_ascii_case("desktop"))
                {
                    return Err(io::Error::other("Only .desktop applications can be pinned"));
                }
                let source = std::path::absolute(path)?;
                let canonical = fs::canonicalize(&source)?;
                if pins.applications.iter().chain(additions.iter()).any(
                    |pin: &PinnedApplication| {
                        pin.source == source
                            || pin.desktop_file == source
                            || fs::canonicalize(&pin.source)
                                .is_ok_and(|existing| existing == canonical)
                    },
                ) {
                    continue;
                }
                let contents = fs::read_to_string(&source)?;
                let entry = DesktopEntry::parse(&source, &contents);
                if !entry.is_visible_application()
                    || entry.name(None).is_none_or(str::is_empty)
                    || entry.command_for_files(&[]).is_none()
                {
                    return Err(io::Error::other(format!(
                        "{} is not a launchable application",
                        path.display()
                    )));
                }
                let key = source
                    .as_os_str()
                    .as_encoded_bytes()
                    .iter()
                    .fold(0xcbf29ce484222325_u64, |hash, byte| {
                        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                    });
                let mut suffix = 0;
                let desktop_file = loop {
                    let name = if suffix == 0 {
                        format!("{key:016x}.desktop")
                    } else {
                        format!("{key:016x}-{suffix}.desktop")
                    };
                    let path = self.launchers.join(name);
                    if !pins
                        .applications
                        .iter()
                        .chain(additions.iter())
                        .any(|pin| pin.desktop_file == path)
                    {
                        break path;
                    }
                    suffix += 1;
                };
                fs::create_dir_all(&self.launchers)?;
                super::atomic_write_config(&desktop_file, contents.as_bytes())?;
                additions.push(PinnedApplication {
                    source,
                    desktop_file,
                });
            }
            pins.applications.extend(additions);
            Ok(())
        })
    }

    pub fn unpin(&self, source: &Path) -> io::Result<PinnedApplications> {
        self.update(|pins| {
            pins.applications.retain(|pin| pin.source != source);
            Ok(())
        })
    }

    fn update(
        &self,
        edit: impl FnOnce(&mut PinnedApplications) -> io::Result<()>,
    ) -> io::Result<PinnedApplications> {
        fs::create_dir_all(
            self.config
                .parent()
                .ok_or_else(|| io::Error::other("Missing configuration directory"))?,
        )?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.config.with_extension("lock"))?;
        lock.lock()?;
        let mut pins = self.load()?;
        edit(&mut pins)?;
        let contents = toml::to_string_pretty(&pins).map_err(io::Error::other)?;
        super::atomic_write_config(&self.config, contents.as_bytes())?;
        Ok(pins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn launcher(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(format!("{name}.desktop"));
        fs::write(&path, format!("[Desktop Entry]\nType=Application\nName={name}\nExec=example %U\nCategories=Utility;\n")).unwrap();
        path
    }

    #[test]
    fn pins_are_copied_persistent_and_idempotent() {
        let root = tempdir().unwrap();
        let store = PinnedAppsStore::new(
            root.path().join("config/pins.toml"),
            root.path().join("data"),
        );
        let source = launcher(root.path(), "Example app");
        let pins = store.pin(&[source.clone(), source.clone()]).unwrap();
        assert_eq!(pins.applications.len(), 1);
        assert_eq!(
            fs::read(&source).unwrap(),
            fs::read(&pins.applications[0].desktop_file).unwrap()
        );
        assert_eq!(store.load().unwrap(), pins);
        let copy = pins.applications[0].desktop_file.clone();
        assert_eq!(store.pin(std::slice::from_ref(&copy)).unwrap(), pins);
        fs::remove_file(&source).unwrap();
        assert!(DesktopEntry::parse_file(&copy).unwrap().is_application());
        assert!(store.unpin(&source).unwrap().applications.is_empty());
        assert!(copy.exists());
    }

    #[test]
    fn invalid_input_and_corrupt_config_preserve_saved_pins() {
        let root = tempdir().unwrap();
        let config = root.path().join("pins.toml");
        let store = PinnedAppsStore::new(config.clone(), root.path().join("data"));
        let source = launcher(root.path(), "Valid");
        let pins = store.pin(std::slice::from_ref(&source)).unwrap();
        let invalid = root.path().join("bad.desktop");
        fs::write(
            &invalid,
            "[Desktop Entry]\nType=Link\nURL=https://example.com\n",
        )
        .unwrap();
        assert!(store.pin(&[invalid]).is_err());
        assert_eq!(store.load().unwrap(), pins);
        assert!(store.pin(&[root.path().join("bad.txt")]).is_err());
        fs::write(&config, "invalid = [").unwrap();
        assert!(store.pin(&[source]).is_err());
        assert_eq!(fs::read_to_string(config).unwrap(), "invalid = [");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_launchers_keep_the_catalog_path_and_deduplicate_aliases() {
        let root = tempdir().unwrap();
        let source = launcher(root.path(), "Source");
        let alias = root.path().join("Installed.desktop");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        let store = PinnedAppsStore::new(root.path().join("pins.toml"), root.path().join("data"));
        let pins = store.pin(&[alias.clone(), source]).unwrap();
        assert_eq!(pins.applications.len(), 1);
        assert_eq!(pins.applications[0].source, alias);
    }

    #[test]
    fn concurrent_writers_keep_both_applications() {
        let root = tempdir().unwrap();
        let store = PinnedAppsStore::new(root.path().join("pins.toml"), root.path().join("data"));
        let threads: Vec<_> = ["First", "Second"]
            .into_iter()
            .map(|name| {
                let path = launcher(root.path(), name);
                let store = store.clone();
                std::thread::spawn(move || store.pin(&[path]).unwrap())
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(store.load().unwrap().applications.len(), 2);
    }
}
