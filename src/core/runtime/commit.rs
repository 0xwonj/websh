use std::collections::BTreeSet;
use std::sync::Arc;

use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::engine::GlobalFs;
use crate::core::merge;
use crate::core::storage::{
    CommitDelta, CommitFileAddition, CommitOutcome, CommitRequest, ScannedSubtree, StorageBackend,
    StorageError, StorageResult,
};
use crate::models::VirtualPath;

pub async fn commit_backend(
    backend: Arc<dyn StorageBackend>,
    mount_root: VirtualPath,
    changes: ChangeSet,
    message: String,
    expected_head: Option<String>,
    auth_token: Option<String>,
) -> StorageResult<CommitOutcome> {
    let request = prepare_commit(
        &backend,
        &mount_root,
        &changes,
        message,
        expected_head,
        auth_token,
    )
    .await?;
    backend.commit(&request).await
}

async fn prepare_commit(
    backend: &Arc<dyn StorageBackend>,
    mount_root: &VirtualPath,
    changes: &ChangeSet,
    message: String,
    expected_head: Option<String>,
    auth_token: Option<String>,
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

    let normalized_changes = normalized_staged_changes(&staged_changes);
    let delta = build_commit_delta(
        &base_snapshot,
        mount_root,
        &staged_changes,
        &normalized_changes,
    )?;

    merge::apply_staged_changes_to_global_for_root(&mut merged, &normalized_changes, mount_root);

    let merged_snapshot = merged
        .export_mount_snapshot(mount_root)
        .ok_or_else(|| StorageError::BadRequest(format!("missing mount root {mount_root}")))?;

    Ok(CommitRequest {
        delta,
        merged_snapshot,
        message,
        expected_head,
        auth_token,
    })
}

fn normalized_staged_changes(changes: &ChangeSet) -> ChangeSet {
    let deleted_dirs = delete_directory_paths(changes);
    let mut normalized = ChangeSet::new();

    for (path, entry) in changes.iter_staged() {
        if is_descendant_of_deleted_dir(path, &deleted_dirs) {
            continue;
        }
        normalized.upsert(path.clone(), entry.change.clone());
    }

    normalized
}

fn build_commit_delta(
    base_snapshot: &ScannedSubtree,
    mount_root: &VirtualPath,
    original_staged_changes: &ChangeSet,
    normalized_changes: &ChangeSet,
) -> StorageResult<CommitDelta> {
    let mut additions = Vec::new();
    let mut deletions = Vec::new();
    let mut changed_paths: Vec<_> = original_staged_changes
        .iter_staged()
        .map(|(path, _)| path.clone())
        .collect();

    for (path, entry) in normalized_changes.iter_staged() {
        match &entry.change {
            ChangeType::CreateFile { content, .. } | ChangeType::UpdateFile { content, .. } => {
                additions.push(CommitFileAddition {
                    path: path.clone(),
                    content: content.clone(),
                });
            }
            ChangeType::DeleteFile => {
                deletions.push(path.clone());
            }
            ChangeType::DeleteDirectory => {
                deletions.extend(deleted_files_for_directory_change(
                    base_snapshot,
                    mount_root,
                    path,
                ));
            }
            ChangeType::CreateBinary { .. } | ChangeType::CreateDirectory { .. } => {}
        }
    }

    additions.sort_by(|left, right| left.path.cmp(&right.path));
    deletions.sort();
    deletions.dedup();
    changed_paths.sort();
    changed_paths.dedup();

    let addition_paths = additions
        .iter()
        .map(|addition| addition.path.clone())
        .collect::<BTreeSet<_>>();
    if let Some(conflict) = deletions.iter().find(|path| addition_paths.contains(*path)) {
        return Err(StorageError::BadRequest(format!(
            "commit delta has both addition and deletion for {conflict}"
        )));
    }

    Ok(CommitDelta {
        additions,
        deletions,
        changed_paths,
    })
}

fn delete_directory_paths(changes: &ChangeSet) -> Vec<VirtualPath> {
    changes
        .iter_staged()
        .filter_map(|(path, entry)| {
            matches!(entry.change, ChangeType::DeleteDirectory).then(|| path.clone())
        })
        .collect()
}

fn is_descendant_of_deleted_dir(path: &VirtualPath, deleted_dirs: &[VirtualPath]) -> bool {
    deleted_dirs
        .iter()
        .any(|deleted_dir| path != deleted_dir && path.starts_with(deleted_dir))
}

fn deleted_files_for_directory_change(
    base_snapshot: &ScannedSubtree,
    mount_root: &VirtualPath,
    path: &VirtualPath,
) -> Vec<VirtualPath> {
    let mut deleted = Vec::new();

    let Some(rel_dir) = path.strip_prefix(mount_root) else {
        return deleted;
    };
    for file in &base_snapshot.files {
        let is_descendant = rel_dir.is_empty()
            || file.path == rel_dir
            || file
                .path
                .strip_prefix(rel_dir)
                .is_some_and(|rest| rest.starts_with('/'));
        if is_descendant {
            deleted.push(mount_root.join(&file.path));
        }
    }

    deleted.sort();
    deleted.dedup();
    deleted
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

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
            None,
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
        assert!(request.delta.deletions.is_empty());
        assert_eq!(request.delta.additions.len(), 1);
        assert_eq!(request.delta.changed_paths, vec![p("/site/new.md")]);
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
            None,
        )
        .await
        .expect_err("commit preparation must reject cross-mount staged changes");

        assert!(matches!(error, StorageError::BadRequest(_)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepared_commit_expands_directory_delete_to_descendant_files() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: RefCell::new(Some(ScannedSubtree {
                files: vec![
                    ScannedFile {
                        path: "docs/a.md".to_string(),
                        description: "A".to_string(),
                        meta: FileMetadata::default(),
                    },
                    ScannedFile {
                        path: "docs/deep/b.md".to_string(),
                        description: "B".to_string(),
                        meta: FileMetadata::default(),
                    },
                    ScannedFile {
                        path: "keep.md".to_string(),
                        description: "Keep".to_string(),
                        meta: FileMetadata::default(),
                    },
                ],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(p("/site/docs"), ChangeType::DeleteDirectory);

        let request = prepare_commit(
            &backend,
            &p("/site"),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
            None,
        )
        .await
        .unwrap();

        let paths: Vec<_> = request
            .delta
            .deletions
            .iter()
            .map(|path| path.as_str())
            .collect();
        assert_eq!(paths, vec!["/site/docs/a.md", "/site/docs/deep/b.md"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepared_commit_delete_directory_suppresses_descendant_additions() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: RefCell::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "docs/a.md".to_string(),
                    description: "A".to_string(),
                    meta: FileMetadata::default(),
                }],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/site/docs/a.md"),
            ChangeType::UpdateFile {
                content: "new".to_string(),
                description: None,
            },
        );
        changes.upsert(p("/site/docs"), ChangeType::DeleteDirectory);

        let request = prepare_commit(
            &backend,
            &p("/site"),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
            None,
        )
        .await
        .unwrap();

        assert!(request.delta.additions.is_empty());
        assert_eq!(request.delta.deletions, vec![p("/site/docs/a.md")]);
        assert_eq!(
            request.delta.changed_paths,
            vec![p("/site/docs"), p("/site/docs/a.md")]
        );
        assert!(request.merged_snapshot.files.is_empty());
    }
}
