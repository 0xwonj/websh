//! One-shot boot helpers: construct backends, load persisted draft ChangeSets,
//! and seed bootstrap route nodes.

use std::sync::Arc;

use crate::config::BOOTSTRAP_SITE;
use crate::core::changes::ChangeSet;
use crate::core::engine::GlobalFs;
use crate::core::storage::{StorageBackend, StorageResult};
use crate::models::{
    BootstrapSiteSource, DirectoryMetadata, FileMetadata, FileSidecarMetadata, MountDeclaration,
    NodeKind, RendererKind, RuntimeBackendKind, RuntimeMount, VirtualPath,
};

use super::github::GitHubBackend;
use super::idb;

pub fn bootstrap_runtime_mount() -> RuntimeMount {
    RuntimeMount::new(
        BOOTSTRAP_SITE.mount_root(),
        BOOTSTRAP_SITE.label(),
        RuntimeBackendKind::GitHub,
        BOOTSTRAP_SITE.writable,
    )
}

pub fn build_backend_for_bootstrap_site(source: &BootstrapSiteSource) -> Arc<dyn StorageBackend> {
    let prefix = source.content_root.trim_matches('/').to_string();
    let gateway = source.gateway.trim_end_matches('/');

    Arc::new(GitHubBackend::new(
        source.repo_with_owner,
        source.branch,
        source.mount_root(),
        prefix,
        gateway,
    ))
}

pub fn build_backend_for_declaration(
    declaration: &MountDeclaration,
) -> Option<(RuntimeMount, Arc<dyn StorageBackend>)> {
    match declaration.backend.as_str() {
        "github" => {
            let repo = declaration.repo.clone()?;
            let branch = declaration
                .branch
                .clone()
                .unwrap_or_else(|| "main".to_string());
            let mount_root = VirtualPath::from_absolute(declaration.mount_at.clone()).ok()?;
            let prefix = declaration
                .root
                .clone()
                .unwrap_or_default()
                .trim_matches('/')
                .to_string();
            let gateway = declaration
                .gateway
                .as_deref()
                .unwrap_or("https://raw.githubusercontent.com")
                .trim_end_matches('/');
            let label = declaration.name.clone().unwrap_or_else(|| {
                mount_root
                    .file_name()
                    .map(str::to_string)
                    .unwrap_or_else(|| mount_root.as_str().to_string())
            });

            let mount = RuntimeMount::new(
                mount_root.clone(),
                label,
                RuntimeBackendKind::GitHub,
                declaration.writable,
            );

            Some((
                mount,
                Arc::new(GitHubBackend::new(
                    repo, branch, mount_root, prefix, gateway,
                )),
            ))
        }
        _ => None,
    }
}

pub fn bootstrap_global_fs() -> GlobalFs {
    let mut global = GlobalFs::empty();
    seed_bootstrap_routes(&mut global);
    global
}

pub fn seed_bootstrap_routes(global: &mut GlobalFs) {
    let site_root = VirtualPath::from_absolute("/site").expect("constant path");
    if !global.exists(&site_root) {
        global.upsert_directory(
            site_root,
            DirectoryMetadata {
                title: "site".to_string(),
                ..Default::default()
            },
        );
    }

    seed_bootstrap_app(global, "/site/shell.app", "/shell");
    seed_bootstrap_app(global, "/site/fs.app", "/fs/*path");
}

fn seed_bootstrap_app(global: &mut GlobalFs, node_path: &str, route: &str) {
    let node_path = VirtualPath::from_absolute(node_path).expect("constant path");
    if !global.exists(&node_path) {
        global.upsert_binary_placeholder(node_path.clone(), FileMetadata::default());
    }
    global.set_node_metadata(
        node_path,
        FileSidecarMetadata {
            kind: Some(NodeKind::App),
            renderer: Some(RendererKind::TerminalApp),
            route: Some(route.to_string()),
            ..Default::default()
        }
        .into(),
    );
}

pub async fn hydrate_drafts(mount_id: &str) -> StorageResult<ChangeSet> {
    let db = idb::open_db().await?;
    Ok(idb::load_draft(&db, mount_id).await?.unwrap_or_default())
}

pub async fn hydrate_remote_head(mount_id: &str) -> StorageResult<Option<String>> {
    let db = idb::open_db().await?;
    let key = format!("remote_head.{mount_id}");
    idb::load_metadata(&db, &key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_builds_github_backend() {
        let declaration = MountDeclaration {
            backend: "github".to_string(),
            mount_at: "/mnt/db".to_string(),
            repo: Some("0xwonj/db".to_string()),
            branch: Some("main".to_string()),
            root: Some("content".to_string()),
            ..Default::default()
        };

        let (mount, backend) = build_backend_for_declaration(&declaration).expect("backend");
        assert_eq!(mount.root.as_str(), "/mnt/db");
        assert_eq!(mount.label, "db");
        assert_eq!(backend.backend_type(), "github");
    }

    #[test]
    fn bootstrap_global_fs_seeds_shell_and_fs_routes() {
        let global = bootstrap_global_fs();
        assert!(global.exists(&VirtualPath::from_absolute("/site/shell.app").unwrap()));
        assert!(global.exists(&VirtualPath::from_absolute("/site/fs.app").unwrap()));
        assert_eq!(
            global
                .node_metadata(&VirtualPath::from_absolute("/site/shell.app").unwrap())
                .and_then(|meta| meta.route.as_deref()),
            Some("/shell")
        );
        assert_eq!(
            global
                .node_metadata(&VirtualPath::from_absolute("/site/fs.app").unwrap())
                .and_then(|meta| meta.route.as_deref()),
            Some("/fs/*path")
        );
    }

    #[test]
    fn bootstrap_runtime_mount_is_site_root() {
        let mount = bootstrap_runtime_mount();
        assert_eq!(mount.root.as_str(), "/site");
        assert_eq!(mount.label, "~");
        assert!(mount.writable);
    }
}
