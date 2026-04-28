//! Standalone content renderer page.
//!
//! Phase 7 added a view/edit toggle: when the path is under `/mempool/`
//! and author-mode is on, the chrome surfaces an `edit` button that
//! flips the page into a raw-markdown textarea. Save commits via
//! `save_raw`; the URL never changes during the toggle.

use std::sync::Arc;

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::markdown::MarkdownView;
use crate::components::mempool::{derive_new_path, placeholder_frontmatter, save_raw};
use crate::components::shared::AttestationSigFooter;
use crate::core::engine::{RenderIntent, RouteFrame, push_request_path, replace_request_path};
use crate::models::{FileType, VirtualPath};
use crate::utils::content_routes::{attestation_route_for_node_path, content_route_for_path};
use crate::utils::current_timestamp;
use crate::utils::format::format_date_iso;
use crate::utils::{
    RenderedMarkdown, UrlValidation, data_url_for_bytes, media_type_for_path, object_url_for_bytes,
    render_markdown, rendered_from_html, sanitize_html, validate_redirect_url,
};

stylance::import_crate_style!(css, "src/components/renderer_page.module.css");

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
    Unsupported(String),
}

#[component]
pub fn RendererPage(route: Memo<RouteFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let canonical_path = Memo::new(move |_| route.get().resolution.node_path.clone());
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
    let is_new_route = Memo::new(move |_| route.get().request.url_path == "/new");
    let edit_visible = Memo::new(move |_| {
        author_mode.get()
            && (canonical_path.get().as_str().starts_with("/mempool/") || is_new_route.get())
    });

    // Construction-time seed: on /new, the textarea starts in Edit mode with
    // a frontmatter placeholder; otherwise the page mounts in View with an
    // empty draft (filled lazily when the user toggles to Edit on an
    // existing entry).
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

    let mode = RwSignal::new(initial_mode);
    let draft_body = RwSignal::new(initial_draft);
    let save_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);
    let refetch_epoch = RwSignal::new(0u32);

    // Author-mode redirect for /new — non-author lands on /ledger.
    // The replace_request_path helper dispatches a synthetic hashchange so
    // the router actually re-routes (Phase 6 §7.1).
    Effect::new(move |_| {
        if is_new_route.get() && !author_mode.get() {
            replace_request_path("/ledger");
        }
    });

    // Defensive: if the page is *not* re-mounted across content-path
    // navigation (Leptos's into_any() boundary may keep component identity),
    // reset transient editing state so an in-flight draft from entry A
    // doesn't bleed into entry B. The prev-guard skips the reset on the
    // initial mount so the construction-time seed (Edit + placeholder for
    // /new) survives — same pattern as router.rs:90.
    Effect::new(move |prev: Option<()>| {
        let _ = canonical_path.get();
        if prev.is_some() {
            mode.set(ReaderMode::View);
            save_error.set(None);
        }
    });

    // Raw markdown source — used to seed `draft_body` when the user toggles
    // to Edit. Existing `content` LocalResource discards the raw source after
    // rendering, so we re-fetch it here on demand.
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
            let frame = route.get();
            let path = frame.resolution.node_path.clone();
            let intent = frame.intent.clone();
            let _ = refetch_epoch.get();
            async move { load_renderer_content(ctx, path, intent).await }
        }
    });

    let on_toggle_edit = move |_| {
        // On /new the textarea is already seeded with the placeholder;
        // on existing entries we copy the current source.
        if !is_new_route.get_untracked() {
            let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
            draft_body.set(seed);
        }
        save_error.set(None);
        mode.set(ReaderMode::Edit);
    };

    let on_cancel = {
        let saving = saving;
        move |_| {
            if saving.get_untracked() {
                return;
            }
            if is_new_route.get_untracked() {
                // Cancelling /new: replace, not push, so the abandoned draft
                // URL doesn't pollute history.
                replace_request_path("/ledger");
                return;
            }
            let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
            draft_body.set(seed);
            save_error.set(None);
            mode.set(ReaderMode::View);
        }
    };

    let on_save = {
        let ctx = ctx.clone();
        move |_| {
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
                    let result =
                        save_raw(ctx_clone, target, body, message, true).await;
                    saving.set(false);
                    match result {
                        Ok(()) => {
                            save_error.set(None);
                            push_request_path(&content_route_for_path(
                                target_for_nav.as_str(),
                            ));
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
                        mode.set(ReaderMode::View);
                        refetch_epoch.update(|n| *n += 1);
                        content.refetch();
                    }
                    Err(message) => save_error.set(Some(message)),
                }
            });
        }
    };

    let extra_actions: ChildrenFn = Arc::new(move || {
        view! {
            <Show when=move || mode.get() == ReaderMode::View && edit_visible.get()>
                <button class=css::editButton on:click=on_toggle_edit>"edit"</button>
            </Show>
            <Show when=move || mode.get() == ReaderMode::Edit>
                <button
                    class=css::cancelButton
                    on:click=on_cancel
                    prop:disabled=move || saving.get()
                >"cancel"</button>
                <button
                    class=css::saveButton
                    on:click=on_save
                    prop:disabled=move || saving.get()
                >
                    {move || if saving.get() { "Saving…" } else { "Save" }}
                </button>
            </Show>
        }
        .into_any()
    });

    view! {
        <div class=css::surface>
            <SiteChrome route=route extra_actions=extra_actions />
            <main class=css::page>
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
                                        Ok(RendererContent::Unsupported(message)) => view! {
                                            <div class=css::error>{message}</div>
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
                        on:input=move |ev| draft_body.set(event_target_value(&ev))
                    />
                </Show>
                <Show when=move || !is_new_route.get()>
                    <AttestationSigFooter route=attestation_route />
                </Show>
            </main>
        </div>
    }
}

