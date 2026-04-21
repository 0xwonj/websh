//! Custom error types for the application.
//!
//! Provides structured error handling with meaningful error messages
//! and proper error categorization for each domain:
//!
//! - [`WalletError`] - MetaMask/wallet connection and request errors
//! - [`EnvironmentError`] - localStorage operations for environment variables
//! - [`FetchError`] - Network/fetch-related errors for HTTP requests

use std::fmt;

/// Wallet-related errors for MetaMask/EIP-1193 integration.
#[derive(Debug, Clone)]
pub enum WalletError {
    /// Browser window not available
    NoWindow,
    /// MetaMask or compatible wallet not installed
    NotInstalled,
    /// Failed to create request object
    RequestCreationFailed,
    /// Request to wallet was rejected by user
    RequestRejected(String),
    /// No account returned from wallet
    NoAccount,
}

impl fmt::Display for WalletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWindow => write!(f, "Browser window not available"),
            Self::NotInstalled => write!(
                f,
                "MetaMask not installed. Please install MetaMask extension."
            ),
            Self::RequestCreationFailed => write!(f, "Failed to create wallet request"),
            Self::RequestRejected(msg) => write!(f, "Wallet request rejected: {}", msg),
            Self::NoAccount => write!(f, "No account returned from wallet"),
        }
    }
}

impl std::error::Error for WalletError {}

/// Environment variable errors for localStorage operations.
#[derive(Debug, Clone)]
pub enum EnvironmentError {
    /// localStorage not available.
    StorageUnavailable,
    /// Invalid variable name.
    InvalidVariableName,
    /// Failed to save to localStorage.
    SaveFailed,
    /// Failed to remove from localStorage.
    RemoveFailed,
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StorageUnavailable => write!(f, "localStorage not available"),
            Self::InvalidVariableName => {
                write!(
                    f,
                    "invalid variable name (use letters, numbers, underscores)"
                )
            }
            Self::SaveFailed => write!(f, "failed to save to localStorage"),
            Self::RemoveFailed => write!(f, "failed to remove from localStorage"),
        }
    }
}

impl std::error::Error for EnvironmentError {}

/// Network/fetch-related errors for HTTP requests.
#[derive(Debug, Clone)]
pub enum FetchError {
    /// Browser window not available
    NoWindow,
    /// Failed to create HTTP request
    RequestCreationFailed,
    /// Network request failed (timeout, CORS, etc.)
    NetworkError(String),
    /// HTTP error response (non-2xx status)
    HttpError(u16),
    /// Failed to read response body
    ResponseReadFailed,
    /// Invalid response content (not text)
    InvalidContent,
    /// JSON parsing error
    JsonParseError(String),
    /// Request timed out
    Timeout,
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWindow => write!(f, "Browser window not available"),
            Self::RequestCreationFailed => write!(f, "Failed to create request"),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::HttpError(status) => write!(f, "HTTP error: {}", status),
            Self::ResponseReadFailed => write!(f, "Failed to read response"),
            Self::InvalidContent => write!(f, "Invalid response content"),
            Self::JsonParseError(msg) => write!(f, "JSON parse error: {}", msg),
            Self::Timeout => write!(f, "Request timed out"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Unified application error wrapping the three domain errors.
///
/// Use this when code needs to propagate errors across domain boundaries
/// (e.g., a function that may hit both fetch and environment failures).
/// Each domain-specific error type remains preferred within its own module.
///
/// Implements `From` for each domain error to enable `?` across boundaries.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppError {
    Wallet(WalletError),
    Fetch(FetchError),
    Environment(EnvironmentError),
    Storage(crate::core::storage::StorageError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wallet(e) => write!(f, "{}", e),
            Self::Fetch(e) => write!(f, "{}", e),
            Self::Environment(e) => write!(f, "{}", e),
            Self::Storage(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wallet(e) => Some(e),
            Self::Fetch(e) => Some(e),
            Self::Environment(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

impl From<WalletError> for AppError {
    fn from(e: WalletError) -> Self {
        Self::Wallet(e)
    }
}

impl From<FetchError> for AppError {
    fn from(e: FetchError) -> Self {
        Self::Fetch(e)
    }
}

impl From<EnvironmentError> for AppError {
    fn from(e: EnvironmentError) -> Self {
        Self::Environment(e)
    }
}

impl From<crate::core::storage::StorageError> for AppError {
    fn from(e: crate::core::storage::StorageError) -> Self {
        Self::Storage(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_from_wallet() {
        let wallet_err = WalletError::NoWindow;
        let app_err: AppError = wallet_err.into();
        assert!(matches!(app_err, AppError::Wallet(WalletError::NoWindow)));
    }

    #[test]
    fn test_app_error_from_fetch() {
        let fetch_err = FetchError::HttpError(404);
        let app_err: AppError = fetch_err.into();
        assert!(matches!(app_err, AppError::Fetch(FetchError::HttpError(404))));
    }

    #[test]
    fn test_app_error_from_environment() {
        let env_err = EnvironmentError::InvalidVariableName;
        let app_err: AppError = env_err.into();
        assert!(matches!(
            app_err,
            AppError::Environment(EnvironmentError::InvalidVariableName)
        ));
    }

    #[test]
    fn test_app_error_display_delegates() {
        let app_err = AppError::Fetch(FetchError::HttpError(500));
        assert_eq!(app_err.to_string(), "HTTP error: 500");
    }

    #[test]
    fn test_app_error_source_chain() {
        let app_err = AppError::Wallet(WalletError::NoAccount);
        let source = std::error::Error::source(&app_err);
        assert!(source.is_some());
    }

    #[test]
    fn test_app_error_from_storage() {
        let storage_err = crate::core::storage::StorageError::NoToken;
        let app_err: AppError = storage_err.into();
        assert!(matches!(
            app_err,
            AppError::Storage(crate::core::storage::StorageError::NoToken)
        ));
    }
}
