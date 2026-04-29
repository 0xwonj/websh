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

mod intent;
mod meta;
mod title_block;
mod views;

pub use intent::{ReaderFrame, ReaderIntent};

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::mempool::{derive_new_path, placeholder_frontmatter, save_raw};
use crate::components::shared::AttestationSigFooter;
use crate::core::engine::{RouteFrame, push_request_path, replace_request_path};
use crate::models::VirtualPath;
use crate::utils::content_routes::{attestation_route_for_node_path, content_route_for_path};
use crate::utils::current_timestamp;
use crate::utils::format::format_date_iso;
use crate::utils::{
    RenderedMarkdown, UrlValidation, data_url_for_bytes, object_url_for_bytes, render_markdown,
    rendered_from_html, sanitize_html, validate_redirect_url,
};

use meta::{ReaderMeta, reader_meta};
use title_block::{Ident, TitleBlock};
use views::{
    AssetReaderView, HtmlReaderView, MarkdownEditorView, MarkdownReaderView, PdfReaderView,
    PlainReaderView, RedirectingView,
};

// One stylance import for the whole reader module. `views/*.rs` and
// `title_block.rs` reach this via `crate::components::reader::css` rather
// than re-importing the CSS — every additional `import_crate_style!` site
// duplicates the full constant set and produces dead-code warnings for
// classes that file doesn't reference.
stylance::import_crate_style!(
    #[allow(dead_code)]
    pub(crate) css,
    "src/components/reader/reader.module.css"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReaderMode {
    View,
    Edit,
}

#[derive(Clone)]
enum RendererContent {
    Markdown(RenderedMarkdown),
    Html(RenderedMarkdown),
    Text(String),
    Pdf { url: String },
    Image { url: String },
    Redirecting,
}

#[component]
pub fn Reader(frame: Memo<ReaderFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let canonical_path = Memo::new(move |_| frame.get().resolution.node_path.clone());
    let attestation_route =
        Signal::derive(move || attestation_route_for_node_path(&canonical_path.get()));

    let intent_memo = Memo::new(move |_| frame.get().intent.clone());
    let reader_meta_memo = Memo::new(move |_| reader_meta(ctx, &intent_memo.get()));

    let author_mode = Memo::new(move |_| ctx.runtime_state.with(|rs| rs.github_token_present));
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
        move || {
            let path = canonical_path.get();
            let is_markdown = matches!(intent_memo.get(), ReaderIntent::Markdown { .. });
            let _ = refetch_epoch.get();
            async move {
                if is_markdown {
                    ctx.read_text(&path).await.unwrap_or_default()
                } else {
                    String::new()
                }
            }
        }
    });

    let content = LocalResource::new({
        move || {
            let snapshot = frame.get();
            let path = snapshot.resolution.node_path.clone();
            let intent = snapshot.intent.clone();
            let _ = refetch_epoch.get();
            async move { load_renderer_content(ctx, path, intent).await }
        }
    });

    let on_toggle_edit = move |()| {
        if !draft_dirty.get_untracked() {
            let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
            draft_body.set(seed);
            draft_dirty.set(true);
        }
        save_error.set(None);
        mode.set(ReaderMode::Edit);
    };

    let on_preview = move |()| {
        save_error.set(None);
        mode.set(ReaderMode::View);
    };

    let on_cancel = move |()| {
        if saving.get_untracked() {
            return;
        }
        if is_new_route.get_untracked() {
            replace_request_path("/ledger");
            return;
        }
        let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
        draft_body.set(seed);
        draft_dirty.set(false);
        save_error.set(None);
        mode.set(ReaderMode::View);
    };

    let on_save = move |()| {
        if saving.get_untracked() {
            return;
        }
        let body = draft_body.get_untracked();

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
            let target_for_nav = target.clone();
            spawn_local(async move {
                let result = save_raw(ctx, target, body, message, true).await;
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
        spawn_local(async move {
            let result = save_raw(ctx, path, body, message, false).await;
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
    };

    let on_edit_cb = Callback::new(on_toggle_edit);
    let on_preview_cb = Callback::new(on_preview);
    let on_cancel_cb = Callback::new(on_cancel);
    let on_save_cb = Callback::new(on_save);
    let on_input_dirty_cb = Callback::new(move |()| draft_dirty.set(true));

    let chrome_route = Memo::new(move |_| RouteFrame::from(frame.get()));

    view! {
        <div class=css::surface>
            <SiteChrome route=chrome_route />
            <main class=css::page>
                <Show
                    when=move || !matches!(intent_memo.get(), ReaderIntent::Redirect { .. })
                >
                    <Ident meta=reader_meta_memo />
                    <TitleBlock intent=intent_memo meta=reader_meta_memo />
                </Show>
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
                                    render_view_body(result, reader_meta_memo)
                                })
                            }}
                        </Suspense>
                    }
                >
                    <MarkdownEditorView
                        draft_body=draft_body
                        on_input_dirty=on_input_dirty_cb
                    />
                </Show>
                <Show when=move || !is_new_route.get()>
                    <AttestationSigFooter route=attestation_route />
                </Show>
            </main>
        </div>
    }
}

fn render_view_body(result: Result<RendererContent, String>, meta: Memo<ReaderMeta>) -> AnyView {
    match result {
        Ok(RendererContent::Markdown(rendered)) => {
            let rendered = Signal::derive(move || rendered.clone());
            view! { <MarkdownReaderView rendered=rendered /> }.into_any()
        }
        Ok(RendererContent::Html(rendered)) => {
            let rendered = Signal::derive(move || rendered.clone());
            view! { <HtmlReaderView rendered=rendered /> }.into_any()
        }
        Ok(RendererContent::Text(text)) => view! { <PlainReaderView text=text /> }.into_any(),
        Ok(RendererContent::Pdf { url }) => {
            let title = Signal::derive(move || meta.get().title.clone());
            let m = meta.get_untracked();
            view! {
                <PdfReaderView
                    title=title
                    url=url
                    size_pretty=m.size_pretty
                    abstract_text=m.description
                />
            }
            .into_any()
        }
        Ok(RendererContent::Image { url }) => {
            let alt = meta.get_untracked().title;
            view! { <AssetReaderView url=url alt=alt /> }.into_any()
        }
        Ok(RendererContent::Redirecting) => view! { <RedirectingView /> }.into_any(),
        Err(error) => view! { <div class=css::error>{error}</div> }.into_any(),
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
            .map(|markdown| RendererContent::Markdown(render_markdown(&markdown)))
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
    if media_type == "application/pdf" {
        let url = object_url_for_bytes(&bytes, &media_type)?;
        Ok(RendererContent::Pdf { url })
    } else {
        let url = data_url_for_bytes(&bytes, &media_type);
        Ok(RendererContent::Image { url })
    }
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
