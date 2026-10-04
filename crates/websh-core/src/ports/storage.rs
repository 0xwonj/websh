//! Storage backend port and DTOs shared by runtime engines and adapters.
//!
//! The current contract is intentionally local-task oriented:
//! [`StorageBackendRef`] is an `Rc<dyn StorageBackend>` and futures returned
//! by the trait are not `Send`. That matches the browser/WASM runtime and
//! keeps adapters cheap to clone. Native code that needs cross-thread storage
//! execution should wrap it at the adapter boundary rather than assuming this
//! core port is thread-safe.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::domain::{EntryExtensions, NodeMetadata};

use super::ManifestSnapshotError;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StorageError {
    #[error("public read denied by remote")]
    AuthFailed,
    #[error("path not found on remote: {path}")]
    NotFound { path: String },
    #[error("remote rejected request: {message}")]
    RemoteRejected { message: String },
    #[error("rate limited{}", rate_limit_suffix(retry_after))]
    RateLimited { retry_after: Option<u64> },
    #[error("remote server error: http {status}")]
    Server { status: u16 },
    #[error("network error: {message}")]
    Network { message: String },
    #[error("invalid storage request: {message}")]
    InvalidRequest { message: String },
    #[error("invalid manifest snapshot: {message}")]
    InvalidSnapshot { message: String },
}

fn rate_limit_suffix(retry_after: &Option<u64>) -> String {
    retry_after
        .map(|seconds| format!("; retry after {seconds}s"))
        .unwrap_or_default()
}

impl From<ManifestSnapshotError> for StorageError {
    fn from(source: ManifestSnapshotError) -> Self {
        Self::InvalidSnapshot {
            message: source.to_string(),
        }
    }
}

pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
pub type StorageBackendRef = Rc<dyn StorageBackend>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScannedSubtree {
    pub files: Vec<ScannedFile>,
    pub directories: Vec<ScannedDirectory>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
    pub path: String,
    pub meta: NodeMetadata,
    pub extensions: EntryExtensions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedDirectory {
    pub path: String,
    pub meta: NodeMetadata,
}

pub trait StorageBackend {
    fn backend_type(&self) -> &'static str;

    /// Scan the mount and return its current tree.
    fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>>;

    fn read_text<'a>(&'a self, rel_path: &'a str) -> LocalBoxFuture<'a, StorageResult<String>>;

    fn read_bytes<'a>(&'a self, rel_path: &'a str) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>>;

    /// Return a browser-readable URL for a file when the backend can expose
    /// one directly. Backends that require authenticated/proxied reads should
    /// keep the default and let callers fall back to `read_text`/`read_bytes`.
    fn public_read_url(&self, _rel_path: &str) -> StorageResult<Option<String>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_display_includes_retry_after_when_present() {
        let error = StorageError::RateLimited {
            retry_after: Some(30),
        };

        assert_eq!(error.to_string(), "rate limited; retry after 30s");
    }
}
