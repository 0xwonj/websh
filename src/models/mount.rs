//! Mount system for virtual filesystem backends.
//!
//! Provides a flexible mount system that supports multiple storage backends
//! (GitHub, IPFS, ENS) with configurable aliases for URL routing.

use std::collections::HashMap;

use super::storage::Storage;

// ============================================================================
// Mount
// ============================================================================

/// A mounted filesystem with alias and storage backend.
///
/// Mount combines a URL alias (for routing) with a storage backend
/// (for content access) and optional content prefix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// URL alias (e.g., "~", "work")
    pub alias: String,
    /// Storage backend
    pub storage: Storage,
    /// Optional prefix for content paths (e.g., "~" if content is in ~/*)
    pub content_prefix: Option<String>,
}

impl Mount {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a new mount with the given alias and storage.
    pub fn new(alias: impl Into<String>, storage: Storage) -> Self {
        Self {
            alias: alias.into(),
            storage,
            content_prefix: None,
        }
    }

    /// Create a new mount with content prefix.
    pub fn with_prefix(
        alias: impl Into<String>,
        storage: Storage,
        prefix: impl Into<String>,
    ) -> Self {
        Self {
            alias: alias.into(),
            storage,
            content_prefix: Some(prefix.into()),
        }
    }

    /// Create a GitHub mount (convenience method).
    #[cfg(test)]
    pub fn github(alias: impl Into<String>, owner: &str, repo: &str, branch: &str) -> Self {
        Self::new(alias, Storage::github(owner, repo, branch))
    }

    /// Create a GitHub mount with content prefix (convenience method).
    pub fn github_with_prefix(
        alias: impl Into<String>,
        owner: &str,
        repo: &str,
        branch: &str,
        prefix: impl Into<String>,
    ) -> Self {
        Self::with_prefix(alias, Storage::github(owner, repo, branch), prefix)
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Get the alias for URL path segment.
    #[inline]
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Get the storage backend.
    #[inline]
    pub fn storage(&self) -> &Storage {
        &self.storage
    }

    // ========================================================================
    // URL Generation
    // ========================================================================

    /// Get base URL for raw content (without content prefix).
    pub fn base_url(&self) -> String {
        self.storage.raw_base_url()
    }

    /// Get content URL base (includes content_prefix if set).
    pub fn content_base_url(&self) -> String {
        match &self.content_prefix {
            Some(prefix) => format!("{}/{}", self.storage.raw_base_url(), prefix),
            None => self.storage.raw_base_url(),
        }
    }

    /// Get the manifest URL for this mount.
    pub fn manifest_url(&self) -> String {
        self.storage.manifest_url()
    }

    /// Get a short description of this mount's backend type.
    pub fn description(&self) -> String {
        self.storage.description()
    }

    // ========================================================================
    // Backend Properties
    // ========================================================================

    /// Check if this mount supports writing.
    pub fn is_writable(&self) -> bool {
        self.storage.is_writable()
    }
}

// ============================================================================
// MountRegistry
// ============================================================================

/// Registry of mounted filesystems.
///
/// Manages multiple mounts and provides lookup by alias.
/// The first mount with alias "~" is considered the home/default mount.
#[derive(Clone, Debug, Default)]
pub struct MountRegistry {
    /// All registered mounts, keyed by alias
    mounts: HashMap<String, Mount>,
    /// Order of mount aliases (for iteration)
    order: Vec<String>,
}

impl MountRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            mounts: HashMap::new(),
            order: Vec::new(),
        }
    }

    /// Create a registry from a list of mounts.
    pub fn from_mounts(mounts: Vec<Mount>) -> Self {
        let mut registry = Self::new();
        for mount in mounts {
            registry.register(mount);
        }
        registry
    }

    /// Register a mount.
    ///
    /// If a mount with the same alias already exists, it will be replaced.
    fn register(&mut self, mount: Mount) {
        let alias = mount.alias().to_string();
        if !self.mounts.contains_key(&alias) {
            self.order.push(alias.clone());
        }
        self.mounts.insert(alias, mount);
    }

    /// Get a mount by alias.
    pub fn get(&self, alias: &str) -> Option<&Mount> {
        self.mounts.get(alias)
    }

    /// Get all registered mounts in registration order.
    pub fn all(&self) -> impl Iterator<Item = &Mount> {
        self.order.iter().filter_map(|alias| self.mounts.get(alias))
    }

    /// Get the first (home) mount.
    pub fn first(&self) -> Option<&Mount> {
        self.order.first().and_then(|alias| self.mounts.get(alias))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_mount() {
        let mount = Mount::github("~", "user", "repo", "main");
        assert_eq!(mount.alias(), "~");
        assert_eq!(
            mount.base_url(),
            "https://raw.githubusercontent.com/user/repo/main"
        );
        assert!(mount.is_writable());
    }

    #[test]
    fn test_github_mount_with_prefix() {
        let mount = Mount::github_with_prefix("~", "user", "repo", "main", "~");
        assert_eq!(mount.alias(), "~");
        assert_eq!(
            mount.base_url(),
            "https://raw.githubusercontent.com/user/repo/main"
        );
        assert_eq!(
            mount.content_base_url(),
            "https://raw.githubusercontent.com/user/repo/main/~"
        );
    }

    #[test]
    fn test_ipfs_mount() {
        let mount = Mount::new("data", Storage::ipfs("QmXyz123"));
        assert_eq!(mount.alias(), "data");
        assert_eq!(mount.base_url(), "https://ipfs.io/ipfs/QmXyz123");
        assert!(!mount.is_writable());
    }

    #[test]
    fn test_ens_mount() {
        let mount = Mount::new("vitalik", Storage::ens("vitalik.eth"));
        assert_eq!(mount.alias(), "vitalik");
        assert_eq!(mount.base_url(), "https://vitalik.eth.limo");
        assert!(!mount.is_writable());
    }

    #[test]
    fn test_registry_from_mounts() {
        let mounts = vec![
            Mount::github("~", "user", "repo", "main"),
            Mount::new("data", Storage::ipfs("QmXyz")),
        ];
        let registry = MountRegistry::from_mounts(mounts);

        assert_eq!(registry.all().count(), 2);
        assert!(registry.get("~").is_some());
        assert!(registry.get("data").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_first() {
        let mounts = vec![
            Mount::github("~", "user", "repo", "main"),
            Mount::new("data", Storage::ipfs("QmXyz")),
        ];
        let registry = MountRegistry::from_mounts(mounts);

        let first = registry.first().unwrap();
        assert_eq!(first.alias(), "~");
    }

    #[test]
    fn test_manifest_url() {
        let mount = Mount::github("~", "user", "repo", "main");
        assert_eq!(
            mount.manifest_url(),
            "https://raw.githubusercontent.com/user/repo/main/manifest.json"
        );
    }
}
