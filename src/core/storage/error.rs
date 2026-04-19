//! Storage-related error types.

use std::fmt;

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Storage operation errors.
#[derive(Debug, Clone)]
pub enum StorageError {
    /// Not authenticated (no token or expired)
    NotAuthenticated,
    /// Permission denied (not admin)
    PermissionDenied,
    /// File/directory already exists
    AlreadyExists(String),
    /// File/directory not found
    NotFound(String),
    /// Directory not empty (for delete)
    DirectoryNotEmpty(String),
    /// Invalid path format
    InvalidPath(String),
    /// Network/API error
    NetworkError(String),
    /// Rate limited by backend
    RateLimited,
    /// Conflict (file changed since last read)
    Conflict(String),
    /// Backend-specific error
    BackendError(String),
    /// LocalStorage error
    LocalStorage(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAuthenticated => write!(f, "Not authenticated"),
            Self::PermissionDenied => write!(f, "Permission denied"),
            Self::AlreadyExists(p) => write!(f, "Already exists: {}", p),
            Self::NotFound(p) => write!(f, "Not found: {}", p),
            Self::DirectoryNotEmpty(p) => write!(f, "Directory not empty: {}", p),
            Self::InvalidPath(p) => write!(f, "Invalid path: {}", p),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::RateLimited => write!(f, "Rate limited, please try again later"),
            Self::Conflict(p) => write!(f, "Conflict: {} was modified", p),
            Self::BackendError(msg) => write!(f, "Backend error: {}", msg),
            Self::LocalStorage(msg) => write!(f, "LocalStorage error: {}", msg),
        }
    }
}

impl std::error::Error for StorageError {}
