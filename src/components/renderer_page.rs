//! Standalone content renderer page.

use std::sync::Arc;

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::chrome::SiteChrome;
use crate::components::markdown::MarkdownView;
use crate::components::shared::AttestationSigFooter;
use crate::core::engine::{RenderIntent, RouteFrame};
use crate::models::{FileType, VirtualPath};
use crate::utils::content_routes::attestation_route_for_node_path;
use crate::utils::{
    RenderedMarkdown, UrlValidation, data_url_for_bytes, media_type_for_path, object_url_for_bytes,
    render_markdown, rendered_from_html, sanitize_html, validate_redirect_url,
};

stylance::import_crate_style!(css, "src/components/renderer_page.module.css");

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
    let edit_visible = Memo::new(move |_| {
        author_mode.get() && canonical_path.get().as_str().starts_with("/mempool/")
    });
    let edit_href = Memo::new(move |_| {
        format!("/#/edit{}", canonical_path.get().as_str())
    });

    let extra_actions: ChildrenFn = Arc::new(move || {
        view! {
            <Show when=move || edit_visible.get()>
                <a
                    href=move || edit_href.get()
                    class=css::editLink
                    aria-label="Edit this mempool entry"
                >"edit"</a>
            </Show>
        }
        .into_any()
    });

    let content = LocalResource::new(move || {
        let frame = route.get();
        let path = frame.resolution.node_path.clone();
        let intent = frame.intent.clone();
        async move { load_renderer_content(ctx, path, intent).await }
    });

    view! {
        <div class=css::surface>
            <SiteChrome route=route extra_actions=extra_actions />
            <main class=css::page>
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
                <AttestationSigFooter route=attestation_route />
            </main>
        </div>
    }
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
