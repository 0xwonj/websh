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
use crate::config::LANG_ENV_KEY;
use crate::features::chrome::{HOME_HREF, SiteChrome};
use crate::features::directory::DirectoryPage;
use crate::features::home::HomePage;
use crate::features::ledger::LedgerPage;
use crate::features::ledger::routes::{LEDGER_ROUTE, is_ledger_filter_route_segment};
use crate::features::reader::{Reader, ReaderFrame};
use crate::features::terminal::Shell;
use crate::platform::dom::{current_route_request, focus_terminal_input, replace_request_path};
use crate::runtime::MountLoadStatus;
use crate::shared::components::{
    AttestationSigFooter, ErrorPageActionButton, ErrorPageActionLink, ErrorPageActions,
    ErrorPageBody, ErrorPageDetails, ErrorPageFrame, ErrorPageTone, SiteContentFrame, SiteSurface,
    nearest_attestation_route_for_content_path,
};

const NOT_FOUND_CONTENT_PATH: &str = "/.site/errors/404.md";

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
use websh_core::domain::VirtualPath;
use websh_core::filesystem::{
    GlobalFs, RenderIntent, ResolvedKind, RouteCatalogError, RouteFrame, RouteRequest,
    RouteResolution, RouteRole, RouteSurface, build_render_intent, bundle_variant_href,
    content_route_for_path, is_new_request_path, route_request_targets_runtime_overlay,
    try_resolve_route,
};

/// Main application router.
///
/// Sets up hash-based routing with the following structure:
/// - `/` and `#/` → built-in homepage
/// - `#/ledger` → merged content ledger
/// - `#/websh/*path` → shell surface at canonical cwd
/// - other `#/*` paths → content route resolution against `/`
#[component]
pub fn RouterView() -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    // Raw request from URL hash (updated on hashchange).
    let _raw_request = RwSignal::new(current_route_request());

    // Set up hashchange event listener (runs once on mount).
    #[cfg(target_arch = "wasm32")]
    {
        use crate::platform::wasm_cleanup::WasmCleanup;
        use leptos::prelude::on_cleanup;
        use wasm_bindgen::JsCast;
        let closure = Closure::wrap(Box::new(move || {
            _raw_request.set(current_route_request());
        }) as Box<dyn Fn()>);

        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("hashchange", closure.as_ref().unchecked_ref());

            let cleanup = WasmCleanup(closure);
            on_cleanup(move || {
                let _ =
                    window.remove_event_listener_with_callback("hashchange", cleanup.js_function());
            });
        }
    }

    // Resolved route frame: re-runs whenever the hash changes OR fs loads/changes.
    #[cfg(target_arch = "wasm32")]
    let route = Memo::new(move |_| {
        let request = _raw_request.get();
        let fs = if route_request_targets_runtime_overlay(&request) {
            ctx.system_global_fs.get()
        } else {
            ctx.view_global_fs.get()
        };
        let Some(resolution) = try_resolve_route(&fs, &request)? else {
            return Ok(None);
        };
        let Some(intent) = build_render_intent(&fs, &resolution) else {
            return Ok(None);
        };
        Ok(Some(RouteFrame {
            request,
            resolution,
            intent,
        }))
    });
    #[cfg(not(target_arch = "wasm32"))]
    let route = Memo::new(move |_| Ok(None::<RouteFrame>));

    install_terminal_focus_effect(_raw_request, route);
    #[cfg(target_arch = "wasm32")]
    install_bundle_locale_selector_effect(ctx, route);

    view! {
        {move || {
            let request = _raw_request.get();
            let route_state = route.get();
            if let Err(error) = route_state.clone() {
                return view! { <RouteCatalogInvalid request=request error=error /> }.into_any();
            }
            match BuiltinRoute::detect(&request) {
                Some(BuiltinRoute::Home) => view! {
                    <HomePage route=Memo::new(move |_| {
                        route.get()
                            .ok()
                            .flatten()
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
                None => match route_state.ok().flatten() {
                    Some(frame) => match frame.intent {
                        RenderIntent::TerminalApp { .. } => {
                            view! { <Shell route=static_route_memo(frame.clone()) /> }.into_any()
                        }
                        RenderIntent::DirectoryListing { .. } => {
                            view! { <DirectoryPage route=static_route_memo(frame.clone()) /> }.into_any()
                        }
                        RenderIntent::BundleLocaleSelector { .. } => {
                            view! { <RoutePending request=frame.request.clone() /> }.into_any()
                        }
                        RenderIntent::HtmlContent { .. }
                        | RenderIntent::MarkdownContent { .. }
                        | RenderIntent::PlainContent { .. }
                        | RenderIntent::PdfContent { .. }
                        | RenderIntent::ImageContent { .. }
                        | RenderIntent::Redirect { .. } => {
                            let reader_frame = ReaderFrame::try_from(frame)
                                .expect("non-surface RenderIntent variants convert to ReaderFrame");
                            view! {
                                <Reader frame=Memo::new(move |_| reader_frame.clone()) />
                            }
                            .into_any()
                        }
                    },
                    None => unresolved_route_view(ctx, request),
                }
            }
        }}
    }
}

fn unresolved_route_view(ctx: AppContext, request: RouteRequest) -> AnyView {
    match ctx.mount_status_for(&VirtualPath::root()) {
        Some(MountLoadStatus::Loaded { .. }) => view! { <NotFound request=request /> }.into_any(),
        Some(MountLoadStatus::Failed { error, .. }) => {
            view! { <RootMountFailed request=request error=error /> }.into_any()
        }
        Some(MountLoadStatus::Loading { .. }) | None => {
            view! { <RoutePending request=request /> }.into_any()
        }
    }
}

fn new_compose_frame() -> RouteFrame {
    let request = RouteRequest::new("/new");
    let request_path = request.url_path.clone();
    let node_path = VirtualPath::root();
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path,
            route_path: "/new".to_string(),
            surface: RouteSurface::Content,
            route_owner_path: node_path.clone(),
            node_path: node_path.clone(),
            route_role: RouteRole::ContentNode,
            kind: ResolvedKind::Document,
            params: BTreeMap::new(),
            bundle_variant: None,
        },
        intent: RenderIntent::MarkdownContent { node_path },
    }
}

