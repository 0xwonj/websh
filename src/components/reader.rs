//! Reader page — view and edit modes for content under `/`.
//!
//! For mempool paths in author mode, a small toolbar at the top of the
//! article frame surfaces an `edit` button (View) or `preview / cancel /
//! save` (Edit). The URL never changes across the toggle. `/new` mounts
//! the same component in Edit with a frontmatter placeholder.
//!
//! Toolbar lives inside the reader (document-scoped); site chrome stays
//! site-scoped. Draft state survives the Edit ↔ Preview round-trip via a
//! `draft_dirty` flag — the user's typed content is never silently
//! clobbered by re-seeding from `raw_source`.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::markdown::MarkdownView;
use crate::components::mempool::{derive_new_path, placeholder_frontmatter, save_raw};
use crate::components::shared::AttestationSigFooter;
use crate::core::engine::{
    RenderIntent, RouteFrame, RouteRequest, RouteResolution, push_request_path,
    replace_request_path,
};
use crate::models::{FileType, VirtualPath};
use crate::utils::content_routes::{attestation_route_for_node_path, content_route_for_path};
use crate::utils::current_timestamp;
use crate::utils::format::format_date_iso;
use crate::utils::{
    RenderedMarkdown, UrlValidation, data_url_for_bytes, object_url_for_bytes, render_markdown,
    rendered_from_html, sanitize_html, validate_redirect_url,
};

stylance::import_crate_style!(css, "src/components/reader.module.css");

/// Reader-bound subset of [`RenderIntent`]. Constructed by the router; carries
/// only the variants `Reader` can render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderIntent {
    Html { node_path: VirtualPath },
    Markdown { node_path: VirtualPath },
    Plain { node_path: VirtualPath },
    Asset {
        node_path: VirtualPath,
        media_type: String,
    },
    Redirect { node_path: VirtualPath },
}

/// Reader's narrowed equivalent of [`RouteFrame`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderFrame {
    pub request: RouteRequest,
    pub resolution: RouteResolution,
    pub intent: ReaderIntent,
}

impl From<ReaderIntent> for RenderIntent {
    fn from(intent: ReaderIntent) -> Self {
        match intent {
            ReaderIntent::Html { node_path } => RenderIntent::HtmlContent { node_path },
            ReaderIntent::Markdown { node_path } => RenderIntent::MarkdownContent { node_path },
            ReaderIntent::Plain { node_path } => RenderIntent::PlainContent { node_path },
            ReaderIntent::Asset {
                node_path,
                media_type,
            } => RenderIntent::Asset {
                node_path,
                media_type,
            },
            ReaderIntent::Redirect { node_path } => RenderIntent::Redirect { node_path },
        }
    }
}

impl From<ReaderFrame> for RouteFrame {
    fn from(frame: ReaderFrame) -> Self {
        RouteFrame {
            request: frame.request,
            resolution: frame.resolution,
            intent: frame.intent.into(),
        }
    }
}

impl TryFrom<RouteFrame> for ReaderFrame {
    /// On failure the original frame is returned so the caller can reroute it.
    type Error = RouteFrame;

