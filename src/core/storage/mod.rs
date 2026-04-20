//! Storage abstraction for write operations. See spec §4.

mod backend;
mod error;
mod github;
pub mod idb;
pub mod persist;

pub use backend::{BoxFuture, CommitOutcome, StorageBackend};
pub use error::{StorageError, StorageResult};
#[allow(dead_code)]
pub use github::GitHubBackend;

#[cfg(test)]
mod mock;
#[cfg(test)]
pub use mock::MockBackend;
