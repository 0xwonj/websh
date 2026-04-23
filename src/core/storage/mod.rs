//! Storage abstraction for write operations. See spec §4.

mod backend;
pub(crate) mod boot;
mod error;
pub(crate) mod github;
pub mod idb;
pub mod persist;

pub use backend::{
    BoxFuture, CommitDelta, CommitFileAddition, CommitOutcome, CommitRequest, ScannedDirectory,
    ScannedFile, ScannedSubtree, StorageBackend,
};
pub use error::{StorageError, StorageResult};

#[cfg(any(test, feature = "mock"))]
mod mock;
#[cfg(any(test, feature = "mock"))]
pub use mock::MockBackend;
