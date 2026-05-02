//! Storage abstraction for write operations. See spec §4.

mod backend;
#[cfg(target_arch = "wasm32")]
pub mod boot;
mod error;
pub mod github;
#[cfg(target_arch = "wasm32")]
pub mod idb;
#[cfg(target_arch = "wasm32")]
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
