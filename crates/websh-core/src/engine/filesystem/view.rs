use crate::domain::{
    DirEntry, DisplayPermissions, FsEntry, NodeMetadata, VirtualPath, WalletState,
    is_runtime_overlay_path, runtime_state_root,
};

use super::GlobalFs;
use super::tree::sort_dir_entries;

/// Borrowed read-only view of content with an optional runtime-state projection.
/// Runtime files own their reserved subtree; only ancestor listings are merged.
#[derive(Clone, Copy)]
pub struct FsView<'a> {
    content: &'a GlobalFs,
    runtime: Option<&'a GlobalFs>,
}

impl<'a> FsView<'a> {
    pub fn content(content: &'a GlobalFs) -> Self {
        Self {
            content,
            runtime: None,
        }
    }

    pub fn with_runtime(content: &'a GlobalFs, runtime: &'a GlobalFs) -> Self {
        Self {
            content,
            runtime: Some(runtime),
        }
    }

    fn entry(&self, path: &VirtualPath) -> Option<&'a FsEntry> {
        if let Some(runtime) = self.runtime {
            if is_runtime_overlay_path(path) {
                return runtime.get_entry(path);
            }
            if runtime_state_root().starts_with(path)
                && !self.content.is_directory(path)
                && let Some(entry) = runtime.get_entry(path)
            {
                return Some(entry);
            }
        }
        self.content.get_entry(path)
    }

    pub fn exists(&self, path: &VirtualPath) -> bool {
        self.entry(path).is_some()
    }

    pub fn is_directory(&self, path: &VirtualPath) -> bool {
        self.entry(path).is_some_and(FsEntry::is_directory)
    }

    pub fn node_metadata(&self, path: &VirtualPath) -> Option<&'a NodeMetadata> {
        self.entry(path).map(FsEntry::meta)
    }

    pub fn child_count(&self, path: &VirtualPath) -> Option<usize> {
        if self.runtime.is_some()
            && runtime_state_root().starts_with(path)
            && !is_runtime_overlay_path(path)
        {
            return self.list_dir(path).map(|entries| entries.len());
        }
        match self.entry(path)? {
            FsEntry::Directory { children, .. } => Some(children.len()),
            FsEntry::File { .. } => None,
        }
    }

    pub fn list_dir(&self, path: &VirtualPath) -> Option<Vec<DirEntry>> {
        let Some(runtime) = self.runtime else {
            return self.content.list_dir(path);
        };
        if is_runtime_overlay_path(path) {
            return runtime.list_dir(path);
        }
        if !runtime_state_root().starts_with(path) {
            return self.content.list_dir(path);
        }

        let Some(mut entries) = self.content.list_dir(path) else {
            return runtime.list_dir(path);
        };
        entries.retain(|entry| !is_runtime_overlay_path(&entry.path));
        for entry in runtime.list_dir(path).unwrap_or_default() {
            match entries
                .iter_mut()
                .find(|existing| existing.path == entry.path)
            {
                Some(existing) if existing.is_dir && !is_runtime_overlay_path(&entry.path) => {}
                Some(existing) => *existing = entry,
                None => entries.push(entry),
            }
        }
        sort_dir_entries(&mut entries);
        Some(entries)
    }

    pub fn permissions(
        &self,
        path: &VirtualPath,
        wallet: &WalletState,
    ) -> Option<DisplayPermissions> {
        self.entry(path).map(|entry| entry.permissions(wallet))
    }
}
