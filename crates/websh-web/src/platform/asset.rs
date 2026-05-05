//! Browser asset helpers.

use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct BrowserAssetUrl {
    inner: Rc<BrowserAssetUrlInner>,
}

#[derive(Debug)]
enum BrowserAssetUrlInner {
    Public(String),
    Object(String),
}

impl BrowserAssetUrl {
    pub fn public(url: String) -> Self {
        Self {
            inner: Rc::new(BrowserAssetUrlInner::Public(url)),
        }
    }

    pub fn as_str(&self) -> &str {
        match self.inner.as_ref() {
            BrowserAssetUrlInner::Public(url) | BrowserAssetUrlInner::Object(url) => url,
        }
    }
}

impl Drop for BrowserAssetUrlInner {
    fn drop(&mut self) {
        if let Self::Object(url) = self
            && let Err(error) = web_sys::Url::revoke_object_url(url)
        {
            web_sys::console::warn_1(
                &format!("failed to revoke object URL {url}: {error:?}").into(),
            );
        }
    }
}

pub fn object_url_for_bytes(bytes: &[u8], media_type: &str) -> Result<BrowserAssetUrl, String> {
    let bytes = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&bytes.buffer());

    let options = web_sys::BlobPropertyBag::new();
    options.set_type(media_type);

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|error| format!("failed to create Blob: {error:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|error| format!("failed to create object URL: {error:?}"))?;
    Ok(BrowserAssetUrl {
        inner: Rc::new(BrowserAssetUrlInner::Object(url)),
    })
}
