//! Runtime mount and bootstrap source models.

use std::sync::LazyLock;

use crate::domain::VirtualPath;

static RUNTIME_STATE_ROOT: LazyLock<VirtualPath> = LazyLock::new(|| {
    VirtualPath::from_absolute("/.websh/state").expect("runtime state root is canonical")
});

pub fn runtime_state_root() -> &'static VirtualPath {
    &RUNTIME_STATE_ROOT
}

pub fn is_runtime_overlay_path(path: &VirtualPath) -> bool {
    path.starts_with(runtime_state_root())
}

/// The single code-declared bootstrap source used to discover the root site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapSiteSource {
    pub repo_with_owner: &'static str,
    pub branch: &'static str,
    pub content_root: &'static str,
    pub gateway: &'static str,
}

impl BootstrapSiteSource {
    pub fn mount_root(&self) -> VirtualPath {
        VirtualPath::root()
    }

    pub fn label(&self) -> &'static str {
        "~"
    }
}

/// Mounted runtime subtree and its display label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeMount {
    pub root: VirtualPath,
    pub label: String,
}

impl RuntimeMount {
    pub fn new(root: VirtualPath, label: impl Into<String>) -> Self {
        Self {
            root,
            label: label.into(),
        }
    }

    pub fn contains(&self, path: &VirtualPath) -> bool {
        path.starts_with(&self.root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_site_mount_root_is_root() {
        let source = BootstrapSiteSource {
            repo_with_owner: "0xwonj/db",
            branch: "main",
            content_root: "~",
            gateway: "https://raw.githubusercontent.com",
        };

        assert_eq!(source.mount_root().as_str(), "/");
        assert_eq!(source.label(), "~");
    }

    #[test]
    fn runtime_mount_contains_canonical_subpaths() {
        let mount = RuntimeMount::new(VirtualPath::from_absolute("/db").unwrap(), "db");

        assert!(mount.contains(&VirtualPath::from_absolute("/db/notes/todo.md").unwrap()));
        assert!(!mount.contains(&VirtualPath::from_absolute("/db2").unwrap()));
    }
}
