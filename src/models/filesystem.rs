use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::config::HOME_DIR;

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
// Virtual Path Newtype
// =============================================================================

/// A validated, normalized virtual filesystem path.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VirtualPath(String);

impl VirtualPath {
    /// Create a new VirtualPath from an absolute path string.
    ///
    /// The path is normalized to remove `.` and `..` components.
    pub fn new(path: impl Into<String>) -> Self {
        let path = path.into();
        Self(Self::normalize(&path))
    }

    /// Create a path representing the root directory.
    pub fn root() -> Self {
        Self("/".to_string())
    }

    /// Create a path representing the home directory.
    pub fn home() -> Self {
        Self(HOME_DIR.to_string())
    }

    /// Get the path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolve a relative or absolute path from this path.
    ///
    /// Handles:
    /// - Absolute paths (`/foo/bar`)
    /// - Home directory expansion (`~` and `~/foo`)
    /// - Relative paths (`foo`, `./foo`, `../foo`)
    pub fn resolve(&self, path: &str) -> Self {
        let absolute = if path.starts_with('/') {
            path.to_string()
        } else if path == "~" {
            HOME_DIR.to_string()
        } else if let Some(rest) = path.strip_prefix("~/") {
            format!("{}/{}", HOME_DIR, rest)
        } else if path == ".." {
            self.parent()
                .map(|p| p.0)
                .unwrap_or_else(|| "/".to_string())
        } else if path == "." {
            self.0.clone()
        } else {
            format!("{}/{}", self.0.trim_end_matches('/'), path)
        };

        Self::new(absolute)
    }

    /// Join a path component to this path.
    #[allow(dead_code)]
    pub fn join(&self, component: &str) -> Self {
        Self::new(format!("{}/{}", self.0.trim_end_matches('/'), component))
    }

    /// Get the parent directory, if any.
    pub fn parent(&self) -> Option<Self> {
        if self.0 == "/" {
            return None;
        }

        let parts: Vec<&str> = self.0.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() <= 1 {
            Some(Self::root())
        } else {
            Some(Self::new(format!(
                "/{}",
                parts[..parts.len() - 1].join("/")
            )))
        }
    }

    /// Get the file/directory name (last component).
    #[allow(dead_code)]
    pub fn name(&self) -> Option<&str> {
        if self.0 == "/" {
            None
        } else {
            self.0.rsplit('/').next()
        }
    }

    /// Check if this path is the home directory.
    #[allow(dead_code)]
    pub fn is_home(&self) -> bool {
        self.0 == HOME_DIR
    }

    /// Format for display, replacing home directory with `~`.
    pub fn display(&self) -> String {
        let home_with_slash = format!("{}/", HOME_DIR);

        if self.0 == HOME_DIR {
            "~".to_string()
        } else if self.0.starts_with(&home_with_slash) {
            format!("~/{}", &self.0[home_with_slash.len()..])
        } else {
            self.0.clone()
        }
    }

    /// Normalize a path by resolving `.` and `..` components.
    fn normalize(path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/').filter(|s| !s.is_empty()) {
            match part {
                ".." => {
                    parts.pop();
                }
                "." => {}
                _ => parts.push(part),
            }
        }

        if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        }
    }
}

