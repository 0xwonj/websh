//! Wasm-only storage boot helpers: GitHub backend constructors and IDB
//! hydration. The pure runtime scaffolding (bootstrap mount, empty global
//! filesystem) lives in `runtime::boot` so it compiles on every target.

#![cfg(target_arch = "wasm32")]

use std::sync::Arc;

use crate::domain::changes::ChangeSet;
use crate::domain::{
    BootstrapSiteSource, MountDeclaration, RuntimeBackendKind, RuntimeMount, VirtualPath,
};
use crate::storage::{StorageBackend, StorageResult};

use super::github::GitHubBackend;
use super::github::path::normalize_repo_prefix;
use super::idb;

type DeclaredBackend = (RuntimeMount, Arc<dyn StorageBackend>);

pub(crate) fn build_backend_for_bootstrap_site(
    source: &BootstrapSiteSource,
) -> Arc<dyn StorageBackend> {
    let prefix = source.content_root.trim_matches('/').to_string();
    let gateway = source.gateway.trim_end_matches('/');

    Arc::new(
        GitHubBackend::new(
            source.repo_with_owner,
            source.branch,
            source.mount_root(),
            prefix,
            gateway,
        )
        .expect("bootstrap site source must have a valid content root"),
    )
}

pub(crate) fn build_backend_for_declaration(
    declaration: &MountDeclaration,
) -> Result<Option<DeclaredBackend>, String> {
    match declaration.backend.as_str() {
        "github" => {
            let repo = declaration
                .repo
                .clone()
                .ok_or_else(|| format!("github mount {} is missing repo", declaration.mount_at))?;
            let branch = declaration
                .branch
                .clone()
                .unwrap_or_else(|| "main".to_string());
            let mount_root = VirtualPath::from_absolute(declaration.mount_at.clone())
                .map_err(|_| format!("invalid mount_at: {}", declaration.mount_at))?;
            if !is_canonical_mount_root(&mount_root) {
                return Err(format!("noncanonical mount_at: {}", declaration.mount_at));
            }
            let prefix = normalize_repo_prefix(&declaration.root.clone().unwrap_or_default())
                .map_err(|error| format!("invalid root for {}: {error}", declaration.mount_at))?;
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

            let backend =
                GitHubBackend::new(repo, branch, mount_root, prefix, gateway).map_err(|error| {
                    format!("invalid github backend {}: {error}", declaration.mount_at)
                })?;

            Ok(Some((mount, Arc::new(backend))))
        }
        _ => Ok(None),
    }
}

fn is_canonical_mount_root(path: &VirtualPath) -> bool {
    if path.is_root() || path.as_str().contains('\\') {
        return false;
    }
    let segments = path.segments().collect::<Vec<_>>();
    if segments
        .iter()
        .any(|segment| *segment == "." || *segment == ".." || segment.chars().any(char::is_control))
    {
        return false;
    }
    format!("/{}", segments.join("/")) == path.as_str()
}

pub async fn hydrate_drafts(draft_id: &str) -> StorageResult<ChangeSet> {
    let db = idb::open_db().await?;
    Ok(idb::load_draft(&db, draft_id).await?.unwrap_or_default())
}

pub(crate) async fn hydrate_remote_head(mount_id: &str) -> StorageResult<Option<String>> {
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
            mount_at: "/db".to_string(),
            repo: Some("0xwonj/db".to_string()),
            branch: Some("main".to_string()),
            root: Some("content".to_string()),
            ..Default::default()
        };

        let (mount, backend) = build_backend_for_declaration(&declaration)
            .expect("valid declaration")
            .expect("backend");
        assert_eq!(mount.root.as_str(), "/db");
        assert_eq!(mount.label, "db");
        assert_eq!(backend.backend_type(), "github");
    }

    #[test]
    fn declaration_rejects_noncanonical_mount_root() {
        let declaration = MountDeclaration {
            backend: "github".to_string(),
            mount_at: "/db/../bad".to_string(),
            repo: Some("0xwonj/db".to_string()),
            branch: Some("main".to_string()),
            root: Some("content".to_string()),
            ..Default::default()
        };

        assert!(build_backend_for_declaration(&declaration).is_err());
    }
}
