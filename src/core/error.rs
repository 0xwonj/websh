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
