use std::collections::BTreeSet;
use std::sync::Arc;

use crate::domain::VirtualPath;
use crate::domain::changes::{ChangeSet, ChangeType};
use crate::filesystem::GlobalFs;
use crate::filesystem::merge;
use crate::storage::{
    CommitDelta, CommitFileAddition, CommitOutcome, CommitRequest, ScannedSubtree, StorageBackend,
    StorageError, StorageResult,
};

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

/// Compose a [`CommitRequest`] from the local change set + the remote
/// base snapshot.
///
/// Caveat: `base_snapshot` is fetched via the same scan path as boot
/// (raw.githubusercontent.com), which is fronted by a CDN with up to a
/// 5-minute edge cache. `expectedHeadOid` (re-fetched fresh via GraphQL
/// in [`crate::storage::github::GitHubBackend::commit`]) protects
/// the *git tree* from concurrent commits, but `manifest.json` is a
/// regular file we overwrite using this potentially-stale base. If
/// another writer landed a manifest change inside the staleness window,
/// their entries will be silently dropped from our merged snapshot. For
/// a single-author site this is a non-issue; multi-writer setups should
/// either coordinate externally or read the manifest through the
/// GraphQL `object(expression: "<oid>:manifest.json")` path against the
/// same OID we mutate against.
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
    let cleanup_paths = staged_cleanup_paths(&staged_changes);
    let delta = build_commit_delta(&base_snapshot, mount_root, &normalized_changes)?;

    merge::apply_staged_changes_to_global_for_root(&mut merged, &normalized_changes, mount_root);

    let merged_snapshot = merged
        .export_mount_snapshot(mount_root)
        .ok_or_else(|| StorageError::BadRequest(format!("missing mount root {mount_root}")))?;

    Ok(CommitRequest {
        delta,
        cleanup_paths,
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
    normalized_changes: &ChangeSet,
) -> StorageResult<CommitDelta> {
    let mut additions = Vec::new();
    let mut deletions = Vec::new();

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
    })
}

