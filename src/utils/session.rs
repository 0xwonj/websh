//! Session-scoped GitHub token storage.
//!
//! Uses `window.sessionStorage` — the token survives reloads within the
//! same tab and clears on tab close. XSS exposure is acknowledged (spec
//! §8.3): any script running in this origin can read the token, so only
//! narrowly-scoped tokens should be used.
//!
//! All helpers silently no-op (or return `None`) when the storage is
//! unavailable — either because this code runs outside a browser (e.g.
//! in `cargo test`) or because the browser refuses access. The caller
//! cannot distinguish "key missing" from "storage unavailable", which
//! is deliberate.

use crate::utils::dom::session_storage;

const KEY: &str = "websh.gh_token";

/// Reads the GitHub token from `sessionStorage`.
///
/// Returns `None` if sessionStorage is unavailable (non-browser test
/// env, or browser refusal) or the key is absent.
pub fn get_gh_token() -> Option<String> {
    session_storage()?.get_item(KEY).ok().flatten()
}

/// Writes the GitHub token into `sessionStorage`.
///
/// Silently no-ops if sessionStorage is unavailable.
pub fn set_gh_token(token: &str) {
    if let Some(s) = session_storage() {
        let _ = s.set_item(KEY, token);
    }
}

/// Removes the GitHub token from `sessionStorage`.
///
/// Silently no-ops if sessionStorage is unavailable.
pub fn clear_gh_token() {
    if let Some(s) = session_storage() {
        let _ = s.remove_item(KEY);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_key_is_websh_gh_token() {
        assert_eq!(KEY, "websh.gh_token");
    }

    #[test]
    fn get_token_returns_none_outside_browser() {
        assert_eq!(get_gh_token(), None);
    }

    #[test]
    fn set_token_does_not_panic_outside_browser() {
        set_gh_token("abc");
    }

    #[test]
    fn clear_token_does_not_panic_outside_browser() {
        clear_gh_token();
    }
}
