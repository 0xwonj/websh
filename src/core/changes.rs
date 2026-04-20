//! ChangeSet — unified tracker for in-progress filesystem edits.
//!
//! See `docs/superpowers/specs/2026-04-20-phase3-write-design.md` §3.2.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::models::{DirectoryMetadata, FileMetadata, VirtualPath};
use crate::utils::current_timestamp;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChangeType {
    CreateFile { content: String, meta: FileMetadata },
    CreateBinary { blob_id: String, mime: String, meta: FileMetadata },
    UpdateFile { content: String, description: Option<String> },
    DeleteFile,
    CreateDirectory { meta: DirectoryMetadata },
    DeleteDirectory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub change: ChangeType,
    pub staged: bool,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    entries: BTreeMap<VirtualPath, Entry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub creates_staged: usize,
    pub creates_unstaged: usize,
    pub updates_staged: usize,
    pub updates_unstaged: usize,
    pub deletes_staged: usize,
    pub deletes_unstaged: usize,
}

impl Summary {
    pub fn total(&self) -> usize {
        self.creates_staged + self.creates_unstaged
            + self.updates_staged + self.updates_unstaged
            + self.deletes_staged + self.deletes_unstaged
    }
    pub fn total_staged(&self) -> usize {
        self.creates_staged + self.updates_staged + self.deletes_staged
    }
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert-or-replace a change at `path`. New entries default to `staged = true`
    /// in Phase 3a (this flips to `false` in Phase 3b — spec §12.2/§12.3).
    pub fn upsert(&mut self, path: VirtualPath, change: ChangeType) {
        let entry = Entry {
            change,
            staged: true,
            timestamp: current_timestamp(),
        };
        self.entries.insert(path, entry);
    }

    pub fn stage(&mut self, path: &VirtualPath) {
        if let Some(e) = self.entries.get_mut(path) {
            e.staged = true;
        }
    }

    pub fn unstage(&mut self, path: &VirtualPath) {
        if let Some(e) = self.entries.get_mut(path) {
            e.staged = false;
        }
    }

    pub fn discard(&mut self, path: &VirtualPath) {
        self.entries.remove(path);
    }

    pub fn stage_all(&mut self) {
        for e in self.entries.values_mut() {
            e.staged = true;
        }
    }

    pub fn unstage_all(&mut self) {
        for e in self.entries.values_mut() {
            e.staged = false;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn get(&self, path: &VirtualPath) -> Option<&Entry> {
        self.entries.get(path)
    }

    pub fn is_staged(&self, path: &VirtualPath) -> bool {
        self.entries.get(path).is_some_and(|e| e.staged)
    }

    pub fn is_deleted(&self, path: &VirtualPath) -> bool {
        matches!(
            self.entries.get(path).map(|e| &e.change),
            Some(ChangeType::DeleteFile | ChangeType::DeleteDirectory)
        )
    }

    pub fn iter_all(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)> {
        self.entries.iter()
    }

    pub fn iter_staged(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)> {
        self.entries.iter().filter(|(_, e)| e.staged)
    }

    pub fn iter_unstaged(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)> {
        self.entries.iter().filter(|(_, e)| !e.staged)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn summary(&self) -> Summary {
        let mut s = Summary::default();
        for (_, e) in self.iter_all() {
            let bucket = match &e.change {
                ChangeType::CreateFile { .. }
                | ChangeType::CreateBinary { .. }
                | ChangeType::CreateDirectory { .. } => {
                    if e.staged { &mut s.creates_staged } else { &mut s.creates_unstaged }
                }
                ChangeType::UpdateFile { .. } => {
                    if e.staged { &mut s.updates_staged } else { &mut s.updates_unstaged }
                }
                ChangeType::DeleteFile | ChangeType::DeleteDirectory => {
                    if e.staged { &mut s.deletes_staged } else { &mut s.deletes_unstaged }
                }
            };
            *bucket += 1;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    fn create_file(content: &str) -> ChangeType {
        ChangeType::CreateFile {
            content: content.to_string(),
            meta: FileMetadata::default(),
        }
    }

    #[test]
    fn upsert_defaults_staged_true() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("hi"));
        assert!(cs.is_staged(&p("/a.md")));
        assert_eq!(cs.len(), 1);
    }

    #[test]
    fn unstage_then_stage_roundtrip() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("hi"));
        cs.unstage(&p("/a.md"));
        assert!(!cs.is_staged(&p("/a.md")));
        cs.stage(&p("/a.md"));
        assert!(cs.is_staged(&p("/a.md")));
    }

    #[test]
    fn discard_removes_entry() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("hi"));
        cs.discard(&p("/a.md"));
        assert!(cs.get(&p("/a.md")).is_none());
    }

    #[test]
    fn is_deleted_matches_delete_variants() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/gone.md"), ChangeType::DeleteFile);
        cs.upsert(p("/keep.md"), create_file("x"));
        assert!(cs.is_deleted(&p("/gone.md")));
        assert!(!cs.is_deleted(&p("/keep.md")));
    }

    #[test]
    fn iter_all_yields_sorted_order() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/z.md"), create_file("z"));
        cs.upsert(p("/a.md"), create_file("a"));
        cs.upsert(p("/m.md"), create_file("m"));
        let paths: Vec<_> = cs.iter_all().map(|(p, _)| p.as_str().to_string()).collect();
        assert_eq!(paths, vec!["/a.md", "/m.md", "/z.md"]);
    }

    #[test]
    fn iter_staged_filters_unstaged() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("a"));
        cs.upsert(p("/b.md"), create_file("b"));
        cs.unstage(&p("/b.md"));
        let staged: Vec<_> = cs.iter_staged().map(|(p, _)| p.as_str().to_string()).collect();
        assert_eq!(staged, vec!["/a.md"]);
    }

    #[test]
    fn summary_counts_buckets() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/new.md"), create_file("x"));
        cs.upsert(
            p("/upd.md"),
            ChangeType::UpdateFile { content: "y".into(), description: None },
        );
        cs.upsert(p("/del.md"), ChangeType::DeleteFile);
        cs.unstage(&p("/del.md"));
        let s = cs.summary();
        assert_eq!(s.creates_staged, 1);
        assert_eq!(s.updates_staged, 1);
        assert_eq!(s.deletes_unstaged, 1);
        assert_eq!(s.total(), 3);
        assert_eq!(s.total_staged(), 2);
    }
}
