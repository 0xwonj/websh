//! Browser GitHub storage backend and mount builders.

use std::rc::Rc;

use websh_core::domain::{BootstrapSiteSource, GitHubMount, RuntimeMount};
use websh_core::ports::StorageBackendRef;

mod client;
mod path;

pub use client::GitHubBackend;

type DeclaredBackend = (
    RuntimeMount,
    StorageBackendRef,
    Option<super::mount_cache::CacheDescriptor>,
);

pub fn build_backend_for_bootstrap_site(source: &BootstrapSiteSource) -> StorageBackendRef {
    let config = GitHubMount::bootstrap(source).expect("bootstrap site source must be valid");
    Rc::new(GitHubBackend::new(config))
}

pub fn build_backend_for_declaration(config: GitHubMount) -> DeclaredBackend {
    let mount = config.runtime_mount();
    let backend = GitHubBackend::new(config);
    let descriptor = backend.cache_descriptor();
    (mount, Rc::new(backend), descriptor)
}
