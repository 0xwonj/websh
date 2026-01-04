//! Caching utilities for network requests.
//!
//! Provides sessionStorage-based caching for the current browser session.
//! Cache is automatically cleared when the tab/window is closed,
//! ensuring fresh content on new visits while avoiding redundant
//! fetches during navigation within the same session.

use serde::{de::DeserializeOwned, Serialize};

use super::dom;

/// Cache operation errors.
#[derive(Debug, Clone)]
pub enum CacheError {
    /// sessionStorage not available.
    StorageUnavailable,
    /// Failed to serialize data to JSON.
    SerializationFailed,
    /// Failed to write to storage.
    WriteFailed,
}

/// Get cached data from sessionStorage.
///
/// Returns `None` if the key doesn't exist or deserialization fails.
pub fn get<T: DeserializeOwned>(key: &str) -> Option<T> {
    let storage = dom::session_storage()?;
    let json = storage.get_item(key).ok()??;
    serde_json::from_str(&json).ok()
}

/// Store data in sessionStorage.
pub fn set<T: Serialize>(key: &str, data: &T) -> Result<(), CacheError> {
    let storage = dom::session_storage().ok_or(CacheError::StorageUnavailable)?;
    let json = serde_json::to_string(data).map_err(|_| CacheError::SerializationFailed)?;
    storage
        .set_item(key, &json)
        .map_err(|_| CacheError::WriteFailed)
}

