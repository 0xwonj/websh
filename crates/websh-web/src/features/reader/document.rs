use std::sync::Arc;

use crate::app::AppContext;
use crate::platform::redirect::{UrlValidation, validate_redirect_url};
use crate::platform::{BrowserAssetUrl, object_url_for_bytes};
use crate::render::{RenderedMarkdown, render_markdown, rendered_from_html, sanitize_html};
use websh_core::domain::VirtualPath;
use websh_core::support::media_type_for_path;

use super::{ReaderIntent, ReaderLoadError};

#[derive(Clone)]
pub(super) enum RendererContent {
    Markdown(RenderedMarkdown),
    Html(RenderedMarkdown),
    Text(String),
    Pdf { url: BrowserAssetUrl },
    Image { url: BrowserAssetUrl },
    Redirecting,
}

#[derive(Clone)]
pub(super) struct ReaderDocument {
    pub(super) content: RendererContent,
    pub(super) assets: Vec<BrowserAssetUrl>,
}

pub(super) async fn load_reader_document(
    ctx: AppContext,
    intent: ReaderIntent,
) -> Result<ReaderDocument, ReaderLoadError> {
    let path = content_path_for_intent(&intent);
    let content = match intent {
        ReaderIntent::Markdown { .. } => {
            let markdown = ctx
                .read_text(&path)
                .await
                .map_err(|source| ReaderLoadError::Read {
                    path: path.clone(),
                    source,
                })?;
            let (rendered, assets) =
                super::resources::resolve_resources(ctx, &path, render_markdown(&markdown)).await?;
            return Ok(ReaderDocument {
                content: RendererContent::Markdown(rendered),
                assets,
            });
        }
        ReaderIntent::Html { .. } => {
            let html = ctx
                .read_text(&path)
                .await
                .map_err(|source| ReaderLoadError::Read {
                    path: path.clone(),
                    source,
                })?;
            let rendered = rendered_from_html(sanitize_html(&html));
            let (rendered, assets) =
                super::resources::resolve_resources(ctx, &path, rendered).await?;
            return Ok(ReaderDocument {
                content: RendererContent::Html(rendered),
                assets,
            });
        }
        ReaderIntent::Plain { .. } => ctx
            .read_text(&path)
            .await
            .map(RendererContent::Text)
            .map_err(|source| ReaderLoadError::Read {
                path: path.clone(),
                source,
            })?,
        ReaderIntent::Pdf { .. } => load_pdf(ctx, &path).await?,
        ReaderIntent::Image { .. } => load_image(ctx, &path).await?,
        ReaderIntent::Redirect { .. } => load_redirect(ctx, &path).await?,
    };

    Ok(ReaderDocument {
        content,
        assets: Vec::new(),
    })
}

fn content_path_for_intent(intent: &ReaderIntent) -> VirtualPath {
    match intent {
        ReaderIntent::Markdown { node_path }
        | ReaderIntent::Html { node_path }
        | ReaderIntent::Plain { node_path }
        | ReaderIntent::Pdf { node_path }
        | ReaderIntent::Image { node_path }
        | ReaderIntent::Redirect { node_path } => node_path.clone(),
    }
}

async fn load_pdf(ctx: AppContext, path: &VirtualPath) -> Result<RendererContent, ReaderLoadError> {
    Ok(RendererContent::Pdf {
        url: load_asset(ctx, path).await?,
    })
}

async fn load_image(
    ctx: AppContext,
    path: &VirtualPath,
) -> Result<RendererContent, ReaderLoadError> {
    Ok(RendererContent::Image {
        url: load_asset(ctx, path).await?,
    })
}

pub(super) async fn load_asset(
    ctx: AppContext,
    path: &VirtualPath,
) -> Result<BrowserAssetUrl, ReaderLoadError> {
    let bytes = ctx
        .read_bytes(path)
        .await
        .map_err(|source| ReaderLoadError::Read {
            path: path.clone(),
            source,
        })?;
    object_url_for_bytes(&bytes, media_type_for_path(path.as_str())).map_err(|source| {
        ReaderLoadError::Asset {
            path: path.clone(),
            source,
        }
    })
}

async fn load_redirect(
    ctx: AppContext,
    path: &VirtualPath,
) -> Result<RendererContent, ReaderLoadError> {
    let target = ctx
        .read_text(path)
        .await
        .map_err(|source| ReaderLoadError::Read {
            path: path.clone(),
            source,
        })?;
    match validate_redirect_url(target.trim()) {
        UrlValidation::Valid(safe_url) => {
            if let Some(window) = web_sys::window()
                && window.location().set_href(&safe_url).is_err()
            {
                return Err(ReaderLoadError::RedirectFailed);
            }
            Ok(RendererContent::Redirecting)
        }
        UrlValidation::Invalid(source) => Err(ReaderLoadError::RedirectBlocked {
            source: Arc::new(source),
        }),
    }
}
