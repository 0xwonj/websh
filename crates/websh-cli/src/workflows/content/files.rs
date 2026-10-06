use std::path::Path;
use websh_core::domain::NodeKind;
use websh_core::filesystem::content_route_for_path;

pub(crate) fn route_for_content_path(rel_path: &str) -> String {
    content_route_for_path(rel_path)
}

pub(crate) fn kind_for_content_path(rel_path: &str) -> NodeKind {
    match Path::new(rel_path).extension().and_then(|ext| ext.to_str()) {
        Some("md" | "html" | "htm") => NodeKind::Page,
        Some("link") => NodeKind::Redirect,
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => NodeKind::Asset,
        Some("pdf") => NodeKind::Document,
        Some("app") => NodeKind::App,
        Some("json") => NodeKind::Data,
        _ => NodeKind::Document,
    }
}
