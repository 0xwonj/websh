//! Environment variable management using localStorage.
//!
//! User variables are stored with a prefix and can be modified with export/unset.
//! All other localStorage entries are read-only.

use crate::config::{DEFAULT_USER_VARS, USER_VAR_PREFIX, display};
use crate::core::error::EnvironmentError;
use crate::utils::dom;

/// Check if a variable name is valid.
///
/// Valid names must:
/// - Not be empty
/// - Start with a letter or underscore
/// - Contain only alphanumeric characters and underscores
pub fn is_valid_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap();

    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Set a user environment variable.
pub fn set_user_var(key: &str, value: &str) -> Result<(), EnvironmentError> {
    if !is_valid_var_name(key) {
        return Err(EnvironmentError::InvalidVariableName);
    }

    let storage = dom::local_storage().ok_or(EnvironmentError::StorageUnavailable)?;
    let prefixed_key = format!("{}{}", USER_VAR_PREFIX, key);
    storage
        .set_item(&prefixed_key, value)
        .map_err(|_| EnvironmentError::SaveFailed)
}

/// Get a user environment variable.
pub fn get_user_var(key: &str) -> Option<String> {
    let storage = dom::local_storage()?;
    let prefixed_key = format!("{}{}", USER_VAR_PREFIX, key);
    storage.get_item(&prefixed_key).ok()?
}

/// Remove a user environment variable.
pub fn unset_user_var(key: &str) -> Result<(), EnvironmentError> {
    let storage = dom::local_storage().ok_or(EnvironmentError::StorageUnavailable)?;
    let prefixed_key = format!("{}{}", USER_VAR_PREFIX, key);
    storage
        .remove_item(&prefixed_key)
        .map_err(|_| EnvironmentError::RemoveFailed)
}

/// Initialize default user variables if not already set
pub fn init_defaults() {
    for (key, value) in DEFAULT_USER_VARS {
        if get_user_var(key).is_none() {
            let _ = set_user_var(key, value);
        }
    }
}

/// Get all user variables as (key, value) pairs.
pub fn get_all_user_vars() -> Vec<(String, String)> {
    let Some(storage) = dom::local_storage() else {
        return Vec::new();
    };

    let mut vars = Vec::new();
    let len = storage.length().unwrap_or(0);

    for i in 0..len {
        if let Ok(Some(key)) = storage.key(i)
            && let Some(var_name) = key.strip_prefix(USER_VAR_PREFIX)
                && let Ok(Some(value)) = storage.get_item(&key) {
                    vars.push((var_name.to_string(), value));
                }
    }

    vars.sort_by(|a, b| a.0.cmp(&b.0));
    vars
}

/// Get all localStorage entries (raw, no prefix filtering).
pub fn get_all_storage() -> Vec<(String, String)> {
    let Some(storage) = dom::local_storage() else {
        return Vec::new();
    };

    let mut vars = Vec::new();
    let len = storage.length().unwrap_or(0);

    for i in 0..len {
        if let Ok(Some(key)) = storage.key(i)
            && let Ok(Some(value)) = storage.get_item(&key) {
                vars.push((key, value));
            }
    }

    vars.sort_by(|a, b| a.0.cmp(&b.0));
    vars
}

/// Generate .profile content from all localStorage entries
pub fn generate_profile() -> String {
    let mut lines = Vec::new();
    lines.push("# ~/.profile".to_string());
    lines.push(String::new());

    let all_storage = get_all_storage();

    if all_storage.is_empty() {
        lines.push("# No data in localStorage".to_string());
        lines.push("# Use 'export KEY=value' to set variables".to_string());
    } else {
        // Group by prefix
        let mut user_vars = Vec::new();
        let mut other_vars = Vec::new();

        for (key, value) in all_storage {
            if let Some(var_name) = key.strip_prefix(USER_VAR_PREFIX) {
                user_vars.push((var_name.to_string(), value));
            } else {
                other_vars.push((key, value));
            }
        }

        // Show other/system variables first
        if !other_vars.is_empty() {
            lines.push("# System variables (read-only)".to_string());
            for (key, value) in other_vars {
                // Truncate long values for display
                let display_value = if value.len() > display::MAX_VAR_DISPLAY_LEN {
                    format!("{}...", &value[..display::TRUNCATED_PREVIEW_LEN])
                } else {
                    value
                };
                lines.push(format!("{}=\"{}\"", key, display_value));
            }
            lines.push(String::new());
        }

        // Show user variables
        if !user_vars.is_empty() {
            lines.push("# User variables".to_string());
            for (key, value) in user_vars {
                lines.push(format!("export {}=\"{}\"", key, value));
            }
        }
    }

    lines.join("\n")
}

/// Format user variables for `export` command output
pub fn format_export_output() -> Vec<String> {
    let mut lines = Vec::new();
    let user_vars = get_all_user_vars();

    for (key, value) in user_vars {
        lines.push(format!("declare -x {}=\"{}\"", key, value));
    }

    if lines.is_empty() {
        lines.push("# No user variables set".to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_var_names() {
        assert!(is_valid_var_name("FOO"));
        assert!(is_valid_var_name("foo"));
        assert!(is_valid_var_name("_foo"));
        assert!(is_valid_var_name("FOO_BAR"));
        assert!(is_valid_var_name("foo123"));
        assert!(is_valid_var_name("_123"));
        assert!(is_valid_var_name("a"));
        assert!(is_valid_var_name("_"));
    }

    #[test]
    fn test_invalid_var_names() {
        assert!(!is_valid_var_name(""));
        assert!(!is_valid_var_name("123"));
        assert!(!is_valid_var_name("1foo"));
        assert!(!is_valid_var_name("foo-bar"));
        assert!(!is_valid_var_name("foo.bar"));
        assert!(!is_valid_var_name("foo bar"));
        assert!(!is_valid_var_name("foo=bar"));
    }
}
