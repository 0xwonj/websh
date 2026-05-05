//! Storage backend port and DTOs shared by runtime engines and adapters.
//!
//! The current contract is intentionally local-task oriented:
//! [`StorageBackendRef`] is an `Rc<dyn StorageBackend>` and futures returned
//! by the trait are not `Send`. That matches the browser/WASM runtime and
//! keeps adapters cheap to clone. Native code that needs cross-thread storage
//! execution should wrap it at the adapter boundary rather than assuming this
//! core port is thread-safe.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::domain::{EntryExtensions, NodeMetadata, VirtualPath};

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageError {
    AuthFailed,
    Conflict { remote_head: String },
    NotFound(String),
    ValidationFailed(String),
    RateLimited { retry_after: Option<u64> },
    ServerError(u16),
    NetworkError(String),
    NoToken,
    BadRequest(String),
}

// Two variants (Conflict / RateLimited) format dynamically: Conflict truncates
// the remote head and RateLimited switches on the retry-after duration.
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthFailed => write!(f, "token invalid or lacks permission"),
            Self::Conflict { remote_head } => write!(
                f,
                "remote changed (now {}). run 'sync refresh'",
                &remote_head[..remote_head.len().min(8)]
            ),
            Self::NotFound(p) => write!(f, "path not found on remote: {p}"),
            Self::ValidationFailed(m) => write!(f, "rejected by remote: {m}"),
            Self::RateLimited {
                retry_after: Some(n),
            } => write!(f, "rate limited. try again in {n}s"),
            Self::RateLimited { retry_after: None } => write!(f, "rate limited"),
            Self::ServerError(c) => write!(f, "remote server error (HTTP {c})"),
            Self::NetworkError(m) => write!(f, "network error: {m}"),
            Self::NoToken => write!(f, "no GitHub token. run 'sync auth set <token>'"),
            Self::BadRequest(m) => write!(f, "bad request: {m}"),
        }
    }
}

impl std::error::Error for StorageError {}

pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
pub type StorageBackendRef = Rc<dyn StorageBackend>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScannedSubtree {
    pub files: Vec<ScannedFile>,
    pub directories: Vec<ScannedDirectory>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitBase {
    pub snapshot: ScannedSubtree,
    pub expected_head: Option<String>,
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

#[derive(Debug)]
pub struct CommitOutcome {
    pub new_head: String,
    pub committed_paths: Vec<VirtualPath>,
}

#[derive(Clone, Debug)]
pub struct CommitFileAddition {
    pub path: VirtualPath,
    pub content: String,
}

#[derive(Clone, Debug, Default)]
pub struct CommitDelta {
    pub additions: Vec<CommitFileAddition>,
    pub deletions: Vec<VirtualPath>,
}

#[derive(Clone, Debug)]
pub struct CommitRequest {
    pub delta: CommitDelta,
    pub cleanup_paths: Vec<VirtualPath>,
    pub merged_snapshot: ScannedSubtree,
    pub message: String,
    pub expected_head: Option<String>,
    pub auth_token: Option<String>,
}

pub trait StorageBackend {
    fn backend_type(&self) -> &'static str;

    /// Scan the mount and return its current tree.
    fn scan(&self) -> LocalBoxFuture<'_, StorageResult<ScannedSubtree>>;

    /// Return the remote tree that commit preparation should merge against.
    ///
    /// Most backends can use the same path as [`StorageBackend::scan`].
    /// Backends with cached scan reads should override this to return a base
    /// tied to the same optimistic-concurrency token used for the commit.
    fn commit_base(
        &self,
        expected_head: Option<String>,
        _auth_token: Option<String>,
    ) -> LocalBoxFuture<'_, StorageResult<CommitBase>> {
        Box::pin(async move {
            Ok(CommitBase {
                snapshot: self.scan().await?,
                expected_head,
            })
        })
    }

    fn read_text<'a>(&'a self, rel_path: &'a str) -> LocalBoxFuture<'a, StorageResult<String>>;

    fn read_bytes<'a>(&'a self, rel_path: &'a str) -> LocalBoxFuture<'a, StorageResult<Vec<u8>>>;

    /// Return a browser-readable URL for a file when the backend can expose
    /// one directly. Backends that require authenticated/proxied reads should
    /// keep the default and let callers fall back to `read_text`/`read_bytes`.
    fn public_read_url(&self, _rel_path: &str) -> StorageResult<Option<String>> {
        Ok(None)
    }

    /// Commit one prepared atomic batch. Runtime code prepares the merged
    /// metadata snapshot so backend implementations do not assemble filesystems.
    fn commit<'a>(
        &'a self,
        request: &'a CommitRequest,
    ) -> LocalBoxFuture<'a, StorageResult<CommitOutcome>>;
}