fn staged_cleanup_paths(changes: &ChangeSet) -> Vec<VirtualPath> {
    let mut paths: Vec<_> = changes
        .iter_staged()
        .map(|(path, _)| path.clone())
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

fn delete_directory_paths(changes: &ChangeSet) -> Vec<VirtualPath> {
    changes
        .iter_staged()
        .filter(|(_, entry)| matches!(entry.change, ChangeType::DeleteDirectory))
        .map(|(path, _)| path.clone())
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
    use std::sync::Mutex;

    use crate::domain::{EntryExtensions, Fields, NodeKind, NodeMetadata, SCHEMA_VERSION};
    use crate::storage::{BoxFuture, ScannedFile, ScannedSubtree, StorageBackend, StorageResult};

    use super::*;

    fn blank_meta() -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }

    struct PrepareBackend {
        scan: Mutex<Option<ScannedSubtree>>,
    }

    impl StorageBackend for PrepareBackend {
        fn backend_type(&self) -> &'static str {
            "prepare"
        }

        fn scan(&self) -> BoxFuture<'_, StorageResult<ScannedSubtree>> {
            let scan = self.scan.lock().unwrap().take().unwrap_or_default();
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
            scan: Mutex::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "keep.md".to_string(),
                    meta: blank_meta(),
                    extensions: EntryExtensions::default(),
                }],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/new.md"),
            ChangeType::CreateFile {
                content: "new".to_string(),
                meta: blank_meta(),
                extensions: EntryExtensions::default(),
            },
        );
        let unstaged = p("/draft.md");
        changes.upsert(
            unstaged.clone(),
            ChangeType::CreateFile {
                content: "draft".to_string(),
                meta: blank_meta(),
                extensions: EntryExtensions::default(),
            },
        );
        changes.unstage(&unstaged);

        let request = prepare_commit(
            &backend,
            &VirtualPath::root(),
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
        assert_eq!(request.cleanup_paths, vec![p("/new.md")]);
        assert_eq!(request.expected_head.as_deref(), Some("old"));
        assert_eq!(request.auth_token, None);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepared_commit_rejects_staged_changes_outside_mount_root() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: Mutex::new(Some(ScannedSubtree::default())),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/other/new.md"),
            ChangeType::CreateFile {
                content: "db".to_string(),
                meta: blank_meta(),
                extensions: EntryExtensions::default(),
            },
        );

        let error = prepare_commit(
            &backend,
            &p("/db"),
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
            scan: Mutex::new(Some(ScannedSubtree {
                files: vec![
                    ScannedFile {
                        path: "docs/a.md".to_string(),
                        meta: blank_meta(),
                        extensions: EntryExtensions::default(),
                    },
                    ScannedFile {
                        path: "docs/deep/b.md".to_string(),
                        meta: blank_meta(),
                        extensions: EntryExtensions::default(),
                    },
                    ScannedFile {
                        path: "keep.md".to_string(),
                        meta: blank_meta(),
                        extensions: EntryExtensions::default(),
                    },
                ],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(p("/docs"), ChangeType::DeleteDirectory);

        let request = prepare_commit(
            &backend,
            &VirtualPath::root(),
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
        assert_eq!(paths, vec!["/docs/a.md", "/docs/deep/b.md"]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn prepared_commit_delete_directory_suppresses_descendant_additions() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: Mutex::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "docs/a.md".to_string(),
                    meta: blank_meta(),
                    extensions: EntryExtensions::default(),
                }],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/docs/a.md"),
            ChangeType::UpdateFile {
                content: "new".to_string(),
                meta: None,
                extensions: None,
            },
        );
        changes.upsert(p("/docs"), ChangeType::DeleteDirectory);

        let request = prepare_commit(
            &backend,
            &VirtualPath::root(),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
            None,
        )
        .await
        .unwrap();

        assert!(request.delta.additions.is_empty());
        assert_eq!(request.delta.deletions, vec![p("/docs/a.md")]);
        assert_eq!(request.cleanup_paths, vec![p("/docs"), p("/docs/a.md")]);
        assert!(request.merged_snapshot.files.is_empty());
    }

    fn meta_with_title(title: &str) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields {
                title: Some(title.to_string()),
                ..Fields::default()
            },
            derived: Fields::default(),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_file_change_propagates_meta_into_exported_snapshot() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: Mutex::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "a.md".to_string(),
                    meta: meta_with_title("old"),
                    extensions: EntryExtensions::default(),
                }],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/a.md"),
            ChangeType::UpdateFile {
                content: "new body".to_string(),
                meta: Some(meta_with_title("new")),
                extensions: None,
            },
        );

        let request = prepare_commit(
            &backend,
            &VirtualPath::root(),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
            None,
        )
        .await
        .unwrap();

        let updated = request
            .merged_snapshot
            .files
            .iter()
            .find(|f| f.path == "a.md")
            .expect("exported file");
        assert_eq!(updated.meta.authored.title.as_deref(), Some("new"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_file_change_propagates_extensions_into_exported_snapshot() {
        use crate::domain::{MempoolFields, MempoolStatus};

        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: Mutex::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "mempool/foo.md".to_string(),
                    meta: blank_meta(),
                    extensions: EntryExtensions {
                        mempool: Some(MempoolFields {
                            status: MempoolStatus::Draft,
                            priority: None,
                            category: Some("writing".to_string()),
                        }),
                    },
                }],
                directories: vec![],
            })),
        });
        let new_ext = EntryExtensions {
            mempool: Some(MempoolFields {
                status: MempoolStatus::Review,
                priority: None,
                category: Some("writing".to_string()),
            }),
        };
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/mempool/foo.md"),
            ChangeType::UpdateFile {
                content: "body".to_string(),
                meta: None,
                extensions: Some(new_ext.clone()),
            },
        );

        let request = prepare_commit(
            &backend,
            &VirtualPath::root(),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
            None,
        )
        .await
        .unwrap();

        let updated = request
            .merged_snapshot
            .files
            .iter()
            .find(|f| f.path == "mempool/foo.md")
            .expect("exported file");
        let mp = updated
            .extensions
            .mempool
            .as_ref()
            .expect("mempool extensions");
        assert_eq!(mp.status, MempoolStatus::Review);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_file_with_none_meta_preserves_base_scan_meta() {
        let backend: Arc<dyn StorageBackend> = Arc::new(PrepareBackend {
            scan: Mutex::new(Some(ScannedSubtree {
                files: vec![ScannedFile {
                    path: "a.md".to_string(),
                    meta: meta_with_title("preserved"),
                    extensions: EntryExtensions::default(),
                }],
                directories: vec![],
            })),
        });
        let mut changes = ChangeSet::new();
        changes.upsert(
            p("/a.md"),
            ChangeType::UpdateFile {
                content: "new body".to_string(),
                meta: None,
                extensions: None,
            },
        );

        let request = prepare_commit(
            &backend,
            &VirtualPath::root(),
            &changes,
            "msg".to_string(),
            Some("old".to_string()),
            None,
        )
        .await
        .unwrap();

        let updated = request
            .merged_snapshot
            .files
            .iter()
            .find(|f| f.path == "a.md")
            .expect("exported file");
        assert_eq!(updated.meta.authored.title.as_deref(), Some("preserved"));
    }
}
