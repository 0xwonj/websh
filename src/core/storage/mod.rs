//! Storage abstraction for write operations. See spec §4.

mod backend;
pub mod boot;
mod error;
pub(crate) mod github;
pub mod idb;
pub mod persist;

pub use backend::{
    BoxFuture, CommitOutcome, CommitRequest, ScannedDirectory, ScannedFile, ScannedSubtree,
    StorageBackend,
};
pub use error::{StorageError, StorageResult};
#[allow(dead_code)]
pub use github::GitHubBackend;

#[cfg(any(test, feature = "mock"))]
mod mock;
#[cfg(any(test, feature = "mock"))]
pub use mock::MockBackend;
