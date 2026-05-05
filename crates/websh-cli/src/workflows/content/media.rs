use std::path::Path;

use websh_core::domain::{Fields, ImageDim, PageSize};

use crate::CliResult;

use super::frontmatter::strip_yaml_frontmatter;

/// Compute file-type-specific derived fields (page_size for PDFs,
/// dimensions for images, word_count for markdown). Filesystem-level
/// fields (`size_bytes`, `modified_at`, `content_sha256`) are populated
/// by the caller.
pub(crate) fn derived_for_path(
    file_path: &Path,
    rel_path: &str,
    bytes: &[u8],
) -> CliResult<Fields> {
    let mut fields = Fields::default();
    let extension = Path::new(rel_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase());

    match extension.as_deref() {
        Some("pdf") => match read_pdf_dimensions(file_path) {
            Ok((page_size, page_count, rotation)) => {
                fields.page_size = Some(page_size);
                fields.page_count = Some(page_count);
                fields.rotation = Some(rotation);
            }
            Err(error) => {
                eprintln!("warn: pdf {}: {error}", rel_path);
            }
        },
        Some("png" | "jpg" | "jpeg" | "gif" | "webp") => match imagesize::blob_size(bytes) {
            Ok(dim) => {
                fields.image_dimensions = Some(ImageDim {
                    width: u32::try_from(dim.width).unwrap_or(u32::MAX),
                    height: u32::try_from(dim.height).unwrap_or(u32::MAX),
                });
            }
            Err(error) => {
                eprintln!("warn: image {}: {error}", rel_path);
            }
        },
        Some("md") => match std::str::from_utf8(bytes) {
            Ok(text) => {
                let body = strip_yaml_frontmatter(text);
                let count = body.split_whitespace().count();
                fields.word_count = Some(u32::try_from(count).unwrap_or(u32::MAX));
            }
            Err(error) => {
                eprintln!("warn: markdown {}: {error}", rel_path);
            }
        },
        _ => {}
    }
    Ok(fields)
}

fn read_pdf_dimensions(path: &Path) -> Result<(PageSize, u32, u32), String> {
    let doc = lopdf::Document::load(path).map_err(|e| format!("load: {e}"))?;
    let pages = doc.get_pages();
    let page_count = u32::try_from(pages.len()).unwrap_or(u32::MAX);
    let (_, page_id) = pages.iter().next().ok_or_else(|| "no pages".to_string())?;
    let page = doc
        .get_object(*page_id)
        .map_err(|e| format!("page object: {e}"))?
        .as_dict()
        .map_err(|e| format!("page dict: {e}"))?;
    let media_box = page
        .get(b"MediaBox")
        .map_err(|e| format!("MediaBox: {e}"))?
        .as_array()
        .map_err(|e| format!("MediaBox array: {e}"))?;
    if media_box.len() < 4 {
        return Err("MediaBox has < 4 entries".to_string());
    }
    let nums: Vec<f64> = media_box
        .iter()
        .map(|obj| {
            obj.as_float()
                .map(|f| f as f64)
                .or_else(|_| obj.as_i64().map(|i| i as f64))
                .unwrap_or(0.0)
        })
        .collect();
    let width = (nums[2] - nums[0]).abs();
    let height = (nums[3] - nums[1]).abs();
    let rotation = page
        .get(b"Rotate")
        .ok()
        .and_then(|obj| obj.as_i64().ok())
        .map(|r| r.rem_euclid(360))
        .unwrap_or(0) as u32;
    let (final_w, final_h) = if rotation % 180 == 90 {
        (height, width)
    } else {
        (width, height)
    };
    Ok((
        PageSize {
            width: final_w.round() as u32,
            height: final_h.round() as u32,
        },
        page_count,
        rotation,
    ))
}
