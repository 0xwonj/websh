use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// =============================================================================
// File Metadata
// =============================================================================

/// Metadata for files and directories.
///
/// Note: `FileMetadata` is never serialized standalone — only `FileEntry` and
/// `Manifest` hit the wire. That lets us add new fields (like `tags` below)
/// without needing `#[serde(default)]` guards.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes (None for directories or unknown)
    pub size: Option<u64>,
    /// Last modification time as Unix timestamp
    pub modified: Option<u64>,
    /// Tags for categorization (mirrors `FileEntry.tags` — needed to round-trip
    /// tags through `VirtualFs` back into a `Manifest`).
    pub tags: Vec<String>,
    /// Access filter (None = publicly readable)
    pub access: Option<AccessFilter>,
}

impl FileMetadata {
    /// Check if this file is access-restricted.
    pub fn is_restricted(&self) -> bool {
        self.access.is_some()
    }
}

/// Metadata for directories (from .meta.json).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DirectoryMetadata {
    /// Display title
    pub title: String,
    /// Longer description text
    pub description: Option<String>,
    /// Icon identifier (e.g., "folder-code")
    pub icon: Option<String>,
    /// Thumbnail image path
    pub thumbnail: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
}

/// Access-control metadata for a file.
///
/// "Access" is advisory — it filters who the UI shows content to. Actual
/// cryptographic confidentiality is NOT provided in Phase 3/4 Option B.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessFilter {
    /// Wallet addresses listed as recipients.
    pub recipients: Vec<Recipient>,
}

/// A single listed recipient.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recipient {
    /// Wallet address (checksum or lowercase).
    pub address: String,
}

// =============================================================================
// Display Permissions
// =============================================================================

/// Unix-style permission display (computed at runtime).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayPermissions {
    /// Is this a directory?
    pub is_dir: bool,
    /// Read permission (based on access filter)
    pub read: bool,
    /// Write permission (based on admin/mount status)
    pub write: bool,
    /// Execute permission
    pub execute: bool,
}

impl fmt::Display for DisplayPermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            if self.is_dir { 'd' } else { '-' },
            if self.read { 'r' } else { '-' },
            if self.write { 'w' } else { '-' },
            if self.execute { 'x' } else { '-' },
        )
    }
}

// =============================================================================
// Manifest Types
// =============================================================================

/// Root manifest structure from manifest.json
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Manifest {
    /// File entries
    pub files: Vec<FileEntry>,
    /// Directory metadata entries
    pub directories: Vec<DirectoryEntry>,
}

/// File entry from manifest.json
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileEntry {
    /// File path (relative to content root)
    pub path: String,
    /// Display title/description
    pub title: String,
    /// File size in bytes
    pub size: Option<u64>,
    /// Last modification time (Unix timestamp)
    pub modified: Option<u64>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Access filter (None = publicly readable)
    pub access: Option<AccessFilter>,
}

impl FileEntry {
    /// Convert to FileMetadata.
    pub fn to_metadata(&self) -> FileMetadata {
        FileMetadata {
            size: self.size,
            modified: self.modified,
            tags: self.tags.clone(),
            access: self.access.clone(),
        }
    }

    /// Convert a runtime `FsEntry` back to a manifest `FileEntry`.
    ///
    /// Notes:
    /// - The manifest `path` is the *content path* (what the manifest
    ///   originally carried), not the VFS absolute path. We mirror
    ///   `VirtualFs::from_manifest`, which stores `file.path` into
    ///   `FsEntry::File.content_path`. If that's missing (synthetic file with
    ///   no manifest origin — e.g. `.profile`), callers are expected to
    ///   filter the entry out before calling this; we fall back to the VFS
    ///   path as a defensive default rather than panicking.
    /// - `title` reverses `from_manifest`'s mapping: it goes into
    ///   `FsEntry::File.description` on load, so we read it back from there.
    ///
    /// Panics if `entry` is a directory.
    pub fn from_fs(path: &crate::models::VirtualPath, entry: &FsEntry) -> FileEntry {
        match entry {
            FsEntry::File {
                content_path,
                description,
                meta,
            } => FileEntry {
                path: content_path
                    .clone()
                    .unwrap_or_else(|| path.as_str().trim_start_matches('/').to_string()),
                title: description.clone(),
                size: meta.size,
                modified: meta.modified,
                tags: meta.tags.clone(),
                access: meta.access.clone(),
            },
            FsEntry::Directory { .. } => panic!("FileEntry::from_fs called on directory"),
        }
    }
}

