use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// =============================================================================
// File Metadata
// =============================================================================

/// Metadata for files and directories.
///
/// Note: `FileMetadata` is never serialized standalone. Backend adapters
/// decide how these fields map onto their private on-wire formats.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes (None for directories or unknown)
    pub size: Option<u64>,
    /// Last modification time as Unix timestamp
    pub modified: Option<u64>,
    /// Tags for categorization.
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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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
/// "Access" is advisory: it filters who the UI shows content to. Actual
/// cryptographic confidentiality is not provided by this metadata field.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccessFilter {
    /// Wallet addresses listed as recipients.
    pub recipients: Vec<Recipient>,
}

/// A single listed recipient.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
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

/// Directory entry returned by canonical filesystem directory listings.
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub path: crate::models::VirtualPath,
    pub is_dir: bool,
    pub title: String,
    pub file_meta: Option<FileMetadata>,
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

/// Supported file types for the reader
#[derive(Clone, Debug, PartialEq)]
pub enum FileType {
    Html,
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
            Some("html" | "htm") => Self::Html,
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
        assert_eq!(FileType::from_path("index.html"), FileType::Html);
        assert_eq!(FileType::from_path("blog/hello.md"), FileType::Markdown);
        assert_eq!(FileType::from_path("papers/research.pdf"), FileType::Pdf);
        assert_eq!(FileType::from_path("images/photo.png"), FileType::Image);
        assert_eq!(FileType::from_path("images/photo.JPG"), FileType::Image);
        assert_eq!(FileType::from_path("links/github.link"), FileType::Link);
        assert_eq!(FileType::from_path("unknown/file.xyz"), FileType::Unknown);
    }
}

/// Represents an entry in the canonical filesystem tree.
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
