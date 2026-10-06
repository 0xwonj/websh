//! Browser GitHub storage backend and mount builders.

use std::rc::Rc;

use websh_core::domain::{GitHubMount, RuntimeMount};
use websh_core::ports::StorageBackendRef;

mod client;

pub use client::{GitHubBackend, rejected};

type DeclaredBackend = (
    RuntimeMount,
    StorageBackendRef,
    Option<super::mount_cache::CacheDescriptor>,
);

pub fn build_backend_for_declaration(config: GitHubMount) -> DeclaredBackend {
    let mount = config.runtime_mount();
    let backend = GitHubBackend::new(config);
    let descriptor = backend.cache_descriptor();
    (mount, Rc::new(backend), descriptor)
}

/// Rebuild the reader from exact wire evidence before accepting a cached listing.
pub fn restore_backend(
    descriptor: &super::mount_cache::CacheDescriptor,
    snapshot: websh_core::publication::SourceSnapshot,
) -> websh_core::ports::StorageResult<Rc<GitHubBackend>> {
    let config = if descriptor.root == "/" {
        GitHubMount::bootstrap(&websh_site::BOOTSTRAP_SITE).map_err(rejected)?
    } else {
        serde_json::from_value(serde_json::json!({
            "backend": "github", "mount_at": descriptor.root, "repo": descriptor.repo,
            "branch": descriptor.reference, "root": descriptor.prefix, "trust": "unsigned"
        }))
        .map_err(rejected)?
    };
    let backend = GitHubBackend::from_snapshot(config, snapshot)?;
    if backend.cache_descriptor().as_ref() != Some(descriptor) {
        return Err(rejected("cached source descriptor mismatch"));
    }
    Ok(Rc::new(backend))
}
