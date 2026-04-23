use std::sync::Arc;

use crate::core::changes::ChangeSet;
use crate::core::engine::GlobalFs;
use crate::core::merge;
use crate::core::storage::{
    CommitOutcome, CommitRequest, StorageBackend, StorageError, StorageResult,
};
use crate::models::VirtualPath;

pub async fn commit_backend(
    backend: Arc<dyn StorageBackend>,
    mount_root: VirtualPath,
    changes: ChangeSet,
    message: String,
    expected_head: Option<String>,
) -> StorageResult<CommitOutcome> {
    let request = prepare_commit(&backend, &mount_root, &changes, message, expected_head).await?;
    backend.commit(&request).await
}

async fn prepare_commit(
    backend: &Arc<dyn StorageBackend>,
    mount_root: &VirtualPath,
    changes: &ChangeSet,
    message: String,
    expected_head: Option<String>,
) -> StorageResult<CommitRequest> {
    let base_snapshot = backend.scan().await?;
    let mut merged = GlobalFs::empty();
    merged
        .mount_scanned_subtree(mount_root.clone(), &base_snapshot)
        .map_err(|error| StorageError::BadRequest(format!("assemble commit view: {error:?}")))?;

    let staged_changes = changes.staged_subset();
    for (path, _) in staged_changes.iter_staged() {
        if !path.starts_with(mount_root) {
            return Err(StorageError::BadRequest(format!(
                "staged change {path} is outside commit root {mount_root}"
            )));
        }
    }

    merge::apply_staged_changes_to_global_for_root(&mut merged, &staged_changes, mount_root);

    let merged_snapshot = merged
        .export_mount_snapshot(mount_root)
        .ok_or_else(|| StorageError::BadRequest(format!("missing mount root {mount_root}")))?;

    Ok(CommitRequest {
        changes: staged_changes,
        merged_snapshot,
        message,
        expected_head,
        auth_token: crate::core::runtime::state::get_github_token(),
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use crate::core::changes::ChangeType;
    use crate::core::storage::{
        BoxFuture, ScannedFile, ScannedSubtree, StorageBackend, StorageResult,
    };
    use crate::models::FileMetadata;

    use super::*;

    struct PrepareBackend {
        scan: RefCell<Option<ScannedSubtree>>,
    }

    impl StorageBackend for PrepareBackend {
        fn backend_type(&self) -> &'static str {
            "prepare"
        }

        fn scan(&self) -> BoxFuture<'_, StorageResult<ScannedSubtree>> {
            let scan = self.scan.borrow_mut().take().unwrap_or_default();
            Box::pin(async move { Ok(scan) })
        }

        fn read_text<'a>(&'a self, _rel_path: &'a str) -> BoxFuture<'a, StorageResult<String>> {
            Box::pin(async move { unreachable!("read unused") })
        }

        fn read_bytes<'a>(&'a self, _rel_path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>> {
            Box::pin(async move { unreachable!("read unused") })
        }

        fn commit<'a>(
            &'a self,
            _request: &'a CommitRequest,
        ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
            Box::pin(async move { unreachable!("commit unused") })
        }
    }

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepared_commit_contains_merged_staged_snapshot() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: RefCell::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "keep.md".to_string(),
                    description: "Keep".to_string(),
                    meta: FileMetadata::default(),
                }],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/site/new.md"),
            ChangeType::CreateFile {
                content: "new".to_string(),
                meta: FileMetadata::default(),
            },
        );
        let unstaged = p("/site/draft.md");
        changes.upsert(
            unstaged.clone(),
            ChangeType::CreateFile {
                content: "draft".to_string(),
                meta: FileMetadata::default(),
            },
        );
        changes.unstage(&unstaged);

        let request = prepare_commit(
            &backend,
            &p("/site"),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
        )
        .await
        .unwrap();

        let paths: Vec<_> = request
            .merged_snapshot
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        assert_eq!(paths, vec!["keep.md", "new.md"]);
        assert_eq!(request.changes.len(), 1);
        assert_eq!(request.expected_head.as_deref(), Some("old"));
        assert_eq!(request.auth_token, None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepared_commit_rejects_staged_changes_outside_mount_root() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: RefCell::new(Some(ScannedSubtree::default())),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/mnt/db/new.md"),
            ChangeType::CreateFile {
                content: "db".to_string(),
                meta: FileMetadata::default(),
            },
        );

        let error = prepare_commit(
            &backend,
            &p("/site"),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
        )
        .await
        .expect_err("commit preparation must reject cross-mount staged changes");

        assert!(matches!(error, StorageError::BadRequest(_)));
    }
}