fn ledger_filter_frame(request: RouteRequest) -> RouteFrame {
    let request_path = request.url_path.clone();
    let node_path = if request.url_path.trim_matches('/') == LEDGER_ROUTE {
        VirtualPath::root()
    } else {
        VirtualPath::from_absolute(&request.url_path).unwrap_or_else(|_| VirtualPath::root())
    };
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path,
            route_path: node_path.to_string(),
            surface: RouteSurface::Content,
            route_owner_path: node_path.clone(),
            node_path: node_path.clone(),
            route_role: RouteRole::ContentNode,
            kind: ResolvedKind::Directory,
            params: BTreeMap::new(),
            bundle_variant: None,
        },
        intent: RenderIntent::DirectoryListing { node_path },
    }
}

fn home_frame(request: RouteRequest) -> RouteFrame {
    let request_path = request.url_path.clone();
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path,
            route_path: "/".to_string(),
            surface: RouteSurface::Content,
            route_owner_path: VirtualPath::root(),
            node_path: VirtualPath::root(),
            route_role: RouteRole::ContentNode,
            kind: ResolvedKind::Directory,
            params: BTreeMap::new(),
            bundle_variant: None,
        },
        intent: RenderIntent::DirectoryListing {
            node_path: VirtualPath::root(),
        },
    }
}

