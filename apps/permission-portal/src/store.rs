//! Persistent permission records. Subjects are absolute executable paths
//! (or Flatpak app ids); permissions are namespaced capability names such as
//! `screen-capture`. Only durable decisions are written — "allow once"
//! verdicts live and die with the pending request.

use delib::xdg::XdgPaths;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Capabilities the service currently knows how to mediate.
pub const SCREEN_CAPTURE: &str = "screen-capture";

/// Subjects trusted without prompting. `coconut-screenshots` is the session's
/// own capture tool: matching on the full executable path (not just the name)
/// means a renamed binary at a foreign location still triggers the dialog —
/// replacing the installed binary would require write access the attacker
/// already needed to bypass everything else anyway.
const PRESEEDED: &[(&str, &str)] = &[("coconut-screenshots", SCREEN_CAPTURE)];

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
    /// Unix seconds when the decision was recorded.
    pub granted_at: u64,
}

#[derive(Default, Serialize, Deserialize)]
struct Disk {
    entries: Vec<Entry>,
}

pub struct Store {
    path: PathBuf,
    entries: Vec<Entry>,
}

fn store_path() -> PathBuf {
    XdgPaths::from_env()
        .data_home
        .join("coconut/permissions.json")
}

fn is_preseeded(subject: &str, permission: &str) -> bool {
    PRESEEDED
        .iter()
        .any(|(name, pre)| *pre == permission && subject.rsplit('/').next() == Some(*name))
}

impl Store {
    pub fn load(path: PathBuf) -> Self {
        let entries = std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Disk>(&raw).ok())
            .map(|disk| disk.entries)
            .unwrap_or_default();
        Self { path, entries }
    }

    pub fn lookup(&self, subject: &str, permission: &str) -> Option<Verdict> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.subject == subject && entry.permission == permission)
            .map(|entry| entry.verdict)
            .or(is_preseeded(subject, permission).then_some(Verdict::Allow))
    }

    pub fn set(&mut self, subject: &str, permission: &str, verdict: Verdict) {
        self.entries
            .retain(|entry| !(entry.subject == subject && entry.permission == permission));
        self.entries.push(Entry {
            subject: subject.into(),
            permission: permission.into(),
            verdict,
            granted_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs())
                .unwrap_or_default(),
        });
        self.save();
    }

    /// Removes one stored decision so the next access prompts again.
    /// Returns false when nothing was stored.
    pub fn revoke(&mut self, subject: &str, permission: &str) -> bool {
        let before = self.entries.len();
        self.entries
            .retain(|entry| !(entry.subject == subject && entry.permission == permission));
        let changed = before != self.entries.len();
        if changed {
            self.save();
        }
        changed
    }

    /// Drops every record for a subject.
    pub fn forget(&mut self, subject: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.subject != subject);
        let removed = before - self.entries.len();
        if removed > 0 {
            self.save();
        }
        removed
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn save(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let disk = Disk {
            entries: self.entries.clone(),
        };
        if let Ok(raw) = serde_json::to_string_pretty(&disk) {
            let _ = std::fs::write(&self.path, raw);
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::load(store_path())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn store() -> Store {
        Store::load(Path::new("/nonexistent/permissions.json").to_path_buf())
    }

    #[test]
    fn preseeded_subjects_are_allowed() {
        let store = store();
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
        assert_eq!(
            store.lookup("/home/u/.local/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots-evil", SCREEN_CAPTURE),
            None
        );
        assert_eq!(store.lookup("/usr/bin/grim", SCREEN_CAPTURE), None);
    }

    #[test]
    fn set_overrides_preseed_and_revoke_restores_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("permissions.json"));
        store.set(
            "/usr/bin/coconut-screenshots",
            SCREEN_CAPTURE,
            Verdict::Deny,
        );
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Deny)
        );
        assert!(store.revoke("/usr/bin/coconut-screenshots", SCREEN_CAPTURE));
        assert_eq!(
            store.lookup("/usr/bin/coconut-screenshots", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
        assert!(!store.revoke("/usr/bin/coconut-screenshots", SCREEN_CAPTURE));
    }

    #[test]
    fn entries_survive_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.json");
        let mut store = Store::load(path.clone());
        store.set("/usr/bin/foo", SCREEN_CAPTURE, Verdict::Allow);
        drop(store);
        let store = Store::load(path);
        assert_eq!(
            store.lookup("/usr/bin/foo", SCREEN_CAPTURE),
            Some(Verdict::Allow)
        );
    }

    #[test]
    fn forget_removes_every_entry_for_a_subject() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::load(dir.path().join("permissions.json"));
        store.set("/usr/bin/foo", SCREEN_CAPTURE, Verdict::Allow);
        store.set("/usr/bin/foo", "camera", Verdict::Deny);
        store.set("/usr/bin/bar", SCREEN_CAPTURE, Verdict::Deny);
        assert_eq!(store.forget("/usr/bin/foo"), 2);
        assert_eq!(store.lookup("/usr/bin/foo", SCREEN_CAPTURE), None);
        assert_eq!(
            store.lookup("/usr/bin/bar", SCREEN_CAPTURE),
            Some(Verdict::Deny)
        );
    }
}
