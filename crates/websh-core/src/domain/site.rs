//! Validated GitHub configuration shared by content generation and browser reads.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{BootstrapSiteSource, RuntimeMount, VirtualPath, VirtualPathParseError};

const RAW_GITHUB_GATEWAY: &str = "https://raw.githubusercontent.com";

/// A supported mount with canonical paths and resolved source defaults.
/// Deserialization validates the complete declaration before it reaches an adapter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "MountInput", into = "MountInput")]
pub struct GitHubMount {
    mount_at: VirtualPath,
    repo: String,
    branch: String,
    root: String,
    gateway: String,
    name: Option<String>,
    trust: MountTrust,
}

#[derive(Debug, Error)]
pub enum MountConfigError {
    #[error("mount target must be a top-level public directory: {0}")]
    InvalidMountRoot(VirtualPath),
    #[error("invalid GitHub owner/repository: {0}")]
    InvalidRepo(String),
    #[error("invalid GitHub branch: {0}")]
    InvalidBranch(String),
    #[error("invalid repository root: {0}")]
    InvalidRoot(#[source] VirtualPathParseError),
    #[error("unsupported gateway: {0}; expected https://raw.githubusercontent.com")]
    UnsupportedGateway(String),
    #[error("mount name must not be blank or contain control characters")]
    InvalidName,
    #[error("external mounts must explicitly allow unsigned content")]
    InvalidTrust,
}

/// A signed root authenticates its own snapshot, never an independently changing mount.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MountTrust {
    Owner,
    Unsigned,
}

