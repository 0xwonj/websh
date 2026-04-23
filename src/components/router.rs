//! Application router component.
//!
//! Handles URL-based routing with hash history for IPFS deployment.
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
#[cfg(target_arch = "wasm32")]
use crate::core::engine::FsEngine;
use crate::core::engine::{
    RenderIntent, RouteFrame, RouteRequest, parent_request_path, push_request_path,
};
use crate::utils::dom::focus_terminal_input;

// ============================================================================
// Main Router
// ============================================================================

/// Main application router.
///
/// Sets up hash-based routing with the following structure:
/// - `#/` → site/public route resolution
/// - `#/shell` → shell entrypoint at `/site`
/// - `#/fs/*path` → canonical filesystem browsing namespace
#[component]
pub fn RouterView() -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Raw request from URL hash (updated on hashchange).
    let _raw_request = RwSignal::new(RouteRequest::current());

    // Set up hashchange event listener (runs once on mount).
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        let closure = Closure::wrap(Box::new(move || {
            _raw_request.set(RouteRequest::current());
        }) as Box<dyn Fn()>);

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());
        }

        // Keep the closure alive for the lifetime of the app
        closure.forget();
    }

    // Resolved route frame: re-runs whenever the hash changes OR fs loads/changes.
    #[cfg(target_arch = "wasm32")]
    let route = Memo::new(move |_| {
        let request = _raw_request.get();
        ctx.view_global_fs.with(|fs| {
            let resolution = fs.resolve_route(&request)?;
            let intent = fs.build_render_intent(&resolution)?;
            Some(RouteFrame {
                request,
                resolution,
                intent,
            })
        })
    });
    #[cfg(not(target_arch = "wasm32"))]
    let route = Memo::new(move |_| None::<RouteFrame>);

    // Focus terminal input when returning to a shell/explorer surface.
    Effect::new(move |prev_was_reader: Option<bool>| {
        let is_reader = route.get().is_some_and(|frame| {
            !matches!(
                frame.intent,
                RenderIntent::TerminalApp { .. } | RenderIntent::DirectoryListing { .. }
            )
        });
        if prev_was_reader == Some(true) && !is_reader {
            focus_terminal_input();
        }
        is_reader
    });

    view! {
        {move || match route.get() {
            Some(frame) => match frame.intent {
                RenderIntent::TerminalApp { .. } | RenderIntent::DirectoryListing { .. } => {
                    view! { <Shell route=Memo::new(move |_| route.get().expect("frame available")) /> }.into_any()
                }
                _ => {
                    view! { <ReaderOverlay route=Memo::new(move |_| route.get().expect("frame available")) /> }.into_any()
                }
            },
            None => view! { <NotFound /> }.into_any(),
        }}
    }
}

// ============================================================================
// Reader Overlay
// ============================================================================

/// Overlay component for reading files.
///
/// Renders a reader-like surface for non-shell intents.
#[component]
fn ReaderOverlay(route: Memo<RouteFrame>) -> impl IntoView {
    // Close handler - navigate to parent directory
    let on_close = Callback::new(move |_: ()| {
        push_request_path(&parent_request_path(&route.get().request.url_path));
    });

    view! {
        <div class=OVERLAY_CLASS>
            <Reader route=route on_close=on_close />
        </div>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <div style="padding: 2rem; font-family: monospace;">
            <h1>"404"</h1>
            <p>"No route matched the current path."</p>
        </div>
    }
}
