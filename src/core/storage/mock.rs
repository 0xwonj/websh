//! In-memory backend for commit-path tests. Not shipped in WASM build.

use std::cell::RefCell;

use crate::models::VirtualPath;

use super::backend::{BoxFuture, CommitOutcome, CommitRequest, ScannedSubtree, StorageBackend};
use super::error::{StorageError, StorageResult};

pub struct MockBackend {
    pub commit_calls: RefCell<Vec<CommitRecord>>,
    pub next_outcome: RefCell<Option<StorageResult<CommitOutcome>>>,
    pub next_scan: RefCell<Option<StorageResult<ScannedSubtree>>>,
    pub mount_root: VirtualPath,
}

impl Default for MockBackend {
    fn default() -> Self {
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(None),
            next_scan: RefCell::new(None),
            mount_root: VirtualPath::from_absolute("/site").unwrap(),
        }
    }
}

pub struct CommitRecord {
    pub message: String,
    pub expected_head: Option<String>,
    pub paths: Vec<VirtualPath>,
}

impl MockBackend {
    pub fn with_success(scan: ScannedSubtree, new_head: impl Into<String>) -> Self {
        let outcome = CommitOutcome {
            new_head: new_head.into(),
            committed_paths: vec![],
        };
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(Some(Ok(outcome))),
            next_scan: RefCell::new(Some(Ok(scan))),
            mount_root: VirtualPath::from_absolute("/site").unwrap(),
        }
    }

    pub fn with_conflict(head: impl Into<String>) -> Self {
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(Some(Err(StorageError::Conflict {
                remote_head: head.into(),
            }))),
            next_scan: RefCell::new(Some(Ok(ScannedSubtree::default()))),
            mount_root: VirtualPath::from_absolute("/site").unwrap(),
        }
    }
}

impl StorageBackend for MockBackend {
    fn backend_type(&self) -> &'static str {
        "mock"
    }

    fn scan(&self) -> BoxFuture<'_, StorageResult<ScannedSubtree>> {
        let m = self
            .next_scan
            .borrow_mut()
            .take()
            .unwrap_or_else(|| Ok(ScannedSubtree::default()));
        Box::pin(async move { m })
    }

    fn read_text<'a>(&'a self, _rel_path: &'a str) -> BoxFuture<'a, StorageResult<String>> {
        Box::pin(async move { Err(StorageError::NotFound("mock.read_text".into())) })
    }

    fn read_bytes<'a>(&'a self, _rel_path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>> {
        Box::pin(async move { Err(StorageError::NotFound("mock.read_bytes".into())) })
    }

    fn commit<'a>(
        &'a self,
        request: &'a CommitRequest,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
        Box::pin(async move {
            let mut paths: Vec<VirtualPath> = request
                .changes
                .iter_staged()
                .map(|(p, _)| p.clone())
                .collect();
            paths.push(VirtualPath::from_absolute("/manifest.json").unwrap());

            self.commit_calls.borrow_mut().push(CommitRecord {
                message: request.message.clone(),
                expected_head: request.expected_head.clone(),
                paths,
            });
            self.next_outcome
                .borrow_mut()
                .take()
                .unwrap_or_else(|| Err(StorageError::BadRequest("no outcome queued".into())))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::changes::{ChangeSet, ChangeType};
    use crate::models::FileMetadata;

    #[tokio::test(flavor = "current_thread")]
    async fn mock_records_commit_args() {
        let mut cs = ChangeSet::new();
        let p = VirtualPath::from_absolute("/a.md").unwrap();
        cs.upsert(
            p.clone(),
            ChangeType::CreateFile {
                content: "x".into(),
                meta: FileMetadata::default(),
            },
        );

        let backend = MockBackend::with_success(ScannedSubtree::default(), "sha-new");
        let request = CommitRequest {
            changes: cs,
            merged_snapshot: ScannedSubtree::default(),
            message: "msg".to_string(),
            expected_head: Some("sha-old".to_string()),
            auth_token: None,
        };
        let out = backend.commit(&request).await.unwrap();
        assert_eq!(out.new_head, "sha-new");

        let calls = backend.commit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].message, "msg");
        assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
        assert_eq!(
            calls[0].paths,
            vec![p, VirtualPath::from_absolute("/manifest.json").unwrap()]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_conflict_is_returned() {
        let cs = ChangeSet::new();
        let backend = MockBackend::with_conflict("sha-remote");
        let request = CommitRequest {
            changes: cs,
            merged_snapshot: ScannedSubtree::default(),
            message: "m".to_string(),
            expected_head: None,
            auth_token: None,
        };
        let err = backend.commit(&request).await.unwrap_err();
        assert!(matches!(err, StorageError::Conflict { .. }));
    }
}