/// Directory metadata entry from manifest.json
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DirectoryEntry {
    /// Directory path (relative to content root, empty string for root)
    pub path: String,
    /// Display title
    pub title: String,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Longer description text
    pub description: Option<String>,
    /// Icon identifier (e.g., "folder-code", "folder-images")
    pub icon: Option<String>,
    /// Thumbnail image path (relative to content root)
    pub thumbnail: Option<String>,
}

impl DirectoryEntry {
    /// Convert directory metadata back to a manifest `DirectoryEntry`.
    ///
    /// `path` is the relative path (empty string for root).
    pub fn from_meta(path: String, meta: &DirectoryMetadata) -> DirectoryEntry {
        DirectoryEntry {
            path,
            title: meta.title.clone(),
            tags: meta.tags.clone(),
            description: meta.description.clone(),
            icon: meta.icon.clone(),
            thumbnail: meta.thumbnail.clone(),
        }
    }
}

/// Supported file types for the reader
#[derive(Clone, Debug, PartialEq)]
pub enum FileType {
    Markdown,
    Pdf,
    Image,
    Link,
    Unknown,
}

impl FileType {
    /// Detect file type from path extension
    pub fn from_path(path: &str) -> Self {
        match path.rsplit('.').next().map(|s| s.to_lowercase()).as_deref() {
            Some("md") => Self::Markdown,
            Some("pdf") => Self::Pdf,
            Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => Self::Image,
            Some("link") => Self::Link,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // FileType Tests
    // =========================================================================

    #[test]
    fn test_file_type_detection() {
        assert_eq!(FileType::from_path("blog/hello.md"), FileType::Markdown);
        assert_eq!(FileType::from_path("papers/research.pdf"), FileType::Pdf);
        assert_eq!(FileType::from_path("images/photo.png"), FileType::Image);
        assert_eq!(FileType::from_path("images/photo.JPG"), FileType::Image);
        assert_eq!(FileType::from_path("links/github.link"), FileType::Link);
        assert_eq!(FileType::from_path("unknown/file.xyz"), FileType::Unknown);
    }
}

/// Represents an entry in the virtual filesystem
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum FsEntry {
    Directory {
        children: HashMap<String, FsEntry>,
        meta: DirectoryMetadata,
    },
    File {
        content_path: Option<String>,
        description: String,
        meta: FileMetadata,
    },
}

impl FsEntry {
    /// Create a file without content path (static file).
    pub fn file(description: &str) -> Self {
        FsEntry::File {
            content_path: None,
            description: description.to_string(),
            meta: FileMetadata::default(),
        }
    }

    /// Create a file with full metadata.
    pub fn content_file_with_meta(path: &str, description: &str, meta: FileMetadata) -> Self {
        FsEntry::File {
            content_path: Some(path.to_string()),
            description: description.to_string(),
            meta,
        }
    }

    /// Check if this entry is a directory.
    pub fn is_directory(&self) -> bool {
        matches!(self, FsEntry::Directory { .. })
    }

    /// Check if this file is access-restricted.
    pub fn is_restricted(&self) -> bool {
        match self {
            FsEntry::File { meta, .. } => meta.is_restricted(),
            FsEntry::Directory { .. } => false,
        }
    }

    /// Get the file metadata (files only).
    #[allow(dead_code)]
    pub fn file_meta(&self) -> Option<&FileMetadata> {
        match self {
            FsEntry::File { meta, .. } => Some(meta),
            FsEntry::Directory { .. } => None,
        }
    }

    /// Get the directory metadata (directories only).
    #[allow(dead_code)]
    pub fn dir_meta(&self) -> Option<&DirectoryMetadata> {
        match self {
            FsEntry::Directory { meta, .. } => Some(meta),
            FsEntry::File { .. } => None,
        }
    }
}