/// Public declarations cannot replace the site or occupy its system namespace.
pub fn validate_mount_root(root: &VirtualPath) -> Result<(), MountConfigError> {
    if root.segments().count() != 1 || root.as_str().starts_with("/.websh") {
        return Err(MountConfigError::InvalidMountRoot(root.clone()));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Backend {
    GitHub,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MountInput {
    backend: Backend,
    trust: MountTrust,
    mount_at: VirtualPath,
    repo: String,
    #[serde(default = "default_branch")]
    branch: String,
    #[serde(default)]
    root: String,
    #[serde(default = "default_gateway")]
    gateway: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

fn default_branch() -> String {
    "main".to_string()
}

fn default_gateway() -> String {
    RAW_GITHUB_GATEWAY.to_string()
}

impl GitHubMount {
    /// Bootstrap is the only source allowed at the filesystem root.
    pub fn bootstrap(source: &BootstrapSiteSource) -> Result<Self, MountConfigError> {
        Self::from_input(MountInput {
            backend: Backend::GitHub,
            trust: MountTrust::Owner,
            mount_at: source.mount_root(),
            repo: source.repo_with_owner.to_string(),
            branch: source.branch.to_string(),
            root: source.content_root.to_string(),
            gateway: source.gateway.to_string(),
            name: Some(source.label().to_string()),
        })
    }

    fn from_input(input: MountInput) -> Result<Self, MountConfigError> {
        let segments = input.repo.split('/').collect::<Vec<_>>();
        if segments.len() != 2
            || segments.iter().any(|segment| {
                segment.is_empty()
                    || matches!(*segment, "." | "..")
                    || !segment
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            })
        {
            return Err(MountConfigError::InvalidRepo(input.repo));
        }
        if input.branch.is_empty()
            || input.branch.chars().any(char::is_whitespace)
            || VirtualPath::from_absolute(format!("/{}", input.branch)).is_err()
        {
            return Err(MountConfigError::InvalidBranch(input.branch));
        }
        if input
            .name
            .as_deref()
            .is_some_and(|name| name.trim().is_empty() || name.chars().any(char::is_control))
        {
            return Err(MountConfigError::InvalidName);
        }
        // The same canonical path rules govern a repository-relative prefix.
        VirtualPath::from_absolute(format!("/{}", input.root))
            .map_err(MountConfigError::InvalidRoot)?;
        let gateway = match input.gateway.as_str() {
            RAW_GITHUB_GATEWAY => RAW_GITHUB_GATEWAY,
            _ => return Err(MountConfigError::UnsupportedGateway(input.gateway)),
        };
        Ok(Self {
            mount_at: input.mount_at,
            repo: input.repo,
            branch: input.branch,
            root: input.root,
            gateway: gateway.to_string(),
            name: input.name,
            trust: input.trust,
        })
    }

    pub fn trust(&self) -> MountTrust {
        self.trust
    }

    pub fn mount_at(&self) -> &VirtualPath {
        &self.mount_at
    }

    pub fn runtime_mount(&self) -> RuntimeMount {
        RuntimeMount::new(
            self.mount_at.clone(),
            self.name
                .as_deref()
                .unwrap_or_else(|| self.mount_at.file_name().unwrap_or("~")),
        )
    }

    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn root(&self) -> &str {
        &self.root
    }

    pub fn gateway(&self) -> &str {
        &self.gateway
    }
}

impl TryFrom<MountInput> for GitHubMount {
    type Error = MountConfigError;

    fn try_from(input: MountInput) -> Result<Self, Self::Error> {
        validate_mount_root(&input.mount_at)?;
        if input.trust != MountTrust::Unsigned {
            return Err(MountConfigError::InvalidTrust);
        }
        Self::from_input(input)
    }
}

impl From<GitHubMount> for MountInput {
    fn from(mount: GitHubMount) -> Self {
        Self {
            backend: Backend::GitHub,
            trust: mount.trust,
            mount_at: mount.mount_at,
            repo: mount.repo,
            branch: mount.branch,
            root: mount.root,
            gateway: mount.gateway,
            name: mount.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn declarations_resolve_defaults_and_preserve_canonical_sources() {
        let mount: GitHubMount = serde_json::from_value(json!({
            "backend": "github", "trust": "unsigned", "mount_at": "/db", "repo": "owner/repo"
        }))
        .unwrap();
        assert_eq!(mount.runtime_mount().label, "db");
        assert_eq!(mount.branch(), "main");
        assert_eq!(mount.root(), "");
        assert_eq!(mount.gateway(), RAW_GITHUB_GATEWAY);
        assert_eq!(
            serde_json::from_value::<GitHubMount>(serde_json::to_value(&mount).unwrap()).unwrap(),
            mount
        );
        let local: GitHubMount = serde_json::from_value(json!({
            "backend": "github", "trust": "unsigned", "mount_at": "/db", "repo": "owner/repo",
            "branch": "feature/topic", "root": "~/notes", "gateway": "https://raw.githubusercontent.com", "name": "Notes"
        }))
        .unwrap();
        assert_eq!(local.runtime_mount().label, "Notes");
        assert_eq!(local.root(), "~/notes");
        assert_eq!(local.gateway(), RAW_GITHUB_GATEWAY);
    }

    #[test]
    fn declarations_reject_invalid_or_ambiguous_configuration() {
        let valid = json!({"backend": "github", "trust": "unsigned", "mount_at": "/db", "repo": "owner/repo"});
        for (field, values) in [
            ("backend", vec!["other"]),
            ("trust", vec!["owner", "other"]),
            (
                "mount_at",
                vec!["/", "/.websh", "/.websh/state", "/db/sub", "/db/../other"],
            ),
            (
                "repo",
                vec![
                    "",
                    "owner",
                    "owner/repo/extra",
                    "../repo",
                    "owner/repo?query",
                ],
            ),
            (
                "branch",
                vec!["", " ", "main\n", "../main", "main/", "feature//topic"],
            ),
            (
                "root",
                vec!["/", "/content", "content/", "a//b", "a/../b", "a\\b"],
            ),
            (
                "gateway",
                vec!["https://example.com", "https://raw.githubusercontent.com/"],
            ),
            ("name", vec!["", "   ", "Notes\nMore"]),
        ] {
            for value in values {
                let mut input = valid.clone();
                input[field] = json!(value);
                assert!(
                    serde_json::from_value::<GitHubMount>(input).is_err(),
                    "{field}: {value:?}"
                );
            }
        }
        let mut unknown = valid.clone();
        unknown["unexpected"] = json!(true);
        assert!(serde_json::from_value::<GitHubMount>(unknown).is_err());
        let mut missing = valid;
        missing.as_object_mut().unwrap().remove("repo");
        assert!(serde_json::from_value::<GitHubMount>(missing).is_err());
    }

    #[test]
    fn bootstrap_reuses_source_validation_but_owns_the_root() {
        let source = BootstrapSiteSource {
            repo_with_owner: "owner/site",
            branch: "main",
            content_root: "content",
            gateway: RAW_GITHUB_GATEWAY,
        };
        assert!(
            GitHubMount::bootstrap(&source)
                .unwrap()
                .mount_at()
                .is_root()
        );
        assert!(
            GitHubMount::bootstrap(&BootstrapSiteSource {
                content_root: "/content/",
                ..source
            })
            .is_err()
        );
    }
}
