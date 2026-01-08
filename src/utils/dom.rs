//! DOM and Web API utility functions.
//!
//! Provides safe, consistent access to browser APIs with proper error handling.

use wasm_bindgen::JsCast;
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

/// Focus an element by CSS selector.
///
/// Returns `true` if the element was found and focused successfully.
pub fn focus_element(selector: &str) -> bool {
    if let Some(window) = window()
        && let Some(document) = window.document()
        && let Some(element) = document.query_selector(selector).ok().flatten()
        && let Ok(html_element) = element.dyn_into::<web_sys::HtmlElement>()
    {
        html_element.focus().is_ok()
    } else {
        false
    }
}

/// Focus the terminal input element.
///
/// Convenience wrapper around `focus_element("input")`.
#[inline]
pub fn focus_terminal_input() {
    focus_element("input");
}

/// Check if the device is mobile or tablet based on screen width.
///
/// Uses a breakpoint of 768px (common tablet/desktop threshold).
pub fn is_mobile_or_tablet() -> bool {
    window()
        .and_then(|w| w.inner_width().ok())
        .and_then(|w| w.as_f64())
        .is_some_and(|width| width < 768.0)
}

// =============================================================================
// Browser Navigation
// =============================================================================

/// Get the current URL hash (without the '#' prefix).
pub fn get_hash() -> String {
    window()
        .and_then(|w| w.location().hash().ok())
        .unwrap_or_default()
        .trim_start_matches('#')
        .to_string()
}

/// Set the URL hash (adds to browser history).
///
/// The hash should include the '#' prefix.
pub fn set_hash(hash: &str) {
    if let Some(window) = window() {
        let _ = window.location().set_hash(hash);
    }
}

/// Replace the URL hash without adding to browser history.
///
/// The hash should include the '#' prefix.
/// Useful for redirects that shouldn't appear in back button history.
pub fn replace_hash(hash: &str) {
    if let Some(window) = window()
        && let Ok(history) = window.history()
    {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(hash));
    }
}
