//! Environment variable management backed by the runtime `/.websh/state` model.

use crate::config::{DEFAULT_USER_VARS, USER_VAR_PREFIX};
use crate::core::error::EnvironmentError;
use crate::core::runtime::{RuntimeStateSnapshot, state};

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
pub fn set_user_var(key: &str, value: &str) -> Result<RuntimeStateSnapshot, EnvironmentError> {
    if !is_valid_var_name(key) {
        return Err(EnvironmentError::InvalidVariableName);
    }

    state::set_env_var(key, value)
}

/// Get a user environment variable.
pub fn get_user_var(key: &str) -> Option<String> {
    state::get_env_var(key)
}

/// Remove a user environment variable.
pub fn unset_user_var(key: &str) -> Result<RuntimeStateSnapshot, EnvironmentError> {
    state::unset_env_var(key)
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
    let mut vars = state::all_env_vars();
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    vars
}

/// Get all runtime-managed storage entries.
pub fn get_all_storage() -> Vec<(String, String)> {
    let snapshot = state::snapshot();
    let mut vars = snapshot
        .env
        .into_iter()
        .map(|(key, value)| (format!("{USER_VAR_PREFIX}{key}"), value))
        .collect::<Vec<_>>();

    vars.sort_by(|a, b| a.0.cmp(&b.0));
    vars
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
