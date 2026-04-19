//! Storage backend types for virtual filesystem.
//!
//! Defines the storage backends (GitHub, IPFS, ENS) that provide
//! content for mounted filesystems.

// ============================================================================
// Storage Enum
// ============================================================================

/// Storage backend type with connection information.
///
/// Each variant contains the information needed to read from and
/// (for writable backends) write to the storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Storage {
    /// GitHub repository storage.
    ///
    /// Supports both reading (raw.githubusercontent.com) and writing (API).
    GitHub {
        /// Repository owner (username or organization)
        owner: String,
        /// Repository name
        repo: String,
        /// Branch name (e.g., "main")
        branch: String,
    },

    /// IPFS content-addressed storage.
    ///
    /// Read-only; content is fetched via IPFS gateway.
    Ipfs {
        /// Content identifier (CID)
        cid: String,
        /// Optional custom gateway URL (default: https://ipfs.io)
        gateway: Option<String>,
    },

    /// ENS (Ethereum Name Service) contenthash.
    ///
    /// Read-only; resolved via eth.limo gateway.
    Ens {
        /// ENS name (e.g., "vitalik.eth")
        name: String,
    },
}

impl Storage {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a GitHub storage backend.
    pub fn github(
        owner: impl Into<String>,
        repo: impl Into<String>,
        branch: impl Into<String>,
    ) -> Self {
        Self::GitHub {
            owner: owner.into(),
            repo: repo.into(),
            branch: branch.into(),
        }
    }

    /// Create an IPFS storage backend with default gateway.
    #[cfg(test)]
    pub fn ipfs(cid: impl Into<String>) -> Self {
        Self::Ipfs {
            cid: cid.into(),
            gateway: None,
        }
    }

    /// Create an IPFS storage backend with custom gateway.
    #[cfg(test)]
    pub fn ipfs_with_gateway(cid: impl Into<String>, gateway: impl Into<String>) -> Self {
        Self::Ipfs {
            cid: cid.into(),
            gateway: Some(gateway.into()),
        }
    }

    /// Create an ENS storage backend.
    #[cfg(test)]
    pub fn ens(name: impl Into<String>) -> Self {
        Self::Ens { name: name.into() }
    }

    // ========================================================================
    // URL Generation
    // ========================================================================

    /// Get the base URL for raw content fetching.
    ///
    /// This URL is used for reading files directly.
    pub fn raw_base_url(&self) -> String {
        match self {
            Self::GitHub {
                owner,
                repo,
                branch,
            } => {
                format!(
                    "https://raw.githubusercontent.com/{}/{}/{}",
                    owner, repo, branch
                )
            }
            Self::Ipfs { cid, gateway } => {
                let gw = gateway.as_deref().unwrap_or("https://ipfs.io");
                format!("{}/ipfs/{}", gw, cid)
            }
            Self::Ens { name } => {
                format!("https://{}.limo", name)
            }
        }
    }

    /// Get URL for a specific file path.
    pub fn raw_url(&self, path: &str) -> String {
        let base = self.raw_base_url();
        if path.is_empty() {
            base
        } else {
            format!("{}/{}", base, path)
        }
    }

    /// Get the manifest URL for this storage.
    pub fn manifest_url(&self) -> String {
        self.raw_url("manifest.json")
    }

    /// Get GitHub API URL for a file path.
    ///
    /// Returns None for non-GitHub storage.
    pub fn api_url(&self, path: &str) -> Option<String> {
        match self {
            Self::GitHub { owner, repo, .. } => Some(format!(
                "https://api.github.com/repos/{}/{}/contents/{}",
                owner, repo, path
            )),
            _ => None,
        }
    }

    // ========================================================================
    // Backend Properties
    // ========================================================================

    /// Check if this storage backend supports writing.
    pub fn is_writable(&self) -> bool {
        matches!(self, Self::GitHub { .. })
    }

    /// Get a short description of the storage type.
    pub fn description(&self) -> String {
        match self {
            Self::GitHub { owner, repo, .. } => format!("github:{}/{}", owner, repo),
            Self::Ipfs { cid, .. } => format!("ipfs:{}", &cid[..8.min(cid.len())]),
            Self::Ens { name } => format!("ens:{}", name),
        }
    }

    /// Get the backend type identifier.
    pub fn backend_type(&self) -> &'static str {
        match self {
            Self::GitHub { .. } => "github",
            Self::Ipfs { .. } => "ipfs",
            Self::Ens { .. } => "ens",
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_urls() {
        let storage = Storage::github("0xwonj", "db", "main");

        assert_eq!(
            storage.raw_base_url(),
            "https://raw.githubusercontent.com/0xwonj/db/main"
        );
        assert_eq!(
            storage.raw_url("blog/post.md"),
            "https://raw.githubusercontent.com/0xwonj/db/main/blog/post.md"
        );
        assert_eq!(
            storage.manifest_url(),
            "https://raw.githubusercontent.com/0xwonj/db/main/manifest.json"
        );
        assert_eq!(
            storage.api_url("blog/post.md"),
            Some("https://api.github.com/repos/0xwonj/db/contents/blog/post.md".to_string())
        );
        assert!(storage.is_writable());
    }

    #[test]
    fn test_ipfs_urls() {
        let storage = Storage::ipfs("QmXyz123");

        assert_eq!(storage.raw_base_url(), "https://ipfs.io/ipfs/QmXyz123");
        assert_eq!(
            storage.raw_url("file.md"),
            "https://ipfs.io/ipfs/QmXyz123/file.md"
        );
        assert_eq!(storage.api_url("file.md"), None);
        assert!(!storage.is_writable());
    }

    #[test]
    fn test_ipfs_custom_gateway() {
        let storage = Storage::ipfs_with_gateway("QmXyz123", "https://cloudflare-ipfs.com");

        assert_eq!(
            storage.raw_base_url(),
            "https://cloudflare-ipfs.com/ipfs/QmXyz123"
        );
    }

    #[test]
    fn test_ens_urls() {
        let storage = Storage::ens("vitalik.eth");

        assert_eq!(storage.raw_base_url(), "https://vitalik.eth.limo");
        assert_eq!(
            storage.raw_url("file.md"),
            "https://vitalik.eth.limo/file.md"
        );
        assert!(!storage.is_writable());
    }

    #[test]
    fn test_description() {
        assert_eq!(
            Storage::github("user", "repo", "main").description(),
            "github:user/repo"
        );
        assert_eq!(Storage::ipfs("QmXyz12345").description(), "ipfs:QmXyz123");
        assert_eq!(Storage::ens("test.eth").description(), "ens:test.eth");
    }
}