impl fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for VirtualPath {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<&str> for VirtualPath {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for VirtualPath {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for VirtualPath {
    fn as_ref(&self) -> &str {
        &self.0
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
    // VirtualPath Tests
    // =========================================================================

    #[test]
    fn test_virtual_path_new() {
        let path = VirtualPath::new("/home/wonjae");
        assert_eq!(path.as_str(), "/home/wonjae");
    }

    #[test]
    fn test_virtual_path_normalization() {
        assert_eq!(VirtualPath::new("/home/./wonjae").as_str(), "/home/wonjae");
        assert_eq!(
            VirtualPath::new("/home/wonjae/../etc").as_str(),
            "/home/etc"
        );
        assert_eq!(VirtualPath::new("/a/b/c/../../d").as_str(), "/a/d");
        assert_eq!(VirtualPath::new("/").as_str(), "/");
        assert_eq!(VirtualPath::new("/../..").as_str(), "/");
    }

    #[test]
    fn test_virtual_path_root_and_home() {
        assert_eq!(VirtualPath::root().as_str(), "/");
        assert_eq!(VirtualPath::home().as_str(), HOME_DIR);
    }

    #[test]
    fn test_virtual_path_resolve_absolute() {
        let path = VirtualPath::new("/home/wonjae");
        let resolved = path.resolve("/etc/config");
        assert_eq!(resolved.as_str(), "/etc/config");
    }

    #[test]
    fn test_virtual_path_resolve_relative() {
        let path = VirtualPath::new("/home/wonjae");
        assert_eq!(path.resolve("projects").as_str(), "/home/wonjae/projects");
        assert_eq!(path.resolve("./blog").as_str(), "/home/wonjae/blog");
    }

    #[test]
    fn test_virtual_path_resolve_parent() {
        let path = VirtualPath::new("/home/wonjae/projects");
        assert_eq!(path.resolve("..").as_str(), "/home/wonjae");

        let root = VirtualPath::root();
        assert_eq!(root.resolve("..").as_str(), "/");
    }

    #[test]
    fn test_virtual_path_resolve_home() {
        let path = VirtualPath::new("/etc");
        assert_eq!(path.resolve("~").as_str(), HOME_DIR);
        assert_eq!(
            path.resolve("~/blog").as_str(),
            format!("{}/blog", HOME_DIR)
        );
    }

    #[test]
    fn test_virtual_path_join() {
        let path = VirtualPath::new("/home/wonjae");
        let joined = path.join("projects");
        assert_eq!(joined.as_str(), "/home/wonjae/projects");
    }

    #[test]
    fn test_virtual_path_parent() {
        let path = VirtualPath::new("/home/wonjae/projects");
        assert_eq!(path.parent().unwrap().as_str(), "/home/wonjae");

        let home = VirtualPath::new("/home");
        assert_eq!(home.parent().unwrap().as_str(), "/");

        let root = VirtualPath::root();
        assert!(root.parent().is_none());
    }

    #[test]
    fn test_virtual_path_name() {
        let path = VirtualPath::new("/home/wonjae/projects");
        assert_eq!(path.name(), Some("projects"));

        let root = VirtualPath::root();
        assert_eq!(root.name(), None);
    }

    #[test]
    fn test_virtual_path_display() {
        assert_eq!(VirtualPath::new(HOME_DIR).display(), "~");
        assert_eq!(
            VirtualPath::new(format!("{}/projects", HOME_DIR)).display(),
            "~/projects"
        );
        assert_eq!(VirtualPath::new("/etc").display(), "/etc");
    }

    #[test]
    fn test_virtual_path_deref() {
        let path = VirtualPath::new("/home/wonjae");
        // Deref allows &str methods
        assert!(path.starts_with("/home"));
        assert!(path.ends_with("wonjae"));
    }

    #[test]
    fn test_virtual_path_from_string() {
        let path: VirtualPath = "/home/wonjae".into();
        assert_eq!(path.as_str(), "/home/wonjae");

        let path: VirtualPath = String::from("/etc").into();
        assert_eq!(path.as_str(), "/etc");
    }

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
    /// Create a directory with default metadata.
    pub fn dir(entries: Vec<(&str, FsEntry)>) -> Self {
        FsEntry::Directory {
            children: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            description: String::new(),
            meta: FileMetadata::default(),
        }
    }

    /// Create a directory with description and metadata.
    #[allow(dead_code)]
    pub fn dir_with_meta(
        entries: Vec<(&str, FsEntry)>,
        description: &str,
        meta: FileMetadata,
    ) -> Self {
        FsEntry::Directory {
            children: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            description: description.to_string(),
            meta,
        }
    }

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
