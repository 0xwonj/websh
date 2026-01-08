//! Mount system for virtual filesystem backends.
//!
//! Provides a flexible mount system that supports multiple storage backends
//! (GitHub, IPFS, ENS) with configurable aliases for URL routing.

use std::collections::HashMap;

// ============================================================================
// Mount Types
// ============================================================================

/// Storage backend type for a mount.
///
/// Each variant represents a different way to fetch content.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Mount {
    /// GitHub raw content
    GitHub {
        /// URL alias (e.g., "~", "work")
        alias: String,
        /// Base URL for content fetching
        base_url: String,
    },

    /// IPFS gateway
    Ipfs {
        /// URL alias
        alias: String,
        /// Content identifier (CID)
        cid: String,
        /// Optional custom gateway URL
        gateway: Option<String>,
    },

    /// ENS contenthash
    Ens {
        /// URL alias
        alias: String,
        /// ENS name (e.g., "vitalik.eth")
        name: String,
    },
}

impl Mount {
    /// Create a new GitHub mount.
    pub fn github(alias: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self::GitHub {
            alias: alias.into(),
            base_url: base_url.into(),
        }
    }

    /// Create a new IPFS mount.
    #[cfg(test)]
    pub fn ipfs(alias: impl Into<String>, cid: impl Into<String>) -> Self {
        Self::Ipfs {
            alias: alias.into(),
            cid: cid.into(),
            gateway: None,
        }
    }

    /// Create a new IPFS mount with custom gateway.
    #[cfg(test)]
    pub fn ipfs_with_gateway(
        alias: impl Into<String>,
        cid: impl Into<String>,
        gateway: impl Into<String>,
    ) -> Self {
        Self::Ipfs {
            alias: alias.into(),
            cid: cid.into(),
            gateway: Some(gateway.into()),
        }
    }

    /// Create a new ENS mount.
    #[cfg(test)]
    pub fn ens(alias: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Ens {
            alias: alias.into(),
            name: name.into(),
        }
    }

    /// Get the alias for URL path segment.
    #[inline]
    pub fn alias(&self) -> &str {
        match self {
            Self::GitHub { alias, .. } => alias,
            Self::Ipfs { alias, .. } => alias,
            Self::Ens { alias, .. } => alias,
        }
    }

    /// Get base URL for content fetching.
    pub fn base_url(&self) -> String {
        match self {
            Self::GitHub { base_url, .. } => base_url.clone(),
            Self::Ipfs { cid, gateway, .. } => {
                let gw = gateway.as_deref().unwrap_or("https://ipfs.io");
                format!("{}/ipfs/{}", gw, cid)
            }
            Self::Ens { name, .. } => {
                // ENS resolution via eth.limo gateway
                format!("https://{}.limo", name)
            }
        }
    }

    /// Get the manifest URL for this mount.
    pub fn manifest_url(&self) -> String {
        format!("{}/manifest.json", self.base_url())
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

    /// Get all registered mounts in registration order.
    pub fn all(&self) -> impl Iterator<Item = &Mount> {
        self.order.iter().filter_map(|alias| self.mounts.get(alias))
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
        let mount = Mount::github("~", "https://raw.githubusercontent.com/user/repo/main");
        assert_eq!(mount.alias(), "~");
        assert_eq!(
            mount.base_url(),
            "https://raw.githubusercontent.com/user/repo/main"
        );
    }

    #[test]
    fn test_ipfs_mount() {
        let mount = Mount::ipfs("data", "QmXyz123");
        assert_eq!(mount.alias(), "data");
        assert_eq!(mount.base_url(), "https://ipfs.io/ipfs/QmXyz123");
    }

    #[test]
    fn test_ipfs_mount_custom_gateway() {
        let mount = Mount::ipfs_with_gateway("data", "QmXyz123", "https://cloudflare-ipfs.com");
        assert_eq!(
            mount.base_url(),
            "https://cloudflare-ipfs.com/ipfs/QmXyz123"
        );
    }

    #[test]
    fn test_ens_mount() {
        let mount = Mount::ens("vitalik", "vitalik.eth");
        assert_eq!(mount.alias(), "vitalik");
        assert_eq!(mount.base_url(), "https://vitalik.eth.limo");
    }

    #[test]
    fn test_registry_from_mounts() {
        let mounts = vec![
            Mount::github("~", "https://example.com"),
            Mount::ipfs("data", "QmXyz"),
        ];
        let registry = MountRegistry::from_mounts(mounts);

        assert_eq!(registry.all().count(), 2);
    }

    #[test]
    fn test_manifest_url() {
        let mount = Mount::github("~", "https://raw.githubusercontent.com/user/repo/main");
        assert_eq!(
            mount.manifest_url(),
            "https://raw.githubusercontent.com/user/repo/main/manifest.json"
        );
    }
}
