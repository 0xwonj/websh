//! Runtime mount and bootstrap source models.

use crate::models::VirtualPath;

/// The single code-declared bootstrap source used to discover `/site`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapSiteSource {
    pub repo_with_owner: &'static str,
    pub branch: &'static str,
    pub content_root: &'static str,
    pub gateway: &'static str,
    pub writable: bool,
}

impl BootstrapSiteSource {
    pub fn mount_root(&self) -> VirtualPath {
        VirtualPath::from_absolute("/site").expect("bootstrap site root must be absolute")
    }

    pub fn label(&self) -> &'static str {
        "~"
    }
}

/// Backend kind associated with a mounted canonical subtree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeBackendKind {
    GitHub,
    Ipfs,
    Ens,
}

/// Mounted runtime subtree plus write ownership metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeMount {
    pub root: VirtualPath,
    pub label: String,
    pub backend_kind: RuntimeBackendKind,
    pub writable: bool,
}

impl RuntimeMount {
    pub fn new(
        root: VirtualPath,
        label: impl Into<String>,
        backend_kind: RuntimeBackendKind,
        writable: bool,
    ) -> Self {
        Self {
            root,
            label: label.into(),
            backend_kind,
            writable,
        }
    }

    pub fn contains(&self, path: &VirtualPath) -> bool {
        path.starts_with(&self.root)
    }

    pub fn storage_id(&self) -> String {
        if self.root.as_str() == "/site" {
            "~".to_string()
        } else {
            self.root.as_str().trim_start_matches('/').replace('/', ":")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_site_mount_root_is_site() {
        let source = BootstrapSiteSource {
            repo_with_owner: "0xwonj/db",
            branch: "main",
            content_root: "~",
            gateway: "https://raw.githubusercontent.com",
            writable: true,
        };

        assert_eq!(source.mount_root().as_str(), "/site");
        assert_eq!(source.label(), "~");
    }

    #[test]
    fn runtime_mount_storage_id_uses_home_alias_for_site() {
        let mount = RuntimeMount::new(
            VirtualPath::from_absolute("/site").unwrap(),
            "~",
            RuntimeBackendKind::GitHub,
            true,
        );
        assert_eq!(mount.storage_id(), "~");
    }

    #[test]
    fn runtime_mount_contains_canonical_subpaths() {
        let mount = RuntimeMount::new(
            VirtualPath::from_absolute("/mnt/db").unwrap(),
            "db",
            RuntimeBackendKind::GitHub,
            false,
        );

        assert!(mount.contains(&VirtualPath::from_absolute("/mnt/db/notes/todo.md").unwrap()));
        assert!(!mount.contains(&VirtualPath::from_absolute("/mnt/db2").unwrap()));
    }
}
