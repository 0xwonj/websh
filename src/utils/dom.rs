//! DOM and Web API utility functions.
//!
//! Provides safe, consistent access to browser APIs with proper error handling.

use web_sys::{Storage, Window};

/// Get the browser window object.
#[inline]
pub fn window() -> Option<Window> {
    web_sys::window()
}

/// Get localStorage.
#[inline]
pub fn local_storage() -> Option<Storage> {
    window()?.local_storage().ok()?
}

/// Get sessionStorage.
#[inline]
pub fn session_storage() -> Option<Storage> {
    window()?.session_storage().ok()?
}
