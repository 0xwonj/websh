//! One-shot boot helpers: construct the writable mount's backend, load
//! any persisted draft ChangeSet, and seed remote_head.

use std::sync::Arc;

use crate::core::changes::ChangeSet;
use crate::core::storage::{GitHubBackend, StorageBackend, StorageResult};
use crate::models::Mount;

use super::idb;

pub fn build_backend_for_mount(mount: &Mount, token: Option<&str>) -> Option<Arc<dyn StorageBackend>> {
    if !mount.is_writable() {
        return None;
    }
    let token = token?;
    match mount {
        Mount::GitHub { base_url, content_prefix, .. } => {
            let repo = parse_repo_from_base_url(base_url)?;
            let branch = parse_branch_from_base_url(base_url).unwrap_or_else(|| "main".to_string());
            let prefix = content_prefix.clone().unwrap_or_default();
            let manifest_url = format!("{}/manifest.json", base_url);
            Some(Arc::new(GitHubBackend::new(repo, branch, prefix, manifest_url, token)))
        }
        _ => None,
    }
}

/// Parse "owner/repo" from `https://raw.githubusercontent.com/owner/repo/branch/...`
fn parse_repo_from_base_url(url: &str) -> Option<String> {
    let tail = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut parts = tail.splitn(3, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some(format!("{owner}/{repo}"))
}

fn parse_branch_from_base_url(url: &str) -> Option<String> {
    let tail = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut parts = tail.splitn(4, '/');
    let _owner = parts.next()?;
    let _repo = parts.next()?;
    let branch = parts.next()?;
    Some(branch.to_string())
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
    fn parse_repo_from_raw_url() {
        assert_eq!(
            parse_repo_from_base_url("https://raw.githubusercontent.com/0xwonj/db/main/content"),
            Some("0xwonj/db".to_string())
        );
    }

    #[test]
    fn parse_branch_from_raw_url() {
        assert_eq!(
            parse_branch_from_base_url("https://raw.githubusercontent.com/0xwonj/db/main/content"),
            Some("main".to_string())
        );
    }

    #[test]
    fn build_backend_refuses_readonly() {
        let mount = Mount::github_with_prefix(
            "ro", "https://raw.githubusercontent.com/x/y/main", "~",
        );
        assert!(build_backend_for_mount(&mount, Some("t")).is_none());
    }

    #[test]
    fn build_backend_refuses_missing_token() {
        let mount = Mount::github_writable(
            "~", "https://raw.githubusercontent.com/0xwonj/db/main", "~",
        );
        assert!(build_backend_for_mount(&mount, None).is_none());
    }
}
