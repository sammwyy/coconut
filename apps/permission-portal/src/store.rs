use delib::xdg::XdgPaths;
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

pub const SCREEN_CAPTURE: &str = "screen-capture";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    Allow,
    Deny,
}

impl Verdict {
    pub fn allowed(self) -> bool {
        matches!(self, Self::Allow)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub subject: String,
    pub permission: String,
    pub verdict: Verdict,
    pub granted_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Disk {
    entries: Vec<Entry>,
}

pub struct Store {
    path: PathBuf,
    entries: Vec<Entry>,
    trusted_capture: Option<PathBuf>,
}

pub fn store_path() -> PathBuf {
    XdgPaths::from_env()
        .data_home
        .join("coconut/permissions.json")
}

impl Store {
    pub fn load(path: PathBuf) -> io::Result<Self> {
        let entries = match std::fs::read(&path) {
            Ok(raw) => serde_json::from_slice::<Disk>(&raw)?.entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            path,
            entries,
            trusted_capture: crate::identity::sibling("coconut-screenshots")
                .and_then(|path| path.canonicalize().ok()),
        })
    }

    pub fn lookup(&self, subject: &str, permission: &str) -> Option<Verdict> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.subject == subject && entry.permission == permission)
            .map(|entry| entry.verdict)
            .or_else(|| {
                (permission == SCREEN_CAPTURE
                    && self.trusted_capture.as_deref() == Some(Path::new(subject)))
                .then_some(Verdict::Allow)
            })
    }

    pub fn set(&mut self, subject: &str, permission: &str, verdict: Verdict) -> io::Result<()> {
        let mut entries = self.entries.clone();
        entries.retain(|entry| !(entry.subject == subject && entry.permission == permission));
        entries.push(Entry {
            subject: subject.into(),
            permission: permission.into(),
            verdict,
            granted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
        });
        self.replace(entries)
    }

    pub fn revoke(&mut self, subject: &str, permission: &str) -> io::Result<bool> {
        let mut entries = self.entries.clone();
        entries.retain(|entry| !(entry.subject == subject && entry.permission == permission));
        let changed = entries.len() != self.entries.len();
        if changed {
            self.replace(entries)?;
        }
        Ok(changed)
    }

    pub fn forget(&mut self, subject: &str) -> io::Result<usize> {
        let mut entries = self.entries.clone();
        entries.retain(|entry| entry.subject != subject);
        let removed = self.entries.len() - entries.len();
        if removed > 0 {
            self.replace(entries)?;
        }
        Ok(removed)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn replace(&mut self, entries: Vec<Entry>) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("Missing permissions directory"))?;
        std::fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(
            &mut temporary,
            &Disk {
                entries: entries.clone(),
            },
        )?;
        temporary.flush()?;
        temporary.as_file().sync_all()?;
        temporary.persist(&self.path).map_err(|error| error.error)?;
        self.entries = entries;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preseed_matches_only_the_installed_path_and_can_be_overridden() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("permissions.json")).unwrap();
        store.trusted_capture = Some("/usr/bin/coconut-screenshots".into());
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
        assert_eq!(
            store.lookup("/tmp/coconut-screenshots", SCREEN_CAPTURE),
            None
        );
        assert_eq!(
            store.lookup("flatpak:coconut-screenshots", SCREEN_CAPTURE),
            None
        );
        store
            .set(
                "/usr/bin/coconut-screenshots",
                SCREEN_CAPTURE,
                Verdict::Deny,
            )
            .unwrap();
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Deny)
        );
        assert!(store
            .revoke("/usr/bin/coconut-screenshots", SCREEN_CAPTURE)
            .unwrap());
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
    }

    #[test]
    fn decisions_survive_reload_and_revoke_and_forget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.json");
        let mut store = Store::load(path.clone()).unwrap();
        store
            .set("/usr/bin/foo", SCREEN_CAPTURE, Verdict::Allow)
            .unwrap();
        store.set("/usr/bin/foo", "camera", Verdict::Deny).unwrap();
        store
            .set("/usr/bin/bar", SCREEN_CAPTURE, Verdict::Deny)
            .unwrap();
        let mut store = Store::load(path.clone()).unwrap();
        assert_eq!(
            store.lookup("/usr/bin/foo", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
        assert!(store.revoke("/usr/bin/foo", SCREEN_CAPTURE).unwrap());
        assert!(!store.revoke("/usr/bin/foo", SCREEN_CAPTURE).unwrap());
        assert_eq!(store.forget("/usr/bin/foo").unwrap(), 1);
        let store = Store::load(path).unwrap();
        assert_eq!(store.entries().len(), 1);
        assert_eq!(
            store.lookup("/usr/bin/bar", SCREEN_CAPTURE),
            Some(Verdict::Deny)
        );
    }

    #[test]
    fn failed_writes_preserve_memory_and_previous_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.json");
        let mut store = Store::load(path.clone()).unwrap();
        store.set("foo", SCREEN_CAPTURE, Verdict::Deny).unwrap();
        let original = std::fs::read(&path).unwrap();
        store.path = path.join("impossible");
        assert!(store.set("foo", SCREEN_CAPTURE, Verdict::Allow).is_err());
        assert!(store.revoke("foo", SCREEN_CAPTURE).is_err());
        assert!(store.forget("foo").is_err());
        assert_eq!(store.lookup("foo", SCREEN_CAPTURE), Some(Verdict::Deny));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn corrupt_or_unreadable_store_is_not_silently_reset() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.json");
        std::fs::write(&path, "{broken").unwrap();
        assert!(Store::load(path).is_err());
        assert!(Store::load(dir.path().into()).is_err());
    }
}
