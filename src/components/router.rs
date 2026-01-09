//! Application router component.
//!
//! Handles URL-based routing with hash history for IPFS compatibility.
//! Uses native hashchange events instead of leptos_router for true hash routing.
//!
//! # Architecture
//!
//! - **URL hash is the source of truth**: Navigation state is derived from `#/path`
//! - **Shell never re-renders on navigation**: AppLayout is always mounted
//! - **ReaderOverlay is conditional**: Only shown when URL points to a file
//! - **hashchange events**: Browser back/forward buttons work automatically

use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::Closure;

#[cfg(target_arch = "wasm32")]
use crate::app::AppContext;
use crate::components::reader::Reader;
use crate::components::terminal::Shell;
use crate::components::terminal::shell::OVERLAY_CLASS;
use crate::models::AppRoute;
use crate::utils::dom::focus_terminal_input;

// ============================================================================
// Main Router
// ============================================================================

/// Main application router.
///
/// Sets up hash-based routing with the following structure:
/// - `#/` → Root (mount selection)
/// - `#/~/` → Home mount directory (Browse)
/// - `#/~/path/` → Browse directory
/// - `#/~/path/file.ext` → Read file (with overlay)
#[component]
pub fn AppRouter() -> impl IntoView {
    // Create route signal from current URL hash
    let route = RwSignal::new(AppRoute::current());

    // Set up hashchange event listener (runs once on mount)
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let closure = Closure::wrap(Box::new(move || {
            route.set(AppRoute::current());
        }) as Box<dyn Fn()>);

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
        }

        // Keep the closure alive for the lifetime of the app
        closure.forget();
    }

    // Note: Root is now a valid route showing mount selection
    // No redirect needed

    // Focus terminal input when returning from reader overlay
    Effect::new(move |prev_was_file: Option<bool>| {
        let is_file = route.get().is_file();
        // If we were viewing a file and now we're not, focus the terminal input
        if prev_was_file == Some(true) && !is_file {
            focus_terminal_input();
        }
        is_file
    });

    // Convert to Memo for Shell (which expects Memo<AppRoute>)
    let route_memo = Memo::new(move |_| route.get());

    view! {
        // Shell is always rendered (stable across route changes)
        <Shell route=route_memo />

        // ReaderOverlay is shown only for file routes
        <Show when=move || route.get().is_file()>
            <ReaderOverlay route=route_memo />
        </Show>
    }
}

// ============================================================================
// Reader Overlay
// ============================================================================

/// Overlay component for reading files.
///
/// Renders on top of Shell when the current route is a file.
/// Closes by navigating to the parent directory.
#[component]
fn ReaderOverlay(route: Memo<AppRoute>) -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Close handler - navigate to parent directory, push current file to forward stack
    let on_close = Callback::new(move |_: ()| {
        let current = route.get();
        #[cfg(target_arch = "wasm32")]
        ctx.explorer.push_forward(current.clone());
        current.parent().push();
    });

    view! {
        <div class=OVERLAY_CLASS>
            <Reader route=route on_close=on_close />
        </div>
    }
}
