//! DOM and Web API helpers. Public functions degrade to `None` / no-ops
//! on non-wasm targets so callers can write target-agnostic code.
//!
//! `Window` and `Storage` re-export wasm types only on wasm32; callers
//! that need to invoke methods on those types must themselves be inside
//! `#[cfg(target_arch = "wasm32")]` blocks.

#[cfg(target_arch = "wasm32")]
pub use web_sys::{Storage, Window};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

/// Return the browser window. Always `None` on non-wasm targets.
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn window() -> Option<Window> {
    web_sys::window()
}
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn window() -> Option<()> {
    None
}

/// Return localStorage. Always `None` on non-wasm targets.
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn local_storage() -> Option<Storage> {
    window()?.local_storage().ok()?
}
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn local_storage() -> Option<()> {
    None
}

/// Return sessionStorage. Always `None` on non-wasm targets.
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn session_storage() -> Option<Storage> {
    window()?.session_storage().ok()?
}
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn session_storage() -> Option<()> {
    None
}

/// Focus an element by CSS selector. Returns `true` on success.
pub fn focus_element(_selector: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = window()
            && let Some(document) = window.document()
            && let Some(element) = document.query_selector(_selector).ok().flatten()
            && let Ok(html_element) = element.dyn_into::<web_sys::HtmlElement>()
        {
            html_element.focus().is_ok()
        } else {
            false
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

#[inline]
pub fn focus_terminal_input() {
    focus_element("input");
}

/// Current URL hash without the leading `#`.
pub fn get_hash() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default()
            .trim_start_matches('#')
            .to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

/// Set the URL hash (adds a history entry).
pub fn set_hash(_hash: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = window() {
        let _ = window.location().set_hash(_hash);
    }
}

/// Replace the URL hash without adding to browser history.
pub fn replace_hash(_hash: &str) {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = window()
        && let Ok(history) = window.history()
    {
        let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(_hash));
    }
}

/// Dispatch a synthetic `hashchange` event. `history.replaceState` does
/// not fire one per the HTML spec.
pub fn dispatch_hashchange() {
    #[cfg(target_arch = "wasm32")]
    if let Some(window) = window()
        && let Ok(event) = web_sys::Event::new("hashchange")
    {
        let _ = window.dispatch_event(&event);
    }
}
