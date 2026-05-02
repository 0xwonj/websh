use crate::domain::VirtualPath;

const READER_EXTENSIONS: &[&str] = &[".page.html", ".page.md", ".html", ".md", ".link", ".app"];

const INDEX_SUFFIXES: &[&str] = &[
    "/index.page.html",
    "/index.page.md",
    "/index.html",
    "/index.md",
];

pub fn content_route_for_path(path: &str) -> String {
    let normalized = path.trim_matches('/');
    if normalized.is_empty() {
        return "/".to_string();
    }

    let route = INDEX_SUFFIXES
        .iter()
        .find_map(|suffix| normalized.strip_suffix(suffix))
        .map(str::to_string)
        .unwrap_or_else(|| {
            READER_EXTENSIONS
                .iter()
                .find_map(|suffix| normalized.strip_suffix(suffix))
                .unwrap_or(normalized)
                .to_string()
        });

    if route.is_empty() {
        format!("/{normalized}")
    } else {
        format!("/{route}")
    }
}

pub fn content_href_for_path(path: &str) -> String {
    format!("/#{}", content_route_for_path(path))
}

pub fn attestation_route_for_node_path(path: &VirtualPath) -> String {
    content_route_for_path(path.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_route_handles_empty_and_root_paths() {
        assert_eq!(content_route_for_path(""), "/");
        assert_eq!(content_route_for_path("/"), "/");
        assert_eq!(content_href_for_path(""), "/#/");
        assert_eq!(content_href_for_path("/"), "/#/");
    }

    #[test]
    fn content_route_strips_nested_index_files() {
        assert_eq!(content_route_for_path("/writing/index.md"), "/writing");
        assert_eq!(content_route_for_path("writing/index.html"), "/writing");
        assert_eq!(
            content_route_for_path("/papers/zks/index.page.md"),
            "/papers/zks"
        );
        assert_eq!(
            content_route_for_path("/papers/zks/index.page.html"),
            "/papers/zks"
        );
    }

    #[test]
    fn content_route_strips_reader_extensions() {
        assert_eq!(
            content_route_for_path("/papers/tabula.page.md"),
            "/papers/tabula"
        );
        assert_eq!(
            content_route_for_path("/papers/tabula.page.html"),
            "/papers/tabula"
        );
        assert_eq!(
            content_route_for_path("/papers/tabula.html"),
            "/papers/tabula"
        );
        assert_eq!(
            content_route_for_path("/papers/tabula.md"),
            "/papers/tabula"
        );
        assert_eq!(content_route_for_path("/links/site.link"), "/links/site");
        assert_eq!(content_route_for_path("/apps/demo.app"), "/apps/demo");
    }

    #[test]
    fn content_route_preserves_non_reader_extensions() {
        assert_eq!(
            content_route_for_path("/talks/slides.pdf"),
            "/talks/slides.pdf"
        );
        assert_eq!(
            content_route_for_path("/keys/wonjae.asc"),
            "/keys/wonjae.asc"
        );
    }

    #[test]
    fn content_href_adds_hash_prefix() {
        assert_eq!(
            content_href_for_path("/papers/tabula.md"),
            "/#/papers/tabula"
        );
        assert_eq!(content_href_for_path("/writing/index.md"), "/#/writing");
    }

    #[test]
    fn attestation_route_matches_content_route() {
        assert_eq!(
            attestation_route_for_node_path(
                &VirtualPath::from_absolute("/writing/hello.md").unwrap()
            ),
            "/writing/hello"
        );
    }
}
