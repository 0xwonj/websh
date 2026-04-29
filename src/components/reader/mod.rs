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
mod shell;
mod title_block;
mod toolbar;
mod views;

pub use intent::{ReaderFrame, ReaderIntent};

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::mempool::{derive_new_path, placeholder_frontmatter, save_raw};
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
use shell::{ReaderEditBindings, ReaderShell, ReaderShellState};
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
pub(super) enum ReaderMode {
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
    // /new puts the user on the hook for the placeholder content from the
    // first paint, so both flags start true. For existing entries both
    // start false until the user clicks edit (owns) and then types (dirty).
    let initial_owned = is_new_route.get_untracked();
    let initial_dirty = is_new_route.get_untracked();

    let mode = RwSignal::new(initial_mode);
    let draft_body = RwSignal::new(initial_draft);
    // `draft_owned` guards against re-seeding the textarea from `raw_source`
    // when the user round-trips Edit ↔ preview ↔ Edit. `draft_dirty` is the
    // narrower save-state — only flips true on actual keystrokes — and feeds
    // the toolbar's "● unsaved" indicator.
    let draft_owned = RwSignal::new(initial_owned);
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
            draft_owned.set(false);
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
        // Seed the editor only on first entry into Edit; the round-trip
        // back from preview must keep the in-flight draft intact.
        if !draft_owned.get_untracked() {
            let seed = raw_source.get().map(|s| s.to_string()).unwrap_or_default();
            draft_body.set(seed);
            draft_owned.set(true);
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
        draft_owned.set(false);
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
                    draft_owned.set(false);
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

    install_reader_keybindings(KeybindingTargets {
        mode,
        edit_visible,
        saving: saving.read_only(),
        on_save: on_save_cb,
        on_preview: on_preview_cb,
        on_toggle_edit: on_edit_cb,
    });

    let chrome_route = Memo::new(move |_| RouteFrame::from(frame.get()));

    // Mempool drafts are pre-signature; surface a "pending" chip there.
    // Other content paths show a chip only when an attestation exists
    // (the default behaviour). `/new` has no canonical path yet, so the
    // footer renders the border/colophon line without a chip.
    let show_pending = Signal::derive(move || {
        canonical_path.get().as_str().starts_with("/mempool/") && !is_new_route.get()
    });

    let shell_state = ReaderShellState {
        intent: intent_memo,
        meta: reader_meta_memo,
        chrome_route,
        attestation_route,
        show_pending,
        save_error: save_error.read_only(),
    };

    let edit_bindings = ReaderEditBindings {
        mode,
        can_edit: edit_visible,
        saving: saving.read_only(),
        dirty: draft_dirty.read_only(),
        on_edit: on_edit_cb,
        on_preview: on_preview_cb,
        on_save: on_save_cb,
        on_cancel: on_cancel_cb,
    };

    view! {
        <ReaderShell state=shell_state edit=edit_bindings>
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
        </ReaderShell>
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

#[derive(Clone, Copy)]
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
struct KeybindingTargets {
    mode: RwSignal<ReaderMode>,
    edit_visible: Memo<bool>,
    saving: ReadSignal<bool>,
    on_save: Callback<()>,
    on_preview: Callback<()>,
    on_toggle_edit: Callback<()>,
}

#[cfg(target_arch = "wasm32")]
fn install_reader_keybindings(targets: KeybindingTargets) {
    use leptos::prelude::on_cleanup;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    let Some(window) = web_sys::window() else {
        return;
    };

    let closure = Closure::wrap(Box::new(move |ev: web_sys::KeyboardEvent| {
        let mode_now = targets.mode.get_untracked();
        let in_textarea = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
            .is_some();

        if (ev.meta_key() || ev.ctrl_key()) && ev.key() == "s" {
            ev.prevent_default();
            if mode_now == ReaderMode::Edit && !targets.saving.get_untracked() {
                targets.on_save.run(());
            }
            return;
        }

        // Letter shortcuts must not hijack browser modifier combos
        // (Cmd+R reload, Cmd+E find, Alt+R menus, etc.) and must not
        // interfere with typing inside the editor.
        if in_textarea || ev.meta_key() || ev.ctrl_key() || ev.alt_key() {
            return;
        }

        match ev.key().as_str() {
            "r" if mode_now == ReaderMode::Edit && !targets.saving.get_untracked() => {
                targets.on_preview.run(());
            }
            "e" if mode_now == ReaderMode::View && targets.edit_visible.get_untracked() => {
                targets.on_toggle_edit.run(());
            }
            _ => {}
        }
    }) as Box<dyn Fn(web_sys::KeyboardEvent)>);

    let _ = window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());

    let cleanup = WasmCleanup(closure);
    on_cleanup(move || {
        if let Some(window) = web_sys::window() {
            let _ = window.remove_event_listener_with_callback("keydown", cleanup.js_function());
        }
        // `cleanup` drops here, freeing the boxed handler.
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn install_reader_keybindings(_targets: KeybindingTargets) {}

/// Wraps a wasm-bindgen `Closure` so it can travel through Leptos's
/// `Send + Sync + 'static` cleanup bound. The bound exists because
/// reactive_graph is platform-generic; on wasm32 there are no threads,
/// so the closure never crosses threads in practice.
///
/// The accessor method (rather than direct field access) is required so
/// closure disjoint-capture (RFC 2229) sees the whole wrapper as the
/// captured value, inheriting the unsafe `Send + Sync` impl below.
#[cfg(target_arch = "wasm32")]
struct WasmCleanup(wasm_bindgen::closure::Closure<dyn Fn(web_sys::KeyboardEvent)>);

#[cfg(target_arch = "wasm32")]
impl WasmCleanup {
    fn js_function(&self) -> &js_sys::Function {
        use wasm_bindgen::JsCast;
        self.0.as_ref().unchecked_ref()
    }
}

// SAFETY: wasm32 has no threads. The cleanup runs on the same JS thread
// that installed it; the wrapper is never genuinely sent or shared.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for WasmCleanup {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for WasmCleanup {}
