use crate::app::AppContext;
use crate::platform::redirect::{UrlValidation, validate_redirect_url};
use crate::platform::{BrowserAssetUrl, object_url_for_bytes};
use crate::render::{RenderedMarkdown, render_markdown, rendered_from_html, sanitize_html};
use websh_core::domain::VirtualPath;
use websh_core::support::asset::data_url_for_bytes;

use super::ReaderIntent;

#[derive(Clone)]
pub(super) enum RendererContent {
    Markdown(RenderedMarkdown),
    Html(RenderedMarkdown),
    Text(String),
    Pdf { url: BrowserAssetUrl },
    Image { url: String },
    Redirecting,
}

#[derive(Clone)]
pub(super) struct ReaderDocument {
    pub(super) content: RendererContent,
    pub(super) raw_source: Option<String>,
}

pub(super) async fn load_reader_document(
    ctx: AppContext,
    path: VirtualPath,
    intent: ReaderIntent,
) -> Result<ReaderDocument, String> {
    let content = match intent {
        ReaderIntent::Markdown { .. } => {
            let markdown = ctx
                .read_text(&path)
                .await
                .map_err(|error| error.to_string())?;
            return Ok(ReaderDocument {
                content: RendererContent::Markdown(render_markdown(&markdown)),
                raw_source: Some(markdown),
            });
        }
        ReaderIntent::Html { .. } => ctx
            .read_text(&path)
            .await
            .map(|html| RendererContent::Html(rendered_from_html(sanitize_html(&html))))
            .map_err(|error| error.to_string())?,
        ReaderIntent::Plain { .. } => ctx
            .read_text(&path)
            .await
            .map(RendererContent::Text)
            .map_err(|error| error.to_string())?,
        ReaderIntent::Asset { media_type, .. } => load_asset(ctx, &path, media_type).await?,
        ReaderIntent::Redirect { .. } => load_redirect(ctx, &path).await?,
    };

    Ok(ReaderDocument {
        content,
        raw_source: None,
    })
}

async fn load_asset(
    ctx: AppContext,
    path: &VirtualPath,
    media_type: String,
) -> Result<RendererContent, String> {
    let public_url = ctx
        .public_read_url(path)
        .map_err(|error| error.to_string())?;

    if media_type == "application/pdf" {
        if let Some(url) = public_url
            .as_deref()
            .filter(|url| can_embed_pdf_url(url))
            .map(|url| BrowserAssetUrl::public(url.to_owned()))
        {
            return Ok(RendererContent::Pdf { url });
        }
        let bytes = ctx
            .read_bytes(path)
            .await
            .map_err(|error| error.to_string())?;
        let url = object_url_for_bytes(&bytes, &media_type)?;
        Ok(RendererContent::Pdf { url })
    } else {
        if let Some(url) = public_url.filter(|url| can_render_image_url(url)) {
            return Ok(RendererContent::Image { url });
        }
        let bytes = ctx
            .read_bytes(path)
            .await
            .map_err(|error| error.to_string())?;
        let url = data_url_for_bytes(&bytes, &media_type);
        Ok(RendererContent::Image { url })
    }
}

fn can_embed_pdf_url(url: &str) -> bool {
    is_relative_public_url(url) || is_githubusercontent_url(url)
}

fn can_render_image_url(url: &str) -> bool {
    is_relative_public_url(url) || url.trim_start().starts_with("https://")
}

fn is_relative_public_url(url: &str) -> bool {
    let trimmed = url.trim_start();
    !trimmed.starts_with("//") && !has_url_scheme(trimmed)
}

fn has_url_scheme(url: &str) -> bool {
    let head = url.split(['/', '?', '#']).next().unwrap_or_default();
    head.contains(':')
}

fn is_githubusercontent_url(url: &str) -> bool {
    let Some(rest) = url.trim_start().strip_prefix("https://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or_default();
    host == "raw.githubusercontent.com" || host.ends_with(".githubusercontent.com")
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

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn pdf_direct_url_allows_relative_and_githubusercontent_sources() {
        assert!(can_embed_pdf_url("./content/docs/file.pdf"));
        assert!(can_embed_pdf_url("/content/docs/file.pdf"));
        assert!(can_embed_pdf_url(
            "https://raw.githubusercontent.com/owner/repo/main/content/file.pdf"
        ));
    }

    #[wasm_bindgen_test]
    fn pdf_direct_url_rejects_non_csp_sources() {
        assert!(!can_embed_pdf_url(
            "https://gateway.pinata.cloud/ipfs/cid/file.pdf"
        ));
        assert!(!can_embed_pdf_url(
            "//gateway.pinata.cloud/ipfs/cid/file.pdf"
        ));
        assert!(!can_embed_pdf_url("javascript:alert(1)"));
    }

    #[wasm_bindgen_test]
    fn image_direct_url_allows_https_sources() {
        assert!(can_render_image_url("./content/images/file.png"));
        assert!(can_render_image_url(
            "https://gateway.pinata.cloud/ipfs/cid/file.png"
        ));
        assert!(!can_render_image_url("http://example.com/file.png"));
    }
}
