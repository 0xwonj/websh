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

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthFailed => write!(f, "token invalid or lacks permission"),
            Self::Conflict { remote_head } => {
                write!(f, "remote changed (now {}). run 'sync refresh'",
                    &remote_head[..remote_head.len().min(8)])
            }
            Self::NotFound(p) => write!(f, "path not found on remote: {p}"),
            Self::ValidationFailed(m) => write!(f, "rejected by remote: {m}"),
            Self::RateLimited { retry_after: Some(n) } => write!(f, "rate limited. try again in {n}s"),
            Self::RateLimited { retry_after: None } => write!(f, "rate limited"),
            Self::ServerError(c) => write!(f, "remote server error (HTTP {c})"),
            Self::NetworkError(m) => write!(f, "network error: {m}"),
            Self::NoToken => write!(f, "no GitHub token. run 'sync auth <token>'"),
            Self::BadRequest(m) => write!(f, "bad request: {m}"),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_conflict_truncates_sha_to_8() {
        let e = StorageError::Conflict { remote_head: "abcdef1234567890".to_string() };
        assert_eq!(e.to_string(), "remote changed (now abcdef12). run 'sync refresh'");
    }

    #[test]
    fn display_rate_limited_with_retry() {
        let e = StorageError::RateLimited { retry_after: Some(30) };
        assert_eq!(e.to_string(), "rate limited. try again in 30s");
    }
}
