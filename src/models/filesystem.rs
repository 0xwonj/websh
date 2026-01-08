use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// =============================================================================
// File Metadata
// =============================================================================

/// Metadata for files and directories.
#[derive(Clone, Debug, Default)]
pub struct FileMetadata {
    /// File size in bytes (None for directories or unknown)
    pub size: Option<u64>,
    /// Creation time as Unix timestamp (reserved for future use)
    #[allow(dead_code)]
    pub created: Option<u64>,
    /// Last modification time as Unix timestamp
    pub modified: Option<u64>,
    /// Encryption details (None = unencrypted)
    pub encryption: Option<EncryptionInfo>,
}

impl FileMetadata {
    /// Check if this file is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.encryption.is_some()
    }
}

/// Encryption information for access control.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EncryptionInfo {
    /// Encryption algorithm (e.g., "AES-256-GCM")
    pub algorithm: String,
    /// Wrapped symmetric keys for each authorized recipient
    pub wrapped_keys: Vec<WrappedKey>,
}

/// A symmetric key wrapped with a recipient's public key.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WrappedKey {
    /// Recipient identifier (wallet address or public key)
    pub recipient: String,
    /// Symmetric key encrypted with recipient's public key (base64)
    pub encrypted_key: String,
}

// =============================================================================
// Display Permissions
// =============================================================================

/// Unix-style permission display (computed at runtime).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayPermissions {
    /// Is this a directory?
    pub is_dir: bool,
    /// Read permission (based on encryption status)
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
// Manifest Entry
// =============================================================================

/// Entry from manifest.json (external content repository)
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ManifestEntry {
    /// File path (relative to content root)
    pub path: String,
    /// Display title/description
    pub title: String,
    /// File size in bytes
    pub size: Option<u64>,
    /// Creation time (Unix timestamp)
    pub created: Option<u64>,
    /// Last modification time (Unix timestamp)
    pub modified: Option<u64>,
    /// Encryption details (None = unencrypted)
    pub encryption: Option<EncryptionInfo>,
}

impl ManifestEntry {
    /// Convert to FileMetadata
    pub fn to_metadata(&self) -> FileMetadata {
        FileMetadata {
            size: self.size,
            created: self.created,
            modified: self.modified,
            encryption: self.encryption.clone(),
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
        description: String,
        meta: FileMetadata,
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

    /// Check if this file is encrypted.
    pub fn is_encrypted(&self) -> bool {
        match self {
            FsEntry::File { meta, .. } => meta.is_encrypted(),
            FsEntry::Directory { .. } => false,
        }
    }

    /// Get the description of this entry.
    #[allow(dead_code)]
    pub fn description(&self) -> &str {
        match self {
            FsEntry::Directory { description, .. } => description,
            FsEntry::File { description, .. } => description,
        }
    }

    /// Get the metadata of this entry.
    #[allow(dead_code)]
    pub fn meta(&self) -> &FileMetadata {
        match self {
            FsEntry::Directory { meta, .. } => meta,
            FsEntry::File { meta, .. } => meta,
        }
    }
}
