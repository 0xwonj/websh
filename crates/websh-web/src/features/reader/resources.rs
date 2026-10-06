//! Resolve local document resources through the accepted content source before insertion.

use std::collections::{BTreeMap, BTreeSet};

use futures_util::{StreamExt, TryStreamExt, stream};
use wasm_bindgen::JsCast;
use websh_core::domain::{RuntimeMount, VirtualPath, is_runtime_overlay_path};
use websh_core::filesystem::content_href_for_path;

use crate::app::AppContext;
use crate::platform::{BrowserAssetUrl, js_value_message};
use crate::render::RenderedMarkdown;

use super::{ReaderLoadError, document::load_asset};

pub(super) async fn resolve_resources(
    ctx: AppContext,
    path: &VirtualPath,
    mut rendered: RenderedMarkdown,
) -> Result<(RenderedMarkdown, Vec<BrowserAssetUrl>), ReaderLoadError> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| ReaderLoadError::Resources("document unavailable".into()))?;
    let template = document
        .create_element("template")
        .map_err(dom_error)?
        .dyn_into::<web_sys::HtmlTemplateElement>()
        .map_err(|_| ReaderLoadError::Resources("template unavailable".into()))?;
    // Template contents are inert: no resource request escapes before rewriting.
    template.set_inner_html(&rendered.html);
    let mounts = ctx.content.runtime_mounts_snapshot();
    let source_root = source_owner(&mounts, path);
    let images = template
        .content()
        .query_selector_all("img[src]")
        .map_err(dom_error)?;
    let mut local_images = Vec::new();
    let mut paths = BTreeSet::new();
    for index in 0..images.length() {
        let Some(element) = images
            .item(index)
            .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
        else {
            continue;
        };
        let source = element.get_attribute("src").unwrap_or_default();
        if let Some(target) = local_path(path, &source)? {
            if is_runtime_overlay_path(&target) || source_owner(&mounts, &target) != source_root {
                return Err(ReaderLoadError::Resources(
                    "image escapes its content source".into(),
                ));
            }
            paths.insert(target.clone());
            local_images.push((element, target));
        }
    }
    if paths.len() > 64 {
        return Err(ReaderLoadError::Resources(
            "too many embedded images".into(),
        ));
    }
    let assets: BTreeMap<_, _> = stream::iter(paths.into_iter().map(|path| async move {
        let asset = load_asset(ctx, &path).await?;
        Ok::<_, ReaderLoadError>((path, asset))
    }))
    .buffer_unordered(4)
    .try_collect()
    .await?;
    for (element, path) in local_images {
        element
            .set_attribute("src", assets[&path].as_str())
            .map_err(dom_error)?;
    }
    let anchors = template
        .content()
        .query_selector_all("a[href]")
        .map_err(dom_error)?;
    for index in 0..anchors.length() {
        let Some(element) = anchors
            .item(index)
            .and_then(|node| node.dyn_into::<web_sys::Element>().ok())
        else {
            continue;
        };
        let href = element.get_attribute("href").unwrap_or_default();
        if let Some(target) = local_path(path, &href)? {
            element
                .set_attribute("href", &content_href_for_path(target.as_str()))
                .map_err(dom_error)?;
        }
    }
    rendered.html = template.inner_html();
    Ok((rendered, assets.into_values().collect()))
}

fn source_owner(mounts: &[RuntimeMount], path: &VirtualPath) -> VirtualPath {
    mounts
        .iter()
        .filter(|mount| mount.contains(path))
        .max_by_key(|mount| mount.root.as_str().len())
        .map(|mount| mount.root.clone())
        .unwrap_or_else(VirtualPath::root)
}

fn dom_error(value: wasm_bindgen::JsValue) -> ReaderLoadError {
    ReaderLoadError::Resources(js_value_message(&value))
}

fn local_path(base: &VirtualPath, value: &str) -> Result<Option<VirtualPath>, ReaderLoadError> {
    let value = value.trim();
    if value.is_empty()
        || value.starts_with('#')
        || value.starts_with("//")
        || value
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .contains(':')
    {
        return Ok(None);
    }
    let url =
        web_sys::Url::new_with_base(value, &format!("https://content.invalid{}", base.as_str()))
            .map_err(dom_error)?;
    let decoded = js_sys::decode_uri_component(&url.pathname()).map_err(dom_error)?;
    VirtualPath::from_absolute(String::from(decoded))
        .map(Some)
        .map_err(|error| ReaderLoadError::Resources(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn local_resources_resolve_from_document_and_keep_external_links_external() {
        let mounts = vec![
            RuntimeMount::new(VirtualPath::root(), "root"),
            RuntimeMount::new(VirtualPath::from_absolute("/mempool").unwrap(), "mempool"),
        ];
        assert_ne!(
            source_owner(
                &mounts,
                &VirtualPath::from_absolute("/writing/en.md").unwrap()
            ),
            source_owner(
                &mounts,
                &VirtualPath::from_absolute("/mempool/figure.png").unwrap()
            )
        );
        let base = VirtualPath::from_absolute("/writing/topic/en.md").unwrap();
        for (input, expected) in [
            ("figure.png", "/writing/topic/figure.png"),
            ("../other.pdf", "/writing/other.pdf"),
            ("/papers/file.pdf", "/papers/file.pdf"),
            ("figure%20one.png", "/writing/topic/figure one.png"),
        ] {
            assert_eq!(
                local_path(&base, input).unwrap().unwrap().as_str(),
                expected
            );
        }
        for input in [
            "#section",
            "#/papers",
            "https://example.com/a.png",
            "mailto:test@example.com",
            "//example.com/a.png",
        ] {
            assert!(local_path(&base, input).unwrap().is_none());
        }
        assert!(local_path(&base, "bad%2F..%2Ffile.png").is_err());
    }
}
