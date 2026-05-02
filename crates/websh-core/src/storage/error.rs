use std::fmt;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
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

// Two variants (Conflict / RateLimited) format dynamically — Conflict
// truncates the remote head to 8 characters and RateLimited switches on the
// presence of a retry-after duration. thiserror's `#[error("...")]` template
// can't express that succinctly, so Display stays hand-written.
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
