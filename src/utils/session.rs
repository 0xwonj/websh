//! Session-scoped GitHub token helpers backed by runtime `/state`.

/// Reads the current GitHub token from the runtime state store.
pub fn get_gh_token() -> Option<String> {
    crate::core::runtime::state::get_github_token()
}

/// Writes the GitHub token into the runtime state store.
pub fn set_gh_token(token: &str) {
    crate::core::runtime::state::set_github_token(token);
}

/// Removes the GitHub token from the runtime state store.
pub fn clear_gh_token() {
    crate::core::runtime::state::clear_github_token();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_key_is_websh_gh_token() {
        crate::core::runtime::state::reset_for_tests();
        set_gh_token("abc");
        assert_eq!(get_gh_token().as_deref(), Some("abc"));
    }

    #[test]
    fn get_token_returns_none_outside_browser() {
        crate::core::runtime::state::reset_for_tests();
        assert_eq!(get_gh_token(), None);
    }

    #[test]
    fn set_token_does_not_panic_outside_browser() {
        crate::core::runtime::state::reset_for_tests();
        set_gh_token("abc");
        assert_eq!(get_gh_token().as_deref(), Some("abc"));
    }

    #[test]
    fn clear_token_does_not_panic_outside_browser() {
        crate::core::runtime::state::reset_for_tests();
        set_gh_token("abc");
        clear_gh_token();
        assert_eq!(get_gh_token(), None);
    }
}