fn unresolved_content_frame(request: RouteRequest) -> RouteFrame {
    let request_path = request.url_path.clone();
    RouteFrame {
        request: request.clone(),
        resolution: RouteResolution {
            request_path,
            route_path: "/".to_string(),
            surface: RouteSurface::Content,
            route_owner_path: VirtualPath::root(),
            node_path: VirtualPath::root(),
            route_role: RouteRole::ContentNode,
            kind: ResolvedKind::Directory,
            params: BTreeMap::new(),
            bundle_variant: None,
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

/// Refocuses the terminal input when the user returns to a shell surface from
/// a Reader-bound surface. Lives in its own helper so the router body doesn't
/// carry the cross-cutting concern inline.
fn install_terminal_focus_effect(
    raw_request: RwSignal<RouteRequest>,
    route: Memo<Result<Option<RouteFrame>, RouteCatalogError>>,
) {
    Effect::new(move |prev_was_reader: Option<bool>| {
        if matches!(
            BuiltinRoute::detect(&raw_request.get()),
            Some(BuiltinRoute::Home)
        ) {
            return false;
        }

        let is_reader = route.get().ok().flatten().is_some_and(|frame| {
            !matches!(
                frame.intent,
                RenderIntent::TerminalApp { .. }
                    | RenderIntent::DirectoryListing { .. }
                    | RenderIntent::BundleLocaleSelector { .. }
            )
        });
        if prev_was_reader == Some(true) && !is_reader {
            focus_terminal_input();
        }
        is_reader
    });
}

#[cfg(target_arch = "wasm32")]
fn install_bundle_locale_selector_effect(
    ctx: AppContext,
    route: Memo<Result<Option<RouteFrame>, RouteCatalogError>>,
) {
    Effect::new(move |_| {
        let Ok(Some(frame)) = route.get() else {
            return;
        };
        let RenderIntent::BundleLocaleSelector { bundle_path } = frame.intent else {
            return;
        };
        let fs = ctx.view_global_fs.get();
        let runtime_state = ctx.runtime_state.get();
        let lang = runtime_state.env.get(LANG_ENV_KEY).map(String::as_str);
        let Some(href) = locale_selected_bundle_variant_href(&fs, &bundle_path, lang) else {
            return;
        };
        let target_path = href.strip_prefix('#').unwrap_or(&href);
        let target_request = RouteRequest::new(target_path);
        if target_request.url_path != frame.request.url_path {
            replace_request_path(&target_request.url_path);
        }
    });
}

fn locale_selected_bundle_variant_href(
    fs: &GlobalFs,
    bundle_path: &VirtualPath,
    lang: Option<&str>,
) -> Option<String> {
    let bundle = fs.node_metadata(bundle_path)?.bundle.as_ref()?;
    let variant = bundle.selected_variant_for_locale(lang)?;
    Some(bundle_variant_href(bundle_path, bundle, variant))
}

#[component]
fn RouteErrorPage(request: RouteRequest, children: Children) -> impl IntoView {
    let route = static_route_memo(unresolved_content_frame(request));

    view! {
        <SiteSurface class="">
            <SiteChrome route=route />
            <SiteContentFrame class="">
                <ErrorPageFrame>
                    {children()}
                </ErrorPageFrame>
            </SiteContentFrame>
        </SiteSurface>
    }
}

#[component]
fn NotFound(request: RouteRequest) -> impl IntoView {
    let request_path = request.url_path.clone();
    let resolved_path = content_route_for_path(NOT_FOUND_CONTENT_PATH);

    view! {
        <RouteErrorPage request=request>
            <ErrorPageBody
                tone=ErrorPageTone::Missing
                code="404"
                title="Page not found"
                message="The requested page was not found. You can go home, or play a small game."
                meta_resolved=resolved_path
                meta_apology="very sorry"
            >
                <ErrorPageDetails summary="Request path" open=true>
                    <code>{request_path}</code>
                </ErrorPageDetails>
                <ErrorPageActions>
                    <ErrorPageActionLink href=HOME_HREF>"Go home"</ErrorPageActionLink>
                </ErrorPageActions>
            </ErrorPageBody>
            <AttestationSigFooter
                route=Signal::derive(|| {
                    let path = VirtualPath::from_absolute(NOT_FOUND_CONTENT_PATH)
                        .expect("404 attestation path is absolute");
                    nearest_attestation_route_for_content_path(&path)
                })
                show_pending=Signal::derive(|| true)
                colophon=true
            />
        </RouteErrorPage>
    }
}

#[component]
fn RoutePending(request: RouteRequest) -> impl IntoView {
    let request_path = request.url_path.clone();

    view! {
        <RouteErrorPage request=request>
            <ErrorPageBody
                tone=ErrorPageTone::Pending
                code="pending"
                title="Route pending"
                message="The content mount is still loading. This route will resolve when the filesystem is ready."
            >
                <ErrorPageDetails summary="Request path" open=true>
                    <code>{request_path}</code>
                </ErrorPageDetails>
            </ErrorPageBody>
        </RouteErrorPage>
    }
}

#[component]
fn RootMountFailed(request: RouteRequest, error: String) -> impl IntoView {
    let request_path = request.url_path.clone();

    view! {
        <RouteErrorPage request=request>
            <ErrorPageBody
                tone=ErrorPageTone::Failure
                code="mount"
                title="Root mount failed"
                message="The content filesystem could not be mounted, so this route cannot be resolved."
            >
                <ErrorPageDetails summary="Request path" open=true>
                    <code>{request_path}</code>
                </ErrorPageDetails>
                <ErrorPageDetails summary="Mount error" open=false>
                    <code>{error}</code>
                </ErrorPageDetails>
                <ErrorPageActions>
                    <ErrorPageActionButton on_click=Callback::new(move |()| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().reload();
                        }
                    })>
                        "Reload page"
                    </ErrorPageActionButton>
                    <ErrorPageActionLink href=HOME_HREF>"Go home"</ErrorPageActionLink>
                </ErrorPageActions>
            </ErrorPageBody>
        </RouteErrorPage>
    }
}

#[component]
fn RouteCatalogInvalid(request: RouteRequest, error: RouteCatalogError) -> impl IntoView {
    let request_path = request.url_path.clone();
    let error = error.to_string();

    view! {
        <RouteErrorPage request=request>
            <ErrorPageBody
                tone=ErrorPageTone::Failure
                code="catalog"
                title="Route catalog invalid"
                message="The content filesystem has conflicting public routes, so this request cannot be resolved safely."
            >
                <ErrorPageDetails summary="Request path" open=true>
                    <code>{request_path}</code>
                </ErrorPageDetails>
                <ErrorPageDetails summary="Catalog error" open=true>
                    <code>{error}</code>
                </ErrorPageDetails>
                <ErrorPageActions>
                    <ErrorPageActionButton on_click=Callback::new(move |()| {
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().reload();
                        }
                    })>
                        "Reload page"
                    </ErrorPageActionButton>
                    <ErrorPageActionLink href=HOME_HREF>"Go home"</ErrorPageActionLink>
                </ErrorPageActions>
            </ErrorPageBody>
        </RouteErrorPage>
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod builtin_route_tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn detects_builtin_routes() {
        let cases = [
            ("/", Some(BuiltinRoute::Home)),
            ("/ledger", Some(BuiltinRoute::LedgerFilter)),
            ("/writing", Some(BuiltinRoute::LedgerFilter)),
            ("/projects", Some(BuiltinRoute::LedgerFilter)),
            ("/papers", Some(BuiltinRoute::LedgerFilter)),
            ("/talks", Some(BuiltinRoute::LedgerFilter)),
            ("/misc", Some(BuiltinRoute::LedgerFilter)),
            ("/new", Some(BuiltinRoute::NewCompose)),
        ];

        for (path, expected) in cases {
            assert_eq!(
                BuiltinRoute::detect(&RouteRequest::new(path)),
                expected,
                "unexpected builtin route for {path}"
            );
        }
    }

    #[wasm_bindgen_test]
    fn rejects_engine_routes() {
        // `/ledger/foo` and `/papers/x.pdf` lock in that ledger detection is
        // an exact-match on the trimmed path, not a prefix match — sub-paths
        // under reserved categories must reach the engine.
        for path in ["/blog/hello.md", "/websh", "/papers/x.pdf", "/ledger/foo"] {
            assert_eq!(
                BuiltinRoute::detect(&RouteRequest::new(path)),
                None,
                "expected engine route for {path}"
            );
        }
    }

    #[wasm_bindgen_test]
    fn detects_explicit_runtime_state_routes() {
        for path in [
            "/.websh/state",
            "/.websh/state/session",
            "/websh/.websh/state/session",
        ] {
            assert!(
                route_request_targets_runtime_overlay(&RouteRequest::new(path)),
                "expected system fs for {path}"
            );
        }
    }

    #[wasm_bindgen_test]
    fn keeps_normal_routes_on_content_view() {
        for path in ["/", "/ledger", "/websh", "/writing/example"] {
            assert!(
                !route_request_targets_runtime_overlay(&RouteRequest::new(path)),
                "expected content fs for {path}"
            );
        }
    }
}
