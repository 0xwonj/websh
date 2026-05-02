//! Pure runtime-layer scaffolding: the canonical bootstrap mount and an
//! empty `GlobalFs` ready to receive scans. Compiles on every target so
//! host-side shell tests can rebuild the same fixtures the wasm runtime
//! sees on first boot. The host lib build never reaches these helpers
//! (only `runtime::loader` — wasm-only — and tests do), so the dead-code
//! allow only fires on host.

#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use crate::config::BOOTSTRAP_SITE;
use crate::domain::{RuntimeBackendKind, RuntimeMount, VirtualPath};
use crate::filesystem::{GlobalFs, MountError};
use crate::storage::ScannedSubtree;

pub(crate) fn bootstrap_runtime_mount() -> RuntimeMount {
    RuntimeMount::new(
        BOOTSTRAP_SITE.mount_root(),
        BOOTSTRAP_SITE.label(),
        RuntimeBackendKind::GitHub,
        BOOTSTRAP_SITE.writable,
    )
}

pub(crate) fn bootstrap_global_fs() -> GlobalFs {
    GlobalFs::empty()
}

pub(crate) fn seed_bootstrap_routes(_global: &mut GlobalFs) {
    // Shell and explorer are reserved code routes, not filesystem app nodes.
}

pub(crate) fn assemble_global_fs(
    scans: &[(VirtualPath, ScannedSubtree)],
) -> Result<GlobalFs, MountError> {
    let mut global = GlobalFs::empty();
    for (mount_root, scan) in scans {
        global.mount_scanned_subtree(mount_root.clone(), scan)?;
    }
    Ok(global)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntryExtensions, Fields, NodeKind, NodeMetadata, SCHEMA_VERSION};
    use crate::storage::{ScannedDirectory, ScannedFile};

    #[test]
    fn bootstrap_global_fs_has_root_directory_without_app_routes() {
        let global = bootstrap_global_fs();
        assert!(global.exists(&VirtualPath::root()));
        assert!(!global.exists(&VirtualPath::from_absolute("/shell.app").unwrap()));
        assert!(!global.exists(&VirtualPath::from_absolute("/fs.app").unwrap()));
    }

    #[test]
    fn bootstrap_runtime_mount_is_root() {
        let mount = bootstrap_runtime_mount();
        assert_eq!(mount.root.as_str(), "/");
        assert_eq!(mount.label, "~");
        assert!(mount.writable);
    }

    fn file_meta(kind: NodeKind) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }

    fn dir_meta(name: &str) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Directory,
            authored: Fields {
                title: if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                },
                ..Fields::default()
            },
            derived: Fields::default(),
        }
    }

    #[test]
    fn assembles_global_fs_under_canonical_mount_roots() {
        let scan = ScannedSubtree {
            files: vec![ScannedFile {
                path: "index.md".to_string(),
                meta: file_meta(NodeKind::Page),
                extensions: EntryExtensions::default(),
            }],
            directories: vec![ScannedDirectory {
                path: "".to_string(),
                meta: dir_meta("home"),
            }],
        };

        let fs = assemble_global_fs(&[(VirtualPath::root(), scan)])
            .expect("global fs should assemble");

        assert!(
            fs.get_entry(&VirtualPath::from_absolute("/index.md").unwrap())
                .is_some()
        );
    }
}
