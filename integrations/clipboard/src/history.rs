use crate::{config::Config, Entry, Status};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::watch;

struct Item {
    id: u64,
    copied_at: u64,
    inserted: Instant,
    text: Arc<str>,
}

pub(crate) struct History {
    pub config: Config,
    items: VecDeque<Item>,
    pub current: Option<Arc<str>>,
    pub generation: u64,
    next_id: u64,
    pub available: bool,
    pub error: String,
    pub changed: watch::Sender<u64>,
}

impl History {
    pub fn new(config: Config, changed: watch::Sender<u64>) -> Self {
        let next_id = changed.borrow().wrapping_add(1);
        Self {
            config,
            items: VecDeque::new(),
            current: None,
            generation: 0,
            next_id,
            available: false,
            error: "Connecting to Wayland…".into(),
            changed,
        }
    }

    pub fn notify(&self) {
        self.changed
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }

    pub fn selection_changed(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.current = None;
        self.notify();
        self.generation
    }

    pub fn record(&mut self, text: Arc<str>) {
        if text.len() > self.config.max_item_bytes || text.is_empty() {
            return;
        }
        self.current = Some(text.clone());
        if self.config.enabled {
            self.items.retain(|item| item.text != text);
            self.items.push_front(Item {
                id: self.next_id,
                copied_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                inserted: Instant::now(),
                text,
            });
            self.next_id = self.next_id.wrapping_add(1);
            self.prune();
        }
        self.notify();
    }

    pub fn prune(&mut self) {
        let before = self.items.len();
        let mut bytes = 0;
        self.items.retain(|item| {
            if item.text.len() > self.config.max_item_bytes
                || bytes + item.text.len() > self.config.max_total_bytes
                || item.inserted.elapsed() >= Duration::from_secs(self.config.max_age_seconds)
            {
                return false;
            }
            bytes += item.text.len();
            true
        });
        self.items.truncate(self.config.max_entries);
        if before != self.items.len() {
            self.notify();
        }
    }

    pub fn entries(&mut self, offset: usize, limit: usize) -> Vec<Entry> {
        self.prune();
        self.items
            .iter()
            .skip(offset)
            .take(limit.min(50))
            .map(|item| {
                (
                    item.id,
                    item.copied_at,
                    item.text.chars().take(512).collect(),
                )
            })
            .collect()
    }

    pub fn text(&mut self, id: u64) -> Option<Arc<str>> {
        self.prune();
        self.items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.text.clone())
    }

    pub fn remove(&mut self, id: u64) -> bool {
        let before = self.items.len();
        self.items.retain(|item| item.id != id);
        let removed = before != self.items.len();
        if removed {
            self.notify();
        }
        removed
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.notify();
    }

    pub fn status(&self) -> Status {
        (
            self.available,
            self.config.enabled,
            self.error.clone(),
            *self.changed.borrow(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history() -> History {
        History::new(
            Config {
                max_entries: 2,
                max_item_bytes: 8,
                max_total_bytes: 8,
                ..Config::default()
            },
            watch::channel(0).0,
        )
    }

    #[test]
    fn bounded_history_deduplicates_without_reusing_identifiers() {
        let mut history = history();
        for text in ["one", "two", "one", "three"] {
            history.record(text.into());
        }
        let entries = history.entries(0, 50);
        assert_eq!(
            entries,
            vec![
                (4, entries[0].1, "three".into()),
                (3, entries[1].1, "one".into())
            ]
        );
        assert!(history.text(1).is_none());
        assert_eq!(history.entries(1, 1)[0].2, "one");
        history.record("oversized".into());
        assert_eq!(history.entries(0, 50), entries);
    }

    #[test]
    fn expiry_pause_clear_and_removal_preserve_current_selection() {
        let mut history = history();
        history.record("secret".into());
        history.items[0].inserted -= Duration::from_secs(100_000);
        assert!(history.entries(0, 50).is_empty());
        history.config.enabled = false;
        history.record("paused".into());
        assert!(history.entries(0, 50).is_empty());
        assert_eq!(history.current.as_deref(), Some("paused"));
        history.config.enabled = true;
        history.record("new".into());
        assert!(history.remove(2));
        history.clear();
        assert_eq!(history.current.as_deref(), Some("new"));
    }
}
