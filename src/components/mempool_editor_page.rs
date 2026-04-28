//! Page wrapper for the `/#/new` and `/#/edit/<path>` mempool authoring routes.
//!
//! Owns:
//! - Author-mode gating (render-time `<Show>` + Effect-driven redirect, see
//!   Phase 6 design §7).
//! - Edit-mode source body fetch via `LocalResource` (with `<Suspense>` for
//!   the loading shell).
//! - Path-shape acceptance checks for edit mode (design §8).
//! - Navigation callbacks: `on_saved` pushes to the view URL; `on_cancel`
//!   pushes to the view URL (edit) or `/ledger` (new).
//!
//! Mounts `MempoolEditor` once mode + body are resolved; the editor itself
//! is route-agnostic.

use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::mempool::{ComposeMode, MempoolEditor};
use crate::core::engine::{
    GlobalFs, RenderIntent, ResolvedKind, RouteFrame, RouteRequest, RouteResolution, RouteSurface,
    push_request_path, replace_request_path, resolve_route,
};
use crate::models::VirtualPath;
use crate::utils::content_routes::content_route_for_path;

stylance::import_crate_style!(
    css,
    "src/components/mempool_editor_page.module.css"
);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MempoolEditorPageMode {
    New,
    Edit { request_path: String },
}

/// Result of validating an `/edit/<request_path>` URL against the current FS.
///
/// Pure helper for unit testing — see [`check_edit_path`]. Variants that
/// carry a canonical path surface it so the error frame's "back" link can
/// point to the resolved view URL (per design §8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EditPathCheck {
    Ok { canonical: VirtualPath },
    NotMempool,
    NotFound,
    NotEditable { canonical: VirtualPath },
    NotMarkdown { canonical: VirtualPath },
}

impl EditPathCheck {
    pub(crate) fn message(&self) -> &'static str {
        match self {
            EditPathCheck::Ok { .. } => "",
            EditPathCheck::NotMempool => {
                "this URL is not editable — only /mempool/... paths can be edited."
            }
            EditPathCheck::NotFound => "no such mempool entry.",
            EditPathCheck::NotEditable { .. } | EditPathCheck::NotMarkdown { .. } => {
                "this is not a markdown entry."
            }
        }
    }
}

/// Apply the design §8 row 2-5 acceptance checks for an edit-mode request.
/// Author-mode is gated separately (§7).
pub(crate) fn check_edit_path(fs: &GlobalFs, request_path: &str) -> EditPathCheck {
    if !request_path.starts_with("mempool/") {
        return EditPathCheck::NotMempool;
    }
    let absolute = format!("/{request_path}");
    let request = RouteRequest::new(absolute);
    let Some(resolution) = resolve_route(fs, &request) else {
        return EditPathCheck::NotFound;
    };
    match resolution.kind {
        ResolvedKind::Page | ResolvedKind::Document => {}
        _ => {
            return EditPathCheck::NotEditable {
                canonical: resolution.node_path.clone(),
            };
        }
    }
    if !resolution.node_path.as_str().ends_with(".md") {
        return EditPathCheck::NotMarkdown {
            canonical: resolution.node_path.clone(),
        };
    }
    EditPathCheck::Ok {
        canonical: resolution.node_path.clone(),
    }
}