    fn try_from(frame: RouteFrame) -> Result<Self, Self::Error> {
        let intent = match frame.intent {
            RenderIntent::HtmlContent { ref node_path } => ReaderIntent::Html {
                node_path: node_path.clone(),
            },
            RenderIntent::MarkdownContent { ref node_path } => ReaderIntent::Markdown {
                node_path: node_path.clone(),
            },
            RenderIntent::PlainContent { ref node_path } => ReaderIntent::Plain {
                node_path: node_path.clone(),
            },
            RenderIntent::Asset {
                ref node_path,
                ref media_type,
            } => ReaderIntent::Asset {
                node_path: node_path.clone(),
                media_type: media_type.clone(),
            },
            RenderIntent::Redirect { ref node_path } => ReaderIntent::Redirect {
                node_path: node_path.clone(),
            },
            RenderIntent::DirectoryListing { .. } | RenderIntent::TerminalApp { .. } => {
                return Err(frame);
            }
        };
        Ok(ReaderFrame {
            request: frame.request,
            resolution: frame.resolution,
            intent,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReaderMode {
    View,
    Edit,
}

#[derive(Clone)]
enum RendererContent {
    Html(RenderedMarkdown),
    Text(String),
    Asset { url: String, media_type: String },
    Redirecting,
}

#[component]
pub fn Reader(frame: Memo<ReaderFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let canonical_path = Memo::new(move |_| frame.get().resolution.node_path.clone());
    let filename = Memo::new(move |_| {
        canonical_path
            .get()
            .file_name()
            .map(str::to_string)
            .unwrap_or_else(|| canonical_path.get().as_str().trim_matches('/').to_string())
    });
    let attestation_route =
        Signal::derive(move || attestation_route_for_node_path(&canonical_path.get()));

    let author_mode = Memo::new({
        let ctx = ctx.clone();
        move |_| ctx.runtime_state.with(|rs| rs.github_token_present)
    });
    let is_new_route = Memo::new(move |_| frame.get().request.url_path == "/new");
    let edit_visible = Memo::new(move |_| {
        author_mode.get()
            && (canonical_path.get().as_str().starts_with("/mempool/") || is_new_route.get())
    });

    // Construction-time seed.
    //
    // /new starts in Edit with the placeholder; existing entries start in
    // View with an empty draft (filled lazily when the user clicks edit).
    // `draft_dirty` is true on /new from the outset because the placeholder
    // is the user's responsibility, not an on-disk source we should
    // overwrite on a re-toggle.
    let initial_draft = if is_new_route.get_untracked() {
        placeholder_frontmatter(&iso_today())
    } else {
        String::new()
    };
    let initial_mode = if is_new_route.get_untracked() {
        ReaderMode::Edit
    } else {
        ReaderMode::View
    };
    let initial_dirty = is_new_route.get_untracked();

    let mode = RwSignal::new(initial_mode);
    let draft_body = RwSignal::new(initial_draft);
    let draft_dirty = RwSignal::new(initial_dirty);
    let save_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    let refetch_epoch = RwSignal::new(0u32);

    // Author-mode redirect for /new — non-author lands on /ledger.
    Effect::new(move |_| {
        if is_new_route.get() && !author_mode.get() {
            replace_request_path("/ledger");
        }
    });

    // Defensive: if Leptos's into_any() boundary keeps the component identity
    // across content-path navigation, reset transient editing state. The
    // prev-guard skips the reset on the initial mount so /new's Edit seed
    // survives. `draft_body` is intentionally NOT reset — if a stale draft
    // somehow leaks across, the next toggle to Edit re-seeds from
    // raw_source because draft_dirty is now false.
    Effect::new(move |prev: Option<()>| {
        let _ = canonical_path.get();
        if prev.is_some() {
            mode.set(ReaderMode::View);
            save_error.set(None);
            draft_dirty.set(false);
        }
    });

    // Raw markdown source — used to seed `draft_body` when the user toggles
    // to Edit on an existing entry.
    let raw_source = LocalResource::new({
        let ctx_clone = ctx.clone();
        move || {
            let ctx = ctx_clone.clone();
            let path = canonical_path.get();
            let _ = refetch_epoch.get();
            async move {
                if FileType::from_path(path.as_str()) == FileType::Markdown {
                    ctx.read_text(&path).await.unwrap_or_default()
                } else {
                    String::new()
                }
            }
        }
    });

    let content = LocalResource::new({
        let ctx_clone = ctx.clone();
        move || {
            let ctx = ctx_clone.clone();
            let snapshot = frame.get();
            let path = snapshot.resolution.node_path.clone();
            let intent = snapshot.intent.clone();
            let _ = refetch_epoch.get();
            async move { load_renderer_content(ctx, path, intent).await }
        }
    });

    let on_toggle_edit = move |()| {
        // Skip the re-seed if the user already has an in-flight draft
        // (Edit → preview → edit round-trip preserves work).
        if !draft_dirty.get_untracked() {
            let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
            draft_body.set(seed);
            draft_dirty.set(true);
        }
        save_error.set(None);
        mode.set(ReaderMode::Edit);
    };

    let on_preview = move |()| {
        // Flip to View without touching draft_body or draft_dirty so the
        // round-trip back to Edit preserves the user's work.
        save_error.set(None);
        mode.set(ReaderMode::View);
    };

    let on_cancel = move |()| {
        if saving.get_untracked() {
            return;
        }
        if is_new_route.get_untracked() {
            // Cancelling /new: replace, not push, so the abandoned draft URL
            // doesn't pollute history.
            replace_request_path("/ledger");
            return;
        }
        let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
        draft_body.set(seed);
        draft_dirty.set(false);
        save_error.set(None);
        mode.set(ReaderMode::View);
    };

    let on_save = {
        let ctx = ctx.clone();
        move |()| {
            if saving.get_untracked() {
                return;
            }
            let body = draft_body.get_untracked();

            // /new: derive the target path from the typed frontmatter.
            if is_new_route.get_untracked() {
                let target = match derive_new_path(&body) {
                    Ok(target) => target,
                    Err(message) => {
                        save_error.set(Some(message));
                        return;
                    }
                };
                let rel = target
                    .as_str()
                    .trim_start_matches("/mempool/")
                    .trim_end_matches(".md");
                let message = format!("mempool: add {rel}");
                saving.set(true);
                let ctx_clone = ctx.clone();
                let target_for_nav = target.clone();
                spawn_local(async move {
                    let result = save_raw(ctx_clone, target, body, message, true).await;
                    saving.set(false);
                    match result {
                        Ok(()) => {
                            save_error.set(None);
                            push_request_path(&content_route_for_path(target_for_nav.as_str()));
                        }
                        Err(message) => save_error.set(Some(message)),
                    }
                });
                return;
            }

            // Existing entry edit.
            let path = canonical_path.get_untracked();
            if !path.as_str().starts_with("/mempool/") {
                save_error.set(Some(
                    "save is only allowed for /mempool/... paths".to_string(),
                ));
                return;
            }
            let rel = path
                .as_str()
                .trim_start_matches("/mempool/")
                .trim_end_matches(".md");
            let message = format!("mempool: edit {rel}");
            saving.set(true);
            let ctx_clone = ctx.clone();
            spawn_local(async move {
                let result = save_raw(ctx_clone, path, body, message, false).await;
                saving.set(false);
                match result {
                    Ok(()) => {
                        save_error.set(None);
                        draft_dirty.set(false);
                        mode.set(ReaderMode::View);
                        refetch_epoch.update(|n| *n += 1);
                        content.refetch();
                    }
                    Err(message) => save_error.set(Some(message)),
                }
            });
        }
    };

    let on_edit_cb = Callback::new(on_toggle_edit);
    let on_preview_cb = Callback::new(on_preview);
    let on_cancel_cb = Callback::new(on_cancel);
    let on_save_cb = Callback::new(on_save);

    let chrome_route = Memo::new(move |_| RouteFrame::from(frame.get()));

    view! {
        <div class=css::surface>
            <SiteChrome route=chrome_route />
            <main class=css::page>
                <ReaderToolbar
                    mode=mode
                    is_new=is_new_route
                    can_edit=edit_visible
                    saving=saving.read_only()
                    on_edit=on_edit_cb
                    on_preview=on_preview_cb
                    on_save=on_save_cb
                    on_cancel=on_cancel_cb
                />
                {move || save_error.get().map(|message| view! {
                    <div class=css::errorBanner role="alert">{message}</div>
                })}
                <Show
                    when=move || mode.get() == ReaderMode::Edit
                    fallback=move || view! {
                        <Suspense fallback=move || view! {
                            <div class=css::loading>"Loading..."</div>
                        }>
                            {move || {
                                content.get().map(|result| {
                                    match result {
                                        Ok(RendererContent::Html(rendered)) => {
                                            let rendered = Signal::derive(move || rendered.clone());
                                            view! {
                                                <MarkdownView rendered=rendered class=css::markdown />
                                            }.into_any()
                                        }
                                        Ok(RendererContent::Text(text)) => view! {
                                            <pre class=css::rawText>{text}</pre>
                                        }.into_any(),
                                        Ok(RendererContent::Asset { url, media_type }) => {
                                            let name = filename.get();
                                            if media_type == "application/pdf" {
                                                view! {
                                                    <iframe
                                                        src=url
                                                        class=css::pdfViewer
                                                        title=name
                                                    />
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <figure class=css::imageFrame>
                                                        <img src=url alt=name class=css::image />
                                                    </figure>
                                                }.into_any()
                                            }
                                        }
                                        Ok(RendererContent::Redirecting) => view! {
                                            <div class=css::loading>"Redirecting..."</div>
                                        }.into_any(),
                                        Err(error) => view! {
                                            <div class=css::error>{error}</div>
                                        }.into_any(),
                                    }
                                })
                            }}
                        </Suspense>
                    }
                >
                    <textarea
                        class=css::editorTextarea
                        prop:value=move || draft_body.get()
                        on:input=move |ev| {
                            draft_body.set(event_target_value(&ev));
                            draft_dirty.set(true);
                        }
                    />
                </Show>
                <Show when=move || !is_new_route.get()>
                    <AttestationSigFooter route=attestation_route />
                </Show>
            </main>
        </div>
    }
}

#[component]
fn ReaderToolbar(
    mode: RwSignal<ReaderMode>,
    is_new: Memo<bool>,
    can_edit: Memo<bool>,
    saving: ReadSignal<bool>,
    on_edit: Callback<()>,
    on_preview: Callback<()>,
    on_save: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    // Render only when there are actions or a label to show.
    let visible = Memo::new(move |_| {
        mode.get() == ReaderMode::Edit || (mode.get() == ReaderMode::View && can_edit.get())
    });

    let label = Memo::new(move |_| match (mode.get(), is_new.get()) {
        (ReaderMode::Edit, true) => "new draft",
        (ReaderMode::Edit, false) => "editing",
        (ReaderMode::View, true) => "new draft · preview",
        (ReaderMode::View, false) => "",
    });

    view! {
        <Show when=move || visible.get()>
            <header class=css::toolbar>
                <span class=css::toolbarLabel>{move || label.get()}</span>
                <div class=css::toolbarActions>
                    <Show when=move || mode.get() == ReaderMode::View>
                        <button
                            class=css::actionButton
                            on:click=move |_| on_edit.run(())
                        >"edit"</button>
                    </Show>
                    <Show when=move || mode.get() == ReaderMode::Edit>
                        <button
                            class=css::actionButton
                            on:click=move |_| on_preview.run(())
                            prop:disabled=move || saving.get()
                        >"preview"</button>
                        <button
                            class=css::actionButton
                            on:click=move |_| on_cancel.run(())
                            prop:disabled=move || saving.get()
                        >"cancel"</button>
                        <button
                            class=css::actionButtonPrimary
                            on:click=move |_| on_save.run(())
                            prop:disabled=move || saving.get()
                        >
                            {move || if saving.get() { "saving…" } else { "save" }}
                        </button>
                    </Show>
                </div>
            </header>
        </Show>
    }
}

fn iso_today() -> String {
    format_date_iso(current_timestamp() / 1000)
}

async fn load_renderer_content(
    ctx: AppContext,
    path: VirtualPath,
    intent: ReaderIntent,
) -> Result<RendererContent, String> {
    match intent {
        ReaderIntent::Markdown { .. } => ctx
            .read_text(&path)
            .await
            .map(|markdown| RendererContent::Html(render_markdown(&markdown)))
            .map_err(|error| error.to_string()),
        ReaderIntent::Html { .. } => ctx
            .read_text(&path)
            .await
            .map(|html| RendererContent::Html(rendered_from_html(sanitize_html(&html))))
            .map_err(|error| error.to_string()),
        ReaderIntent::Plain { .. } => ctx
            .read_text(&path)
            .await
            .map(RendererContent::Text)
            .map_err(|error| error.to_string()),
        ReaderIntent::Asset { media_type, .. } => load_asset(ctx, &path, media_type).await,
        ReaderIntent::Redirect { .. } => load_redirect(ctx, &path).await,
    }
}

async fn load_asset(
    ctx: AppContext,
    path: &VirtualPath,
    media_type: String,
) -> Result<RendererContent, String> {
    let bytes = ctx
        .read_bytes(path)
        .await
        .map_err(|error| error.to_string())?;
    let url = if media_type == "application/pdf" {
        object_url_for_bytes(&bytes, &media_type)?
    } else {
        data_url_for_bytes(&bytes, &media_type)
    };
    Ok(RendererContent::Asset { url, media_type })
}

async fn load_redirect(ctx: AppContext, path: &VirtualPath) -> Result<RendererContent, String> {
    let target = ctx
        .read_text(path)
        .await
        .map_err(|error| error.to_string())?;
    match validate_redirect_url(target.trim()) {
        UrlValidation::Valid(safe_url) => {
            if let Some(window) = web_sys::window()
                && window.location().set_href(&safe_url).is_err()
            {
                return Err("Failed to redirect".to_string());
            }
            Ok(RendererContent::Redirecting)
        }
        UrlValidation::Invalid(error) => Err(format!("Redirect blocked: {error}")),
    }
}

#[cfg(test)]
mod reader_intent_tests {
    use super::*;

    #[test]
    fn reader_intent_round_trip_html() {
        let intent = ReaderIntent::Html {
            node_path: VirtualPath::from_absolute("/index.html").unwrap(),
        };
        match intent {
            ReaderIntent::Html { node_path } => assert_eq!(node_path.as_str(), "/index.html"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn reader_intent_round_trip_asset() {
        let intent = ReaderIntent::Asset {
            node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
            media_type: "image/png".to_string(),
        };
        if let ReaderIntent::Asset { media_type, .. } = intent {
            assert_eq!(media_type, "image/png");
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn reader_intent_round_trip_redirect() {
        let intent = ReaderIntent::Redirect {
            node_path: VirtualPath::from_absolute("/x.link").unwrap(),
        };
        if let ReaderIntent::Redirect { node_path } = intent {
            assert_eq!(node_path.as_str(), "/x.link");
        } else {
            panic!("unexpected variant");
        }
    }

    fn make_reader_frame(intent: ReaderIntent, request_path: &str) -> ReaderFrame {
        ReaderFrame {
            request: RouteRequest::new(request_path),
            resolution: RouteResolution {
                request_path: request_path.to_string(),
                surface: crate::core::engine::RouteSurface::Content,
                node_path: VirtualPath::from_absolute(request_path).unwrap(),
                kind: crate::core::engine::ResolvedKind::Document,
                params: std::collections::BTreeMap::new(),
            },
            intent,
        }
    }

    fn round_trip(intent: ReaderIntent, request_path: &str) {
        let frame = make_reader_frame(intent.clone(), request_path);
        let route_frame = RouteFrame::from(frame.clone());
        let reconverted =
            ReaderFrame::try_from(route_frame).expect("reader-bound intent round trips");
        assert_eq!(reconverted.intent, intent);
        assert_eq!(reconverted.request, frame.request);
        assert_eq!(reconverted.resolution, frame.resolution);
    }

    #[test]
    fn reader_frame_round_trips_markdown() {
        round_trip(
            ReaderIntent::Markdown {
                node_path: VirtualPath::from_absolute("/blog/hello.md").unwrap(),
            },
            "/blog/hello.md",
        );
    }

    #[test]
    fn reader_frame_round_trips_html() {
        round_trip(
            ReaderIntent::Html {
                node_path: VirtualPath::from_absolute("/index.html").unwrap(),
            },
            "/index.html",
        );
    }

    #[test]
    fn reader_frame_round_trips_plain() {
        round_trip(
            ReaderIntent::Plain {
                node_path: VirtualPath::from_absolute("/notes/x.txt").unwrap(),
            },
            "/notes/x.txt",
        );
    }

    #[test]
    fn reader_frame_round_trips_asset() {
        round_trip(
            ReaderIntent::Asset {
                node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
                media_type: "image/png".to_string(),
            },
            "/cover.png",
        );
    }

    #[test]
    fn reader_frame_round_trips_redirect() {
        round_trip(
            ReaderIntent::Redirect {
                node_path: VirtualPath::from_absolute("/x.link").unwrap(),
            },
            "/x.link",
        );
    }

    #[test]
    fn reader_intent_to_render_intent_preserves_fields() {
        let asset = ReaderIntent::Asset {
            node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
            media_type: "image/png".to_string(),
        };
        let render: RenderIntent = asset.into();
        match render {
            RenderIntent::Asset {
                node_path,
                media_type,
            } => {
                assert_eq!(node_path.as_str(), "/cover.png");
                assert_eq!(media_type, "image/png");
            }
            other => panic!("expected Asset, got {other:?}"),
        }

        let html = ReaderIntent::Html {
            node_path: VirtualPath::from_absolute("/index.html").unwrap(),
        };
        let render: RenderIntent = html.into();
        match render {
            RenderIntent::HtmlContent { node_path } => {
                assert_eq!(node_path.as_str(), "/index.html");
            }
            other => panic!("expected HtmlContent, got {other:?}"),
        }
    }

    #[test]
    fn try_from_route_frame_rejects_directory_listing() {
        let frame = RouteFrame {
            request: RouteRequest::new("/blog"),
            resolution: RouteResolution {
                request_path: "/blog".to_string(),
                surface: crate::core::engine::RouteSurface::Content,
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
                kind: crate::core::engine::ResolvedKind::Directory,
                params: std::collections::BTreeMap::new(),
            },
            intent: RenderIntent::DirectoryListing {
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
            },
        };
        assert!(ReaderFrame::try_from(frame).is_err());
    }
}
