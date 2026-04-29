//! Application router component.
//!
//! Handles URL-based routing with hash history for static hosting.
//! Uses native hashchange events instead of leptos_router for true hash routing.
//!
//! # Architecture
//!
//! - **URL hash is the source of truth**: Navigation state is derived from `#/path`
//! - **Shell never re-renders on navigation**: AppLayout is always mounted
//! - **Reader handles content files**: File routes use a stable page shell
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
use crate::components::reader::{Reader, ReaderFrame};
use crate::components::terminal::Shell;

/// URL patterns that bypass the engine and produce a synthetic [`RouteFrame`].
///
/// Each variant corresponds to a reserved URL prefix (or full path) the
/// router handles directly. The engine never resolves these — it does not
/// know about UI-level concerns like compose mode or ledger filter views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinRoute {
    /// `/` — homepage.
    Home,
    /// `/ledger` and `/<category>` — ledger filter views.
    LedgerFilter,
    /// `/new` — mempool compose flow.
    NewCompose,
}

impl BuiltinRoute {
    /// Classify a request against the reserved URL list. Returns `None`
    /// when the request is to be routed through the engine.
    pub fn detect(request: &RouteRequest) -> Option<Self> {
        if request.url_path == "/" {
            return Some(Self::Home);
        }
        if is_ledger_filter_route_segment(request.url_path.trim_matches('/')) {
            return Some(Self::LedgerFilter);
        }
        if is_new_request_path(request) {
            return Some(Self::NewCompose);
        }
        None
    }
}
#[cfg(target_arch = "wasm32")]
use crate::core::engine::FsEngine;
use crate::core::engine::{
    RenderIntent, ResolvedKind, RouteFrame, RouteRequest, RouteResolution, RouteSurface,
    is_new_request_path,
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

    install_terminal_focus_effect(_raw_request, route);

    view! {
        {move || {
            let request = _raw_request.get();
            match BuiltinRoute::detect(&request) {
                Some(BuiltinRoute::Home) => view! {
                    <HomePage route=Memo::new(move |_| {
                        route
                            .get()
                            .unwrap_or_else(|| home_frame(_raw_request.get()))
                    }) />
                }
                .into_any(),
                Some(BuiltinRoute::LedgerFilter) => view! {
                    <LedgerPage route=Memo::new(move |_| ledger_filter_frame(_raw_request.get())) />
                }
                .into_any(),
                Some(BuiltinRoute::NewCompose) => {
                    let reader_frame = ReaderFrame::try_from(new_compose_frame())
                        .expect("compose route always produces a Reader-bound intent");
                    view! {
                        <Reader frame=Memo::new(move |_| reader_frame.clone()) />
                    }
                    .into_any()
                }
                None => match route.get() {
                    Some(frame) => match frame.intent {
                        RenderIntent::TerminalApp { .. } => {
                            view! { <Shell route=static_route_memo(frame.clone()) /> }.into_any()
                        }
                        RenderIntent::DirectoryListing { .. }
                            if frame.surface() == RouteSurface::Explorer => {
                                view! { <Shell route=static_route_memo(frame.clone()) /> }.into_any()
                            }
                        RenderIntent::DirectoryListing { .. } => {
                            view! { <LedgerPage route=static_route_memo(frame.clone()) /> }.into_any()
                        }
                        RenderIntent::HtmlContent { .. }
                        | RenderIntent::MarkdownContent { .. }
                        | RenderIntent::PlainContent { .. }
                        | RenderIntent::Asset { .. }
                        | RenderIntent::Redirect { .. } => {
                            let reader_frame = ReaderFrame::try_from(frame)
                                .expect("non-surface RenderIntent variants convert to ReaderFrame");
                            view! {
                                <Reader frame=Memo::new(move |_| reader_frame.clone()) />
                            }
                            .into_any()
                        }
                    },
                    None => view! { <NotFound /> }.into_any(),
                }
            }
        }}
    }
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
        intent: RenderIntent::MarkdownContent {
            node_path,
        },
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
        },
    }
}

fn home_frame(request: RouteRequest) -> RouteFrame {
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
        },
    }
}

/// Wraps a concrete [`RouteFrame`] in a [`Memo`] so it can be passed to a
/// component that expects a reactive prop, without each call site repeating
/// the `Option`-unwrap-and-`expect` dance against the outer route Memo.
fn static_route_memo(frame: RouteFrame) -> Memo<RouteFrame> {
    Memo::new(move |_| frame.clone())
}

/// Refocuses the terminal input when the user returns to a shell/explorer
/// surface from a Reader-bound surface. Lives in its own helper so the
/// router body doesn't carry the cross-cutting concern inline.
fn install_terminal_focus_effect(
    raw_request: RwSignal<RouteRequest>,
    route: Memo<Option<RouteFrame>>,
) {
    Effect::new(move |prev_was_reader: Option<bool>| {
        if matches!(
            BuiltinRoute::detect(&raw_request.get()),
            Some(BuiltinRoute::Home)
        ) {
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

#[cfg(test)]
mod builtin_route_tests {
    use super::*;

    #[test]
    fn detects_home() {
        assert_eq!(
            BuiltinRoute::detect(&RouteRequest::new("/")),
            Some(BuiltinRoute::Home)
        );
    }

    #[test]
    fn detects_ledger_root() {
        assert_eq!(
            BuiltinRoute::detect(&RouteRequest::new("/ledger")),
            Some(BuiltinRoute::LedgerFilter)
        );
    }

    #[test]
    fn detects_ledger_category() {
        for category in ["writing", "projects", "papers", "talks", "misc"] {
            let path = format!("/{category}");
            assert_eq!(
                BuiltinRoute::detect(&RouteRequest::new(path.clone())),
                Some(BuiltinRoute::LedgerFilter),
                "expected LedgerFilter for {path}"
            );
        }
    }

    #[test]
    fn detects_compose() {
        assert_eq!(
            BuiltinRoute::detect(&RouteRequest::new("/new")),
            Some(BuiltinRoute::NewCompose)
        );
    }

    #[test]
    fn rejects_engine_routes() {
        // `/ledger/foo` and `/papers/x.pdf` lock in that ledger detection is
        // an exact-match on the trimmed path, not a prefix match — sub-paths
        // under reserved categories must reach the engine.
        for path in [
            "/blog/hello.md",
            "/websh",
            "/explorer/foo",
            "/papers/x.pdf",
            "/ledger/foo",
        ] {
            assert_eq!(
                BuiltinRoute::detect(&RouteRequest::new(path)),
                None,
                "expected engine route for {path}"
            );
        }
    }
}