#[component]
pub fn MempoolEditorPage(mode: MempoolEditorPageMode) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let author_mode = Memo::new({
        let ctx = ctx.clone();
        move |_| ctx.runtime_state.with(|rs| rs.github_token_present)
    });

    // Effect-based redirect for non-author requests. `replace_request_path`
    // (Phase 6 §7.1) dispatches a synthetic hashchange so the router
    // re-runs and unmounts this page.
    let mode_for_effect = mode.clone();
    Effect::new(move |_| {
        if !author_mode.get() {
            let target = match &mode_for_effect {
                MempoolEditorPageMode::New => "/ledger".to_string(),
                MempoolEditorPageMode::Edit { request_path } => {
                    let absolute = format!("/{request_path}");
                    content_route_for_path(&absolute)
                }
            };
            replace_request_path(&target);
        }
    });

    // Synthesize a RouteFrame for SiteChrome breadcrumb / nav highlighting.
    // Editor pages are not part of the canonical content tree; anchoring the
    // chrome at `/ledger` (new) or the resolved canonical path (edit) keeps
    // breadcrumb crumbs pointing at *real* destinations rather than the
    // synthetic `/edit/...` URL segments (each of which would re-mount the
    // editor or 404).
    let route_for_chrome = match &mode {
        MempoolEditorPageMode::New => synthesized_frame("/ledger", VirtualPath::root()),
        MempoolEditorPageMode::Edit { request_path } => {
            let absolute = format!("/{request_path}");
            let node_path =
                VirtualPath::from_absolute(&absolute).unwrap_or_else(|_| VirtualPath::root());
            let chrome_url = content_route_for_path(&absolute);
            synthesized_frame(&chrome_url, node_path)
        }
    };
    let route_for_chrome = Memo::new(move |_| route_for_chrome.clone());

    let mode_for_body = mode;
    let ctx_for_body = ctx;

    view! {
        <div class=css::page>
            <SiteChrome route=route_for_chrome />
            <main class=css::main>
                {move || {
                    if !author_mode.get() {
                        return view! {
                            <div class=css::redirecting>"redirecting…"</div>
                        }.into_any();
                    }
                    match mode_for_body.clone() {
                        MempoolEditorPageMode::New => render_new_mode(ctx_for_body.clone()).into_any(),
                        MempoolEditorPageMode::Edit { request_path } => {
                            render_edit_mode(ctx_for_body.clone(), request_path).into_any()
                        }
                    }
                }}
            </main>
        </div>
    }
}

fn render_new_mode(_ctx: AppContext) -> impl IntoView {
    let on_saved = Callback::new(|saved: VirtualPath| {
        push_request_path(&content_route_for_path(saved.as_str()));
    });
    let on_cancel = Callback::new(|_| {
        push_request_path("/ledger");
    });
    view! {
        <MempoolEditor
            mode=ComposeMode::New { default_category: None }
            on_saved=on_saved
            on_cancel=on_cancel
        />
    }
}

fn render_edit_mode(ctx: AppContext, request_path: String) -> impl IntoView {
    // Note: check_edit_path runs reactively because it depends on view_global_fs.
    // The body fetch (LocalResource) re-runs if request_path changes (it does
    // not here — request_path is immutable per page mount) or if the FS is
    // replaced.
    let request_path_signal = StoredValue::new(request_path);

    let check = Memo::new({
        let ctx = ctx.clone();
        move |_| {
            let path = request_path_signal.get_value();
            ctx.view_global_fs.with(|fs| check_edit_path(fs, &path))
        }
    });

    let body_resource = LocalResource::new({
        let ctx = ctx.clone();
        move || {
            let ctx = ctx.clone();
            let canonical = match check.get() {
                EditPathCheck::Ok { canonical } => Some(canonical),
                _ => None,
            };
            async move {
                match canonical {
                    Some(path) => match ctx.read_text(&path).await {
                        Ok(body) => Ok((path, body)),
                        Err(error) => Err(format!("could not load source: {error}")),
                    },
                    None => Err("invalid edit path".to_string()),
                }
            }
        }
    });

    view! {
        {move || match check.get() {
            EditPathCheck::Ok { canonical } => {
                let canonical_for_cancel = canonical.clone();
                view! {
                    <Suspense fallback=|| view! {
                        <div class=css::loading>"Loading editor…"</div>
                    }>
                        {move || body_resource.get().map(|result| {
                            match result {
                                Ok((path, body)) => {
                                    let cancel_path = canonical_for_cancel.clone();
                                    let on_saved = Callback::new(|saved: VirtualPath| {
                                        push_request_path(&content_route_for_path(saved.as_str()));
                                    });
                                    let on_cancel = Callback::new(move |_| {
                                        push_request_path(&content_route_for_path(cancel_path.as_str()));
                                    });
                                    let mode = ComposeMode::Edit { path, body };
                                    view! {
                                        <MempoolEditor
                                            mode=mode
                                            on_saved=on_saved
                                            on_cancel=on_cancel
                                        />
                                    }.into_any()
                                }
                                Err(message) => view! {
                                    <ErrorFrame
                                        message=message
                                        back_to=content_route_for_path(canonical_for_cancel.as_str())
                                    />
                                }.into_any(),
                            }
                        })}
                    </Suspense>
                }.into_any()
            }
            other => {
                let back = match &other {
                    EditPathCheck::NotMempool | EditPathCheck::NotFound => "/ledger".to_string(),
                    EditPathCheck::NotEditable { canonical }
                    | EditPathCheck::NotMarkdown { canonical } => {
                        content_route_for_path(canonical.as_str())
                    }
                    EditPathCheck::Ok { .. } => unreachable!(),
                };
                view! {
                    <ErrorFrame
                        message=other.message().to_string()
                        back_to=back
                    />
                }.into_any()
            }
        }}
    }
}

