//! Storage module for admin write operations.
//!
//! Provides:
//! - [`StorageBackend`] trait for storage abstraction
//! - [`PendingChanges`] and [`ChangeType`] for tracking modifications
//! - [`StorageError`] for error handling
//! - [`GitHubBackend`] for GitHub Contents API integration
//! - [`local`] for LocalStorage persistence

mod backend;
mod error;
mod github;
pub mod local;
mod pending;
mod staged;

pub use backend::{BoxFuture, StorageBackend};
pub use error::{StorageError, StorageResult};
pub use github::GitHubBackend;
pub use local::save_pending_changes;
pub use pending::{ChangeType, ChangesSummary, PendingChange, PendingChanges};
pub use staged::StagedChanges;
