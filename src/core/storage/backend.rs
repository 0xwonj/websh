//! Storage backend trait. Concrete impls live in `mock.rs`, `github.rs`.
//! See spec §4.

use std::future::Future;
use std::pin::Pin;

use crate::core::changes::ChangeSet;
use crate::models::{Manifest, VirtualPath};

use super::error::StorageResult;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[allow(dead_code)]
#[derive(Debug)]
pub struct CommitOutcome {
    pub new_head: String,
    /// Some if the backend produced a manifest synchronously as part of the
    /// commit response. `GitHubBackend` returns `None` (GraphQL's
    /// `createCommitOnBranch` does not echo file contents); the dispatcher
    /// re-fetches via `fetch_manifest()` after a successful commit.
    pub manifest: Option<Manifest>,
    pub committed_paths: Vec<VirtualPath>,
}

#[allow(dead_code)]
pub trait StorageBackend {
    fn backend_type(&self) -> &'static str;

    /// Commit the staged subset of `changes` as one atomic batch.
    /// `expected_head` is the SHA the caller believed was current at draft-time.
    fn commit<'a>(
        &'a self,
        changes: &'a ChangeSet,
        message: &'a str,
        expected_head: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>>;

    fn fetch_manifest(&self) -> BoxFuture<'_, StorageResult<Manifest>>;
}