#[component]
fn ErrorFrame(message: String, back_to: String) -> impl IntoView {
    let href = format!("/#{back_to}");
    view! {
        <div class=css::error>
            <p class=css::errorMessage>{message}</p>
            <a class=css::errorBack href=href>"← back"</a>
        </div>
    }
}

fn synthesized_frame(url_path: &str, node_path: VirtualPath) -> RouteFrame {
    let request = RouteRequest::new(url_path);
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

#[cfg(test)]
mod tests {
    use crate::core::storage::{ScannedFile, ScannedSubtree};
    use crate::models::FileMetadata;

    use super::*;

    fn fs_with(files: &[&str]) -> GlobalFs {
        let snapshot = ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    description: (*path).to_string(),
                    meta: FileMetadata::default(),
                })
                .collect(),
            directories: vec![],
        };
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    #[test]
    fn check_edit_path_accepts_existing_mempool_markdown() {
        let fs = fs_with(&["mempool/writing/foo.md"]);
        let result = check_edit_path(&fs, "mempool/writing/foo");
        match result {
            EditPathCheck::Ok { canonical } => {
                assert_eq!(canonical.as_str(), "/mempool/writing/foo.md");
            }
            other => panic!("expected Ok, got {:?}", other),
        }
    }

    #[test]
    fn check_edit_path_rejects_non_mempool_prefix() {
        let fs = fs_with(&["papers/foo.md"]);
        assert_eq!(
            check_edit_path(&fs, "papers/foo"),
            EditPathCheck::NotMempool
        );
    }

    #[test]
    fn check_edit_path_rejects_missing_entry() {
        let fs = fs_with(&["mempool/writing/exists.md"]);
        assert_eq!(
            check_edit_path(&fs, "mempool/writing/missing"),
            EditPathCheck::NotFound
        );
    }

    #[test]
    fn check_edit_path_rejects_directory() {
        let fs = fs_with(&["mempool/writing/foo.md"]);
        match check_edit_path(&fs, "mempool/writing") {
            EditPathCheck::NotEditable { canonical } => {
                assert_eq!(canonical.as_str(), "/mempool/writing");
            }
            other => panic!("expected NotEditable, got {:?}", other),
        }
    }

    #[test]
    fn check_edit_path_rejects_non_markdown() {
        let fs = fs_with(&["mempool/keys/foo.asc"]);
        match check_edit_path(&fs, "mempool/keys/foo.asc") {
            EditPathCheck::NotEditable { canonical } => {
                assert_eq!(canonical.as_str(), "/mempool/keys/foo.asc");
            }
            EditPathCheck::NotMarkdown { canonical } => {
                assert_eq!(canonical.as_str(), "/mempool/keys/foo.asc");
            }
            other => panic!("expected NotEditable or NotMarkdown, got {:?}", other),
        }
    }

    #[test]
    fn mempool_root_constant_unchanged() {
        // Sanity that we still know the canonical mempool root.
        use crate::components::mempool::mempool_root;
        assert_eq!(mempool_root().as_str(), "/mempool");
    }
}
