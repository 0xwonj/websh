//! Read-only content reader.

mod actions;
mod document;
mod error;
mod intent;
mod meta;
mod preferences;
mod resources;
mod shell;
mod title_block;
mod views;

pub use intent::{ReaderFrame, ReaderIntent};

use leptos::prelude::*;

use crate::app::{AppContext, RuntimeServices};
use crate::platform::dom::absolute_hash_url_for_request_path;
use websh_core::filesystem::RouteFrame;
use websh_core::support::normalize_locale_tag;

use actions::ReaderActionsBindings;
use document::{ReaderDocument, RendererContent, load_reader_document};
use error::ReaderLoadError;
use meta::{ReaderMeta, reader_meta};
use preferences::{initial_text_scale, intent_supports_text_scale, persist_text_scale};
use shell::{ReaderShell, ReaderShellState};
use views::{
    AssetReaderView, HtmlReaderView, MarkdownReaderView, PdfReaderView, PlainReaderView,
    RedirectingView,
};

// One stylance import for the whole reader module. `views/*.rs` and
// `title_block.rs` reach this via `crate::features::reader::css` rather
// than re-importing the CSS — every additional `import_crate_style!` site
// duplicates the full constant set and produces dead-code warnings for
// classes that file doesn't reference.
stylance::import_crate_style!(
    pub(crate) css,
    "src/features/reader/reader.module.css"
);

#[component]
pub fn Reader(frame: Memo<ReaderFrame>) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");
    let canonical_path = Memo::new(move |_| frame.get().resolution.node_path.clone());
    let attestation_route = Signal::derive(move || canonical_path.get().to_string());

    let intent_memo = Memo::new(move |_| frame.get().intent.clone());
    let reader_meta_memo = Memo::new(move |_| reader_meta(ctx, &frame.get()));

    let text_scale = RwSignal::new(initial_text_scale());

    let content_version = Memo::new(move |_| ctx.content.read_version(&canonical_path.get()));
    let document = LocalResource::new({
        move || {
            let version = content_version.get();
            let snapshot = frame.get();
            let intent = snapshot.intent.clone();
            let path = snapshot.resolution.node_path;
            async move {
                let result = load_reader_document(ctx, intent).await;
                let result = if ctx.content.current_read_version(&path) != version {
                    // Dropping a stale document also releases its owned object URLs.
                    Err(ReaderLoadError::Read {
                        path: path.clone(),
                        source: websh_core::filesystem::ContentReadError::Obsolete {
                            path: path.clone(),
                        },
                    })
                } else {
                    result
                };
                (path, version, result)
            }
        }
    });

    let chrome_route = Memo::new(move |_| RouteFrame::from(frame.get()));
    let set_preferred_locale = Callback::new(move |locale: String| {
        let Some(locale) = normalize_locale_tag(&locale) else {
            return;
        };
        if let Err(error) =
            RuntimeServices::new(ctx).set_env_var(crate::config::LANG_ENV_KEY, &locale)
        {
            leptos::logging::warn!("reader: failed to persist LANG preference: {error}");
        }
    });

    let shell_state = ReaderShellState {
        intent: intent_memo,
        meta: reader_meta_memo,
        chrome_route,
        attestation_route,
        set_preferred_locale,
        ready: Signal::derive(move || {
            document
                .get()
                .filter(|(path, version, _)| {
                    *path == canonical_path.get() && *version == content_version.get()
                })
                .map_or(
                    crate::runtime::content::ReadStatus::Pending,
                    |(_, _, result)| {
                        if result.is_ok() {
                            crate::runtime::content::ReadStatus::Ready
                        } else {
                            crate::runtime::content::ReadStatus::Failed
                        }
                    },
                )
        }),
    };

    let actions_bindings = ReaderActionsBindings {
        text_scalable: Signal::derive(move || intent_supports_text_scale(&intent_memo.get())),
        text_scale: text_scale.read_only(),
        set_text_scale: Callback::new(move |scale| {
            text_scale.set(scale);
            persist_text_scale(scale);
        }),
        share_url: Signal::derive(move || {
            absolute_hash_url_for_request_path(&frame.get().request.url_path)
        }),
    };

    view! {
        <ReaderShell state=shell_state actions=actions_bindings>
            <crate::shared::components::MountStatusNotice path=Signal::derive(move || canonical_path.get()) />
            <Suspense fallback=move || view! { <div class=css::loading>"Loading..."</div> }>
                {move || document.get().filter(|(path, version, _)| *path == canonical_path.get() && *version == content_version.get()).map(|(_, _, result)| render_view_body(result, reader_meta_memo))}
            </Suspense>
        </ReaderShell>
    }
}

fn render_view_body(
    result: Result<ReaderDocument, ReaderLoadError>,
    meta: Memo<ReaderMeta>,
) -> AnyView {
    let document = match result {
        Ok(document) => document,
        Err(error) => return view! { <div class=css::error>{error.to_string()}</div> }.into_any(),
    };

    let _assets = StoredValue::new_local(document.assets);
    match document.content {
        RendererContent::Markdown(rendered) => {
            let rendered = Signal::derive(move || rendered.clone());
            view! { <MarkdownReaderView rendered=rendered /> }.into_any()
        }
        RendererContent::Html(rendered) => {
            let rendered = Signal::derive(move || rendered.clone());
            view! { <HtmlReaderView rendered=rendered /> }.into_any()
        }
        RendererContent::Text(text) => view! { <PlainReaderView text=text /> }.into_any(),
        RendererContent::Pdf { url } => {
            let title = Signal::derive(move || meta.get().title.clone());
            let m = meta.get_untracked();
            view! {
                <PdfReaderView
                    title=title
                    url=url
                    size_pretty=m.size_pretty
                    abstract_text=m.description
                    page_size=m.page_size
                    page_count=m.page_count
                />
            }
            .into_any()
        }
        RendererContent::Image { url } => {
            let m = meta.get_untracked();
            view! {
                <AssetReaderView
                    url=url
                    alt=m.title
                    dimensions=m.image_dimensions
                />
            }
            .into_any()
        }
        RendererContent::Redirecting => view! { <RedirectingView /> }.into_any(),
    }
}
