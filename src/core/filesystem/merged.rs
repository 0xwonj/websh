//! Merged filesystem view combining VirtualFs with pending changes.

use std::collections::HashMap;

use crate::models::{DirectoryMetadata, FileMetadata, FsEntry};

use super::VirtualFs;
use super::entry::{DirEntry, sort_entries};
use crate::core::storage::{ChangeType, PendingChanges};

/// Merged filesystem view.
///
/// Combines an immutable base filesystem with pending changes overlay.
/// This is a computed snapshot managed by `FsState` with automatic memoization.
///
/// Access via `ctx.fs.get()` which returns the memoized merged view.
#[derive(Clone)]
pub struct MergedFs {
    base: VirtualFs,
    pending: PendingChanges,
}

impl MergedFs {
    /// Create merged view from base filesystem and pending changes.
    pub fn new(base: VirtualFs, pending: PendingChanges) -> Self {
        Self { base, pending }
    }

    /// Get reference to base filesystem.
    pub fn base(&self) -> &VirtualFs {
        &self.base
    }

    /// Get reference to pending changes.
    pub fn pending(&self) -> &PendingChanges {
        &self.pending
    }

    /// Check if there are pending changes.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Get entry with overlay applied.
    pub fn get_entry(&self, path: &str) -> Option<FsEntry> {
        if self.pending.is_deleted(path) {
            return None;
        }

        if let Some(change) = self.pending.get(path) {
            return change_to_entry(change, &self.base);
        }

        self.base.get_entry(path).cloned()
    }

    /// List directory with overlay applied.
    pub fn list_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        if self.pending.is_deleted(path) {
            return None;
        }

        let mut entries: HashMap<String, DirEntry> = self
            .base
            .list_dir(path)
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.name.clone(), e))
            .collect();

        let prefix = if path.is_empty() {
            String::new()
        } else {
            format!("{}/", path)
        };

        for change in self.pending.iter() {
            let child_name = extract_child_name(&change.path, path, &prefix);

            if let Some(name) = child_name {
                apply_change_to_entries(&mut entries, name, &change.change_type);
            }
        }

        let mut result: Vec<_> = entries.into_values().collect();
        sort_entries(&mut result);
        Some(result)
    }

    /// Check if path exists.
    pub fn exists(&self, path: &str) -> bool {
        if self.pending.is_deleted(path) {
            return false;
        }
        if self.pending.has_change(path) {
            return true;
        }
        self.base.get_entry(path).is_some()
    }

    /// Check if path is a directory.
    pub fn is_directory(&self, path: &str) -> bool {
        self.get_entry(path).is_some_and(|e| e.is_directory())
    }

    /// Get pending content for a file.
    pub fn get_pending_content(&self, path: &str) -> Option<&str> {
        self.pending
            .get(path)
            .and_then(|change| match &change.change_type {
                ChangeType::CreateFile { content, .. } | ChangeType::UpdateFile { content, .. } => {
                    Some(content.as_str())
                }
                _ => None,
            })
    }

    /// Resolve path with existence check.
    pub fn resolve_path(&self, current: &str, path: &str) -> Option<String> {
        let resolved = VirtualFs::resolve_path_string(current, path);
        if self.exists(&resolved) {
            Some(resolved)
        } else {
            None
        }
    }

    /// Resolve path string without existence check (delegates to VirtualFs).
    pub fn resolve_path_string(current: &str, path: &str) -> String {
        VirtualFs::resolve_path_string(current, path)
    }

    /// Compute permissions for an entry (delegates to base VirtualFs).
    pub fn get_permissions(
        &self,
        entry: &FsEntry,
        wallet: &crate::models::WalletState,
    ) -> crate::models::DisplayPermissions {
        self.base.get_permissions(entry, wallet)
    }

    /// Get the content path for a file.
    pub fn get_file_content_path(&self, path: &str) -> Option<String> {
        match self.get_entry(path)? {
            FsEntry::File { content_path, .. } => content_path,
            _ => None,
        }
    }
}

/// Extract child name if the change path is a direct child of directory.
fn extract_child_name<'a>(change_path: &'a str, dir_path: &str, prefix: &str) -> Option<&'a str> {
    if dir_path.is_empty() {
        if !change_path.contains('/') {
            Some(change_path)
        } else {
            None
        }
    } else {
        change_path.strip_prefix(prefix).and_then(|rest| {
            if !rest.contains('/') {
                Some(rest)
            } else {
                None
            }
        })
    }
}

/// Apply a change to the entries map.
fn apply_change_to_entries(
    entries: &mut HashMap<String, DirEntry>,
    name: &str,
    change: &ChangeType,
) {
    match change {
        ChangeType::DeleteFile | ChangeType::DeleteDirectory => {
            entries.remove(name);
        }
        ChangeType::CreateFile {
            description, meta, ..
        } => {
            entries.insert(
                name.to_string(),
                DirEntry::file(name.to_string(), description.clone(), Some(meta.clone())),
            );
        }
        ChangeType::CreateDirectory { meta } => {
            entries.insert(
                name.to_string(),
                DirEntry::directory(name.to_string(), meta.title.clone()),
            );
        }
        ChangeType::UpdateFile { description, .. } => {
            if let Some(desc) = description
                && let Some(entry) = entries.get_mut(name)
            {
                entry.title = desc.clone();
            }
        }
        ChangeType::CreateBinaryFile {
            description, meta, ..
        } => {
            entries.insert(
                name.to_string(),
                DirEntry::file(name.to_string(), description.clone(), Some(meta.clone())),
            );
        }
    }
}

