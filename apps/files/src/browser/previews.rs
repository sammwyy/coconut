use creamui_image::ImageData;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

const CAPACITY: usize = 256;

#[derive(Default)]
pub(super) struct PreviewCache {
    entries: HashMap<PathBuf, CachedPreview>,
    clock: u64,
}

struct CachedPreview {
    image: Option<ImageData>,
    accessed: u64,
}

impl PreviewCache {
    pub(super) fn get(&mut self, path: &Path) -> Option<Option<ImageData>> {
        let accessed = self.access();
        self.entries.get_mut(path).map(|entry| {
            entry.accessed = accessed;
            entry.image.clone()
        })
    }

    pub(super) fn insert(&mut self, path: PathBuf, image: Option<ImageData>) {
        if self.entries.len() >= CAPACITY && !self.entries.contains_key(&path) {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.accessed)
                .map(|(path, _)| path.clone());
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        let accessed = self.access();
        self.entries.insert(path, CachedPreview { image, accessed });
    }

    pub(super) fn retain(&mut self, mut keep: impl FnMut(&Path) -> bool) {
        self.entries.retain(|path, _| keep(path));
    }

    fn access(&mut self) -> u64 {
        self.clock = self.clock.wrapping_add(1);
        self.clock
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_keeps_recently_viewed_previews() {
        let mut cache = PreviewCache::default();
        let image = ImageData::from_rgba(1, 1, vec![255; 4]).unwrap();
        for index in 0..CAPACITY {
            cache.insert(PathBuf::from(format!("{index}.png")), Some(image.clone()));
        }
        assert!(cache.get(Path::new("0.png")).unwrap().is_some());
        cache.insert(PathBuf::from("new.png"), Some(image));
        assert!(cache.get(Path::new("0.png")).is_some());
        assert!(cache.get(Path::new("1.png")).is_none());
        assert_eq!(cache.entries.len(), CAPACITY);
    }

    #[test]
    fn unavailable_previews_remain_cached_and_updates_do_not_evict_other_files() {
        let mut cache = PreviewCache::default();
        for index in 0..CAPACITY {
            cache.insert(PathBuf::from(format!("{index}.png")), None);
        }
        assert!(matches!(cache.get(Path::new("0.png")), Some(None)));
        let image = ImageData::from_rgba(1, 1, vec![255; 4]).unwrap();
        cache.insert(PathBuf::from("0.png"), Some(image));
        assert_eq!(cache.entries.len(), CAPACITY);
        assert!(cache.get(Path::new("1.png")).is_some());
        assert!(cache.get(Path::new("0.png")).unwrap().is_some());
        cache.retain(|path| path == Path::new("0.png"));
        assert!(cache.get(Path::new("1.png")).is_none());
        assert_eq!(cache.entries.len(), 1);
    }
}
