use std::collections::{BTreeMap, VecDeque};

use websh_core::domain::VirtualPath;

const MAX_TEXT_CACHE_ENTRIES: usize = 64;
const MAX_TEXT_CACHE_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ContentTextCacheKey {
    pub generation: u64,
    pub mount_root: VirtualPath,
    pub rel_path: String,
}

#[derive(Clone, Debug)]
struct CacheEntry {
    text: String,
    bytes: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ContentTextCache {
    entries: BTreeMap<ContentTextCacheKey, CacheEntry>,
    order: VecDeque<ContentTextCacheKey>,
    total_bytes: usize,
}

impl ContentTextCache {
    pub fn get(&mut self, key: &ContentTextCacheKey) -> Option<String> {
        let text = self.entries.get(key)?.text.clone();
        self.touch(key);
        Some(text)
    }

    pub fn insert(&mut self, key: ContentTextCacheKey, text: String) {
        let bytes = text.len();
        if bytes > MAX_TEXT_CACHE_BYTES {
            self.remove(&key);
            return;
        }

        self.remove(&key);
        self.total_bytes += bytes;
        self.order.push_back(key.clone());
        self.entries.insert(key, CacheEntry { text, bytes });
        self.prune();
    }

    pub fn evict_path(&mut self, path: &VirtualPath) {
        let keys = self
            .entries
            .keys()
            .filter(|key| {
                path.starts_with(&key.mount_root)
                    && path
                        .strip_prefix(&key.mount_root)
                        .is_some_and(|rel| rel == key.rel_path)
            })
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            self.remove(&key);
        }
    }

    pub fn evict_mount(&mut self, mount_root: &VirtualPath) {
        let keys = self
            .entries
            .keys()
            .filter(|key| &key.mount_root == mount_root)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            self.remove(&key);
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
        self.total_bytes = 0;
    }

    fn touch(&mut self, key: &ContentTextCacheKey) {
        self.order.retain(|existing| existing != key);
        self.order.push_back(key.clone());
    }

    fn remove(&mut self, key: &ContentTextCacheKey) {
        if let Some(entry) = self.entries.remove(key) {
            self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
        }
        self.order.retain(|existing| existing != key);
    }

    fn prune(&mut self) {
        while self.entries.len() > MAX_TEXT_CACHE_ENTRIES || self.total_bytes > MAX_TEXT_CACHE_BYTES
        {
            let Some(key) = self.order.pop_front() else {
                break;
            };
            if let Some(entry) = self.entries.remove(&key) {
                self.total_bytes = self.total_bytes.saturating_sub(entry.bytes);
            }
        }
    }
}
