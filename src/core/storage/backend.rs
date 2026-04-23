//! Storage backend trait. Concrete impls live in `mock.rs`, `github.rs`.
//! See spec §4.

use std::future::Future;
use std::pin::Pin;

use crate::core::changes::ChangeSet;
use crate::models::{DirectoryMetadata, FileMetadata, VirtualPath};

use super::error::StorageResult;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScannedSubtree {
    pub files: Vec<ScannedFile>,
    pub directories: Vec<ScannedDirectory>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
    pub path: String,
    pub description: String,
    pub meta: FileMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedDirectory {
    pub path: String,
    pub meta: DirectoryMetadata,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct CommitOutcome {
    pub new_head: String,
    pub committed_paths: Vec<VirtualPath>,
}

#[derive(Clone, Debug)]
pub struct CommitRequest {
    pub changes: ChangeSet,
    pub merged_snapshot: ScannedSubtree,
    pub message: String,
    pub expected_head: Option<String>,
    pub auth_token: Option<String>,
}

#[allow(dead_code)]
pub trait StorageBackend {
    fn backend_type(&self) -> &'static str;

    fn scan(&self) -> BoxFuture<'_, StorageResult<ScannedSubtree>>;

    fn read_text<'a>(&'a self, rel_path: &'a str) -> BoxFuture<'a, StorageResult<String>>;

    fn read_bytes<'a>(&'a self, rel_path: &'a str) -> BoxFuture<'a, StorageResult<Vec<u8>>>;

    /// Commit one prepared atomic batch. Runtime code prepares the merged
    /// metadata snapshot so backend implementations do not assemble filesystems.
    fn commit<'a>(
        &'a self,
        request: &'a CommitRequest,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>>;
}
