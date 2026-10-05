use crate::domain::{DirEntry, FsEntry, NodeMetadata, VirtualPath};

use super::super::tree::{collect_metadata_entries, sorted_dir_entries};
use super::GlobalFs;

impl GlobalFs {
    pub fn get_entry(&self, path: &VirtualPath) -> Option<&FsEntry> {
        if path.is_root() {
            return Some(&self.root);
        }

        let mut current = &self.root;
        for part in path.segments() {
            current = match current {
                FsEntry::Directory { children, .. } => children.get(part)?,
                FsEntry::File { .. } => return None,
            };
        }
        Some(current)
    }

    pub fn exists(&self, path: &VirtualPath) -> bool {
        self.get_entry(path).is_some()
    }

    pub fn is_directory(&self, path: &VirtualPath) -> bool {
        matches!(self.get_entry(path), Some(FsEntry::Directory { .. }))
    }

    pub fn has_children(&self, path: &VirtualPath) -> bool {
        matches!(
            self.get_entry(path),
            Some(FsEntry::Directory { children, .. }) if !children.is_empty()
        )
    }

    pub fn child_count(&self, path: &VirtualPath) -> Option<usize> {
        match self.get_entry(path)? {
            FsEntry::Directory { children, .. } => Some(children.len()),
            FsEntry::File { .. } => None,
        }
    }

    pub fn child_names(&self, path: &VirtualPath) -> Option<Vec<String>> {
        match self.get_entry(path)? {
            FsEntry::Directory { children, .. } => {
                let mut names = children.keys().cloned().collect::<Vec<_>>();
                names.sort();
                Some(names)
            }
            FsEntry::File { .. } => None,
        }
    }

    pub fn list_dir(&self, path: &VirtualPath) -> Option<Vec<DirEntry>> {
        match self.get_entry(path)? {
            FsEntry::Directory { children, .. } => Some(sorted_dir_entries(path, children)),
            FsEntry::File { .. } => None,
        }
    }

    /// Iterate over `(path, &NodeMetadata)` for every node in the tree.
    /// Walks the canonical filesystem so it always reflects the live state.
    pub fn metadata_entries(&self) -> Vec<(VirtualPath, &NodeMetadata)> {
        let mut out = Vec::new();
        collect_metadata_entries(&VirtualPath::root(), &self.root, &mut out);
        out
    }
}
