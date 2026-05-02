use base64::{Engine, engine::general_purpose::STANDARD as B64};

pub fn data_url_for_bytes(bytes: &[u8], media_type: &str) -> String {
    format!("data:{media_type};base64,{}", B64.encode(bytes))
}

#[cfg(target_arch = "wasm32")]
pub fn object_url_for_bytes(bytes: &[u8], media_type: &str) -> Result<String, String> {
    let bytes = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&bytes.buffer());

    let options = web_sys::BlobPropertyBag::new();
    options.set_type(media_type);

    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
        .map_err(|error| format!("failed to create Blob: {error:?}"))?;
    web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|error| format!("failed to create object URL: {error:?}"))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn object_url_for_bytes(bytes: &[u8], media_type: &str) -> Result<String, String> {
    Ok(data_url_for_bytes(bytes, media_type))
}

pub fn media_type_for_path(path: &str) -> &'static str {
    match path.rsplit('.').next().map(|ext| ext.to_ascii_lowercase()) {
        Some(ext) => match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            "md" => "text/markdown; charset=utf-8",
            "txt" | "link" => "text/plain; charset=utf-8",
            "json" => "application/json",
            _ => "application/octet-stream",
        },
        None => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_type_maps_common_extensions() {
        assert_eq!(media_type_for_path("doc.pdf"), "application/pdf");
        assert_eq!(media_type_for_path("photo.JPG"), "image/jpeg");
        assert_eq!(media_type_for_path("icon.svg"), "image/svg+xml");
    }

    #[test]
    fn data_url_includes_base64_prefix() {
        let url = data_url_for_bytes(b"hi", "text/plain");
        assert!(url.starts_with("data:text/plain;base64,"));
    }
}
