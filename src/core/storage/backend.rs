//! Storage backend trait definition.

use std::future::Future;
use std::pin::Pin;

use super::error::StorageResult;
use super::pending::PendingChanges;
use crate::models::Manifest;

/// A boxed future for async trait methods in WASM.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Trait for storage backends that support write operations.
///
/// All async methods return boxed futures to support object-safe trait usage
/// in WASM environment where async_trait may have limitations.
pub trait StorageBackend {
    /// Get the backend type identifier (e.g., "github", "ipfs").
    fn backend_type(&self) -> &'static str;

    /// Check if the backend is authenticated and ready for writes.
    fn is_authenticated(&self) -> bool;

    /// Create a new file with content.
    fn create_file(
        &self,
        path: &str,
        content: &str,
        message: &str,
    ) -> BoxFuture<'_, StorageResult<()>>;

    /// Update an existing file's content.
    fn update_file(
        &self,
        path: &str,
        content: &str,
        message: &str,
    ) -> BoxFuture<'_, StorageResult<()>>;

    /// Delete a file.
    fn delete_file(&self, path: &str, message: &str) -> BoxFuture<'_, StorageResult<()>>;

    /// Create a new directory.
    /// May be no-op for backends like GitHub where directories are implicit.
    fn create_directory(&self, path: &str) -> BoxFuture<'_, StorageResult<()>>;

    /// Delete a directory (must be empty for most backends).
    fn delete_directory(&self, path: &str, message: &str) -> BoxFuture<'_, StorageResult<()>>;

    /// Commit all pending changes as a batch operation.
    /// Returns the updated manifest after successful commit.
    fn commit(
        &self,
        changes: &PendingChanges,
        message: &str,
    ) -> BoxFuture<'_, StorageResult<Manifest>>;

    /// Get the file SHA/hash (needed for updates on some backends like GitHub).
    fn get_file_sha(&self, path: &str) -> BoxFuture<'_, StorageResult<Option<String>>>;
}
