//! In-memory backend for commit-path tests. Not shipped in WASM build.

use std::cell::RefCell;

use crate::core::changes::ChangeSet;
use crate::models::{Manifest, VirtualPath};

use super::backend::{BoxFuture, CommitOutcome, StorageBackend};
use super::error::{StorageError, StorageResult};

#[derive(Default)]
pub struct MockBackend {
    pub commit_calls: RefCell<Vec<CommitRecord>>,
    pub next_outcome: RefCell<Option<StorageResult<CommitOutcome>>>,
    pub next_manifest: RefCell<Option<StorageResult<Manifest>>>,
}

pub struct CommitRecord {
    pub message: String,
    pub expected_head: Option<String>,
    pub paths: Vec<VirtualPath>,
}

impl MockBackend {
    pub fn with_success(manifest: Manifest, new_head: impl Into<String>) -> Self {
        let outcome = CommitOutcome {
            new_head: new_head.into(),
            manifest: Some(manifest.clone()),
            committed_paths: vec![],
        };
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(Some(Ok(outcome))),
            next_manifest: RefCell::new(Some(Ok(manifest))),
        }
    }

    pub fn with_conflict(head: impl Into<String>) -> Self {
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(Some(Err(StorageError::Conflict {
                remote_head: head.into(),
            }))),
            next_manifest: RefCell::new(Some(Ok(Manifest::default()))),
        }
    }
}

impl StorageBackend for MockBackend {
    fn backend_type(&self) -> &'static str { "mock" }

    fn commit<'a>(
        &'a self,
        changes: &'a ChangeSet,
        message: &'a str,
        expected_head: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
        let paths: Vec<VirtualPath> = changes
            .iter_staged()
            .map(|(p, _)| p.clone())
            .collect();
        self.commit_calls.borrow_mut().push(CommitRecord {
            message: message.to_string(),
            expected_head: expected_head.map(|s| s.to_string()),
            paths,
        });
        let outcome = self.next_outcome.borrow_mut().take()
            .unwrap_or_else(|| Err(StorageError::BadRequest("no outcome queued".into())));
        Box::pin(async move { outcome })
    }

    fn fetch_manifest(&self) -> BoxFuture<'_, StorageResult<Manifest>> {
        let m = self.next_manifest.borrow_mut().take()
            .unwrap_or_else(|| Ok(Manifest::default()));
        Box::pin(async move { m })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::changes::{ChangeSet, ChangeType};
    use crate::models::{FileMetadata, Manifest};

    #[tokio::test(flavor = "current_thread")]
    async fn mock_records_commit_args() {
        let mut cs = ChangeSet::new();
        let p = VirtualPath::from_absolute("/a.md").unwrap();
        cs.upsert(p.clone(), ChangeType::CreateFile {
            content: "x".into(),
            meta: FileMetadata::default(),
        });

        let backend = MockBackend::with_success(Manifest::default(), "sha-new");
        let out = backend.commit(&cs, "msg", Some("sha-old")).await.unwrap();
        assert_eq!(out.new_head, "sha-new");

        let calls = backend.commit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].message, "msg");
        assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
        assert_eq!(calls[0].paths, vec![p]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_conflict_is_returned() {
        let cs = ChangeSet::new();
        let backend = MockBackend::with_conflict("sha-remote");
        let err = backend.commit(&cs, "m", None).await.unwrap_err();
        assert!(matches!(err, StorageError::Conflict { .. }));
    }
}
