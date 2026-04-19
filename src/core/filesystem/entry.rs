//! Directory entry types and path utilities.

use crate::models::FileMetadata;

/// Directory entry returned by list_dir.
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub title: String,
    pub file_meta: Option<FileMetadata>,
}

impl DirEntry {
    /// Create a new directory entry.
    pub fn new(name: String, is_dir: bool, title: String, file_meta: Option<FileMetadata>) -> Self {
        Self {
            name,
            is_dir,
            title,
            file_meta,
        }
    }

    /// Create a directory entry.
    pub fn directory(name: String, title: String) -> Self {
        Self::new(name, true, title, None)
    }

    /// Create a file entry.
    pub fn file(name: String, title: String, meta: Option<FileMetadata>) -> Self {
        Self::new(name, false, title, meta)
    }
}

/// Sort entries: directories first, then files, hidden last.
pub fn sort_entries(entries: &mut [DirEntry]) {
    entries.sort_by(|a, b| {
        let a_hidden = a.name.starts_with('.');
        let b_hidden = b.name.starts_with('.');

        match (a.is_dir, b.is_dir, a_hidden, b_hidden) {
            (true, false, _, _) => std::cmp::Ordering::Less,
            (false, true, _, _) => std::cmp::Ordering::Greater,
            (_, _, false, true) => std::cmp::Ordering::Less,
            (_, _, true, false) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });
}