fn iso_today() -> String {
    format_date_iso(current_timestamp() / 1000)
}

async fn load_renderer_content(
    ctx: AppContext,
    path: VirtualPath,
    intent: RenderIntent,
) -> Result<RendererContent, String> {
    match intent {
        RenderIntent::MarkdownPage { .. } => ctx
            .read_text(&path)
            .await
            .map(|markdown| RendererContent::Html(render_markdown(&markdown)))
            .map_err(|error| error.to_string()),
        RenderIntent::HtmlPage { .. } => ctx
            .read_text(&path)
            .await
            .map(|html| RendererContent::Html(rendered_from_html(sanitize_html(&html))))
            .map_err(|error| error.to_string()),
        RenderIntent::DocumentReader { .. } => match FileType::from_path(path.as_str()) {
            FileType::Pdf | FileType::Image => load_asset(ctx, &path).await,
            FileType::Html => ctx
                .read_text(&path)
                .await
                .map(|html| RendererContent::Html(rendered_from_html(sanitize_html(&html))))
                .map_err(|error| error.to_string()),
            FileType::Markdown => ctx
                .read_text(&path)
                .await
                .map(|markdown| RendererContent::Html(render_markdown(&markdown)))
                .map_err(|error| error.to_string()),
            FileType::Link => load_redirect(ctx, &path).await,
            FileType::Unknown => ctx
                .read_text(&path)
                .await
                .map(RendererContent::Text)
                .map_err(|error| error.to_string()),
        },
        RenderIntent::Asset { .. } => load_asset(ctx, &path).await,
        RenderIntent::Redirect { .. } => load_redirect(ctx, &path).await,
        RenderIntent::DirectoryListing { .. } => Ok(RendererContent::Unsupported(
            "Directory listings are handled by the explorer.".to_string(),
        )),
        RenderIntent::TerminalApp { .. } => Ok(RendererContent::Unsupported(
            "Applications are handled by websh.".to_string(),
        )),
    }
}

async fn load_asset(ctx: AppContext, path: &VirtualPath) -> Result<RendererContent, String> {
    let media_type = media_type_for_path(path.as_str()).to_string();
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