/// Convert a pending change to FsEntry.
fn change_to_entry(
    change: &crate::core::storage::PendingChange,
    base: &VirtualFs,
) -> Option<FsEntry> {
    match &change.change_type {
        ChangeType::CreateFile {
            description, meta, ..
        } => Some(FsEntry::File {
            content_path: Some(change.path.clone()),
            description: description.clone(),
            meta: meta.clone(),
        }),
        ChangeType::UpdateFile { description, .. } => base.get_entry(&change.path).map(|entry| {
            if let (
                FsEntry::File {
                    content_path,
                    meta,
                    description: _,
                },
                Some(new_desc),
            ) = (entry.clone(), description.as_ref())
            {
                FsEntry::File {
                    content_path,
                    description: new_desc.clone(),
                    meta,
                }
            } else {
                entry.clone()
            }
        }),
        ChangeType::CreateDirectory { meta } => Some(FsEntry::Directory {
            children: HashMap::new(),
            meta: meta.clone(),
        }),
        ChangeType::CreateBinaryFile {
            description, meta, ..
        } => Some(FsEntry::File {
            content_path: Some(change.path.clone()),
            description: description.clone(),
            meta: meta.clone(),
        }),
        ChangeType::DeleteFile | ChangeType::DeleteDirectory => None,
    }
}

/// Create a new file entry for staging.
pub fn create_file_entry(content: &str, description: &str) -> (ChangeType, FileMetadata) {
    let meta = FileMetadata {
        size: Some(content.len() as u64),
        modified: Some(crate::utils::current_timestamp()),
        encryption: None,
    };

    let change = ChangeType::CreateFile {
        content: content.to_string(),
        description: description.to_string(),
        meta: meta.clone(),
    };

    (change, meta)
}

/// Create a new directory entry for staging.
pub fn create_directory_entry(title: &str) -> ChangeType {
    ChangeType::CreateDirectory {
        meta: DirectoryMetadata {
            title: title.to_string(),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DirectoryEntry, FileEntry, Manifest};

    fn create_test_fs() -> VirtualFs {
        let manifest = Manifest {
            files: vec![
                FileEntry {
                    path: "blog/hello.md".to_string(),
                    title: "Hello World".to_string(),
                    size: Some(100),
                    modified: Some(1704153600),
                    tags: vec![],
                    encryption: None,
                },
                FileEntry {
                    path: "readme.md".to_string(),
                    title: "README".to_string(),
                    size: Some(50),
                    modified: None,
                    tags: vec![],
                    encryption: None,
                },
            ],
            directories: vec![DirectoryEntry {
                path: "blog".to_string(),
                title: "Blog".to_string(),
                tags: vec![],
                description: None,
                icon: None,
                thumbnail: None,
            }],
        };
        VirtualFs::from_manifest(&manifest)
    }

    #[test]
    fn test_merged_fs_no_changes() {
        let base = create_test_fs();
        let pending = PendingChanges::new();
        let merged = MergedFs::new(base, pending);

        assert!(!merged.has_pending());
        assert!(merged.exists("blog/hello.md"));
        assert!(merged.exists("readme.md"));
    }

    #[test]
    fn test_merged_fs_create_file() {
        let base = create_test_fs();
        let mut pending = PendingChanges::new();
        pending.add(
            "new.md".to_string(),
            ChangeType::CreateFile {
                content: "# New".to_string(),
                description: "New file".to_string(),
                meta: FileMetadata::default(),
            },
        );

        let merged = MergedFs::new(base, pending);

        assert!(merged.has_pending());
        assert!(merged.exists("new.md"));
        assert_eq!(merged.get_pending_content("new.md"), Some("# New"));
    }

    #[test]
    fn test_merged_fs_delete_file() {
        let base = create_test_fs();
        let mut pending = PendingChanges::new();
        pending.add("readme.md".to_string(), ChangeType::DeleteFile);

        let merged = MergedFs::new(base, pending);

        assert!(!merged.exists("readme.md"));
        assert!(merged.exists("blog/hello.md"));
    }

    #[test]
    fn test_merged_fs_list_dir() {
        let base = create_test_fs();
        let mut pending = PendingChanges::new();
        pending.add(
            "new.md".to_string(),
            ChangeType::CreateFile {
                content: "content".to_string(),
                description: "New".to_string(),
                meta: FileMetadata::default(),
            },
        );
        pending.add("readme.md".to_string(), ChangeType::DeleteFile);

        let merged = MergedFs::new(base, pending);
        let entries = merged.list_dir("").unwrap();
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();

        assert!(names.contains(&"new.md"));
        assert!(!names.contains(&"readme.md"));
        assert!(names.contains(&"blog"));
    }

    #[test]
    fn test_merged_fs_create_directory() {
        let base = create_test_fs();
        let mut pending = PendingChanges::new();
        pending.add(
            "projects".to_string(),
            ChangeType::CreateDirectory {
                meta: DirectoryMetadata {
                    title: "Projects".to_string(),
                    ..Default::default()
                },
            },
        );

        let merged = MergedFs::new(base, pending);

        assert!(merged.exists("projects"));
        assert!(merged.is_directory("projects"));
    }

    #[test]
    fn test_merged_fs_resolve_path() {
        let base = create_test_fs();
        let mut pending = PendingChanges::new();
        pending.add(
            "new.md".to_string(),
            ChangeType::CreateFile {
                content: "".to_string(),
                description: "New".to_string(),
                meta: FileMetadata::default(),
            },
        );

        let merged = MergedFs::new(base, pending);

        assert_eq!(
            merged.resolve_path("", "new.md"),
            Some("new.md".to_string())
        );
        assert_eq!(merged.resolve_path("", "blog"), Some("blog".to_string()));
        assert!(merged.resolve_path("", "nonexistent").is_none());
    }
}
