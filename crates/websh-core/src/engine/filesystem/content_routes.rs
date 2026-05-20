use crate::domain::{BundleMetadata, BundleVariant, VirtualPath};

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
    format!("#{}", content_route_for_path(path))
}

pub fn bundle_variant_href(
    bundle_path: &VirtualPath,
    bundle: &BundleMetadata,
    variant: &BundleVariant,
) -> String {
    if bundle.is_static_default_variant(&variant.id) {
        return content_href_for_path(bundle_path.as_str());
    }
    content_href_for_path(bundle_path.join(&variant.path).as_str())
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
        assert_eq!(content_href_for_path(""), "#/");
        assert_eq!(content_href_for_path("/"), "#/");
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
            "#/papers/tabula"
        );
        assert_eq!(content_href_for_path("/writing/index.md"), "#/writing");
    }

    #[test]
    fn bundle_variant_href_uses_root_for_default_variant() {
        let bundle_path = VirtualPath::from_absolute("/writing/foo").unwrap();
        let bundle = BundleMetadata {
            default_variant: crate::domain::BundleDefaultVariant::Static {
                id: "en".to_string(),
            },
            variants: vec![
                BundleVariant {
                    id: "en".to_string(),
                    path: "en.md".to_string(),
                    label: "English".to_string(),
                    locale: Some("en".to_string()),
                    media_type: None,
                },
                BundleVariant {
                    id: "print_pdf".to_string(),
                    path: "print.pdf".to_string(),
                    label: "Print PDF".to_string(),
                    locale: None,
                    media_type: None,
                },
            ],
        };

        assert_eq!(
            bundle_variant_href(&bundle_path, &bundle, &bundle.variants[0]),
            "#/writing/foo"
        );
        assert_eq!(
            bundle_variant_href(&bundle_path, &bundle, &bundle.variants[1]),
            "#/writing/foo/print.pdf"
        );
    }

    #[test]
    fn locale_bundle_variant_href_uses_explicit_routes_for_all_variants() {
        let bundle_path = VirtualPath::from_absolute("/writing/foo").unwrap();
        let bundle = BundleMetadata {
            default_variant: crate::domain::BundleDefaultVariant::Locale {
                fallback: "en".to_string(),
            },
            variants: vec![
                BundleVariant {
                    id: "en".to_string(),
                    path: "en.md".to_string(),
                    label: "English".to_string(),
                    locale: Some("en".to_string()),
                    media_type: None,
                },
                BundleVariant {
                    id: "ko".to_string(),
                    path: "ko.md".to_string(),
                    label: "Korean".to_string(),
                    locale: Some("ko".to_string()),
                    media_type: None,
                },
            ],
        };

        assert_eq!(
            bundle_variant_href(&bundle_path, &bundle, &bundle.variants[0]),
            "#/writing/foo/en"
        );
        assert_eq!(
            bundle_variant_href(&bundle_path, &bundle, &bundle.variants[1]),
            "#/writing/foo/ko"
        );
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

    #[test]
    fn attestation_route_matches_content_route_for_site_support_paths() {
        assert_eq!(
            attestation_route_for_node_path(&VirtualPath::from_absolute("/.site").unwrap()),
            "/.site"
        );
        assert_eq!(
            attestation_route_for_node_path(
                &VirtualPath::from_absolute("/.site/errors/404.md").unwrap()
            ),
            "/.site/errors/404"
        );
        assert_eq!(
            attestation_route_for_node_path(
                &VirtualPath::from_absolute("/.site/keys/wonjae.asc").unwrap()
            ),
            "/.site/keys/wonjae.asc"
        );
    }
}
