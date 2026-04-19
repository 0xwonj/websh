//! LocalStorage persistence for pending changes and tokens.

use crate::config::{GITHUB_TOKEN_KEY, PENDING_CHANGES_KEY};

use super::error::{StorageError, StorageResult};
use super::pending::PendingChanges;

/// Save pending changes to localStorage.
pub fn save_pending_changes(changes: &PendingChanges) -> StorageResult<()> {
    let storage = get_local_storage()?;
    let json = serde_json::to_string(changes)
        .map_err(|e| StorageError::LocalStorage(e.to_string()))?;
    storage.set_item(PENDING_CHANGES_KEY, &json).map_err(|_| {
        StorageError::LocalStorage("Failed to save pending changes".to_string())
    })?;
    Ok(())
}

/// Load pending changes from localStorage.
pub fn load_pending_changes() -> Option<PendingChanges> {
    let storage = get_local_storage().ok()?;
    let json = storage.get_item(PENDING_CHANGES_KEY).ok()??;
    serde_json::from_str(&json).ok()
}

/// Clear pending changes from localStorage.
pub fn clear_pending_changes() -> StorageResult<()> {
    let storage = get_local_storage()?;
    storage.remove_item(PENDING_CHANGES_KEY).map_err(|_| {
        StorageError::LocalStorage("Failed to clear pending changes".to_string())
    })?;
    Ok(())
}

/// Store GitHub token in localStorage.
pub fn store_github_token(token: &str) -> StorageResult<()> {
    let storage = get_local_storage()?;
    storage
        .set_item(GITHUB_TOKEN_KEY, token)
        .map_err(|_| StorageError::LocalStorage("Failed to store token".to_string()))?;
    Ok(())
}

/// Retrieve stored GitHub token.
pub fn get_github_token() -> Option<String> {
    let storage = get_local_storage().ok()?;
    storage.get_item(GITHUB_TOKEN_KEY).ok()?
}

/// Clear stored GitHub token.
pub fn clear_github_token() -> StorageResult<()> {
    let storage = get_local_storage()?;
    storage
        .remove_item(GITHUB_TOKEN_KEY)
        .map_err(|_| StorageError::LocalStorage("Failed to clear token".to_string()))?;
    Ok(())
}

/// Check if GitHub token is stored.
pub fn has_github_token() -> bool {
    get_github_token().is_some()
}

/// Get localStorage handle.
fn get_local_storage() -> StorageResult<web_sys::Storage> {
    let window =
        web_sys::window().ok_or(StorageError::LocalStorage("No window".to_string()))?;
    window
        .local_storage()
        .map_err(|_| StorageError::LocalStorage("localStorage not available".to_string()))?
        .ok_or(StorageError::LocalStorage(
            "localStorage is null".to_string(),
        ))
}

#[cfg(test)]
mod tests {
    // LocalStorage tests require browser environment
}
