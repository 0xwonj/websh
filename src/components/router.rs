//! Application router component.
//!
//! Handles URL-based routing with hash history for static hosting.
//! Uses native hashchange events instead of leptos_router for true hash routing.
//!
//! # Architecture
//!
//! - **URL hash is the source of truth**: Navigation state is derived from `#/path`
//! - **Shell never re-renders on navigation**: AppLayout is always mounted
//! - **RendererPage handles content files**: File routes use a stable page shell
//! - **hashchange events**: Browser back/forward buttons work automatically

use std::collections::BTreeMap;

use leptos::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::Closure;

#[cfg(target_arch = "wasm32")]
use crate::app::AppContext;
use crate::components::home::HomePage;
use crate::components::ledger_page::LedgerPage;
use crate::components::ledger_routes::{LEDGER_ROUTE, is_ledger_filter_route_segment};
use crate::components::mempool_editor_page::{MempoolEditorPage, MempoolEditorPageMode};
use crate::components::renderer_page::RendererPage;
use crate::components::terminal::Shell;
#[cfg(target_arch = "wasm32")]
use crate::core::engine::FsEngine;
use crate::core::engine::{
    RenderIntent, ResolvedKind, RouteFrame, RouteRequest, RouteResolution, RouteSurface,
    edit_request_path_inner, is_new_request_path,
};
use crate::models::VirtualPath;
use crate::utils::dom::focus_terminal_input;

// ============================================================================
// Main Router
// ============================================================================

/// Main application router.
///
/// Sets up hash-based routing with the following structure:
/// - `/` and `#/` → built-in homepage
/// - `#/ledger` → merged content ledger
/// - `#/websh/*path` → shell surface at canonical cwd
/// - `#/explorer/*path` → explorer surface at canonical cwd
/// - other `#/*` paths → content route resolution against `/`
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
        if _raw_request.with(is_builtin_home_route) {
            return false;
        }

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
        {move || {
            if _raw_request.with(is_builtin_home_route) {
                return view! {
                    <HomePage route=Memo::new(move |_| {
                        route
                            .get()
                            .unwrap_or_else(|| builtin_home_frame(_raw_request.get()))
                    }) />
                }
                .into_any();
            }
            if _raw_request.with(is_ledger_filter_route) {
                return view! {
                    <LedgerPage route=Memo::new(move |_| ledger_filter_frame(_raw_request.get())) />
                }
                .into_any();
            }
            if _raw_request.with(is_new_request_path) {
                return view! {
                    <RendererPage route=Memo::new(move |_| new_compose_frame()) />
                }
                .into_any();
            }
            if let Some(rest) = _raw_request.with(|r| edit_request_path_inner(r).map(str::to_string))
            {
                return view! {
                    <MempoolEditorPage mode=MempoolEditorPageMode::Edit { request_path: rest } />
                }
                .into_any();
            }

            match route.get() {
                Some(frame) => match frame.intent {
                    RenderIntent::TerminalApp { .. } => {
                        view! { <Shell route=Memo::new(move |_| route.get().expect("frame available")) /> }.into_any()
                    }
                    RenderIntent::DirectoryListing { .. }
                        if frame.surface() == RouteSurface::Explorer => {
                            view! { <Shell route=Memo::new(move |_| route.get().expect("frame available")) /> }.into_any()
                        }
                    RenderIntent::DirectoryListing { .. } => {
                        view! { <LedgerPage route=Memo::new(move |_| route.get().expect("frame available")) /> }.into_any()
                    }
                    _ => {
                        view! { <RendererPage route=Memo::new(move |_| route.get().expect("frame available")) /> }.into_any()
                    }
                },
                None => view! { <NotFound /> }.into_any(),
            }
        }}
    }
}

fn is_builtin_home_route(request: &RouteRequest) -> bool {
    request.url_path == "/"
}

fn is_ledger_filter_route(request: &RouteRequest) -> bool {
    is_ledger_filter_route_segment(request.url_path.trim_matches('/'))
}

fn new_compose_frame() -> RouteFrame {
    let request = RouteRequest::new("/new");
    let node_path = VirtualPath::root();
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path: request.url_path,
            surface: RouteSurface::Content,
            node_path: node_path.clone(),
            kind: ResolvedKind::Document,
            params: BTreeMap::new(),
        },
        intent: RenderIntent::DocumentReader { node_path },
    }
}

fn ledger_filter_frame(request: RouteRequest) -> RouteFrame {
    let request = RouteRequest::new(request.url_path);
    let node_path = if request.url_path.trim_matches('/') == LEDGER_ROUTE {
        VirtualPath::root()
    } else {
        VirtualPath::from_absolute(&request.url_path).unwrap_or_else(|_| VirtualPath::root())
    };
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path: request.url_path,
            surface: RouteSurface::Content,
            node_path: node_path.clone(),
            kind: ResolvedKind::Directory,
            params: BTreeMap::new(),
        },
        intent: RenderIntent::DirectoryListing {
            node_path,
            layout: None,
        },
    }
}

fn builtin_home_frame(request: RouteRequest) -> RouteFrame {
    let request = RouteRequest::new(request.url_path);
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path: request.url_path,
            surface: RouteSurface::Content,
            node_path: VirtualPath::root(),
            kind: ResolvedKind::Directory,
            params: BTreeMap::new(),
        },
        intent: RenderIntent::DirectoryListing {
            node_path: VirtualPath::root(),
            layout: None,
        },
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
