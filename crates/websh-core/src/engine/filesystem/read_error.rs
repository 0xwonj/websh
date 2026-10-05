use thiserror::Error;

use crate::domain::VirtualPath;
use crate::ports::StorageError;

/// Shared failures from reading backend content or the runtime overlay.
#[derive(Debug, Clone, Error)]
pub enum ContentReadError {
    #[error("no backend for {path}")]
    NoBackend { path: VirtualPath },
    #[error("content changed while reading {path}; retry the request")]
    Obsolete { path: VirtualPath },
    #[error(transparent)]
    Storage(#[from] StorageError),
}
