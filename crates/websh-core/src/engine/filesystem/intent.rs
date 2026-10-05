use crate::domain::{FileType, VirtualPath};

use super::routing::{ResolvedKind, RouteResolution};

/// Renderer-neutral output produced by the engine and consumed by the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderIntent {
    DirectoryListing { node_path: VirtualPath },
    TerminalApp { node_path: VirtualPath },
    HtmlContent { node_path: VirtualPath },
    MarkdownContent { node_path: VirtualPath },
    PlainContent { node_path: VirtualPath },
    PdfContent { node_path: VirtualPath },
    ImageContent { node_path: VirtualPath },
    Redirect { node_path: VirtualPath },
    BundleLocaleSelector { bundle_path: VirtualPath },
}

pub fn build_render_intent(resolution: &RouteResolution) -> RenderIntent {
    let path = &resolution.node_path;

    match resolution.kind {
        ResolvedKind::Directory => RenderIntent::DirectoryListing {
            node_path: path.clone(),
        },
        ResolvedKind::App => RenderIntent::TerminalApp {
            node_path: path.clone(),
        },
        ResolvedKind::Redirect => RenderIntent::Redirect {
            node_path: path.clone(),
        },
        ResolvedKind::Asset => asset_intent_for_node(path),
        ResolvedKind::Bundle => RenderIntent::BundleLocaleSelector {
            bundle_path: path.clone(),
        },
        ResolvedKind::Page | ResolvedKind::Document => content_intent_for_node(path),
    }
}

fn content_intent_for_node(path: &VirtualPath) -> RenderIntent {
    match FileType::from_path(path.as_str()) {
        FileType::Html => RenderIntent::HtmlContent {
            node_path: path.clone(),
        },
        FileType::Markdown => RenderIntent::MarkdownContent {
            node_path: path.clone(),
        },
        FileType::Pdf => RenderIntent::PdfContent {
            node_path: path.clone(),
        },
        FileType::Image => RenderIntent::ImageContent {
            node_path: path.clone(),
        },
        FileType::Link => RenderIntent::Redirect {
            node_path: path.clone(),
        },
        FileType::Unknown => RenderIntent::PlainContent {
            node_path: path.clone(),
        },
    }
}

fn asset_intent_for_node(path: &VirtualPath) -> RenderIntent {
    match FileType::from_path(path.as_str()) {
        FileType::Pdf => RenderIntent::PdfContent {
            node_path: path.clone(),
        },
        FileType::Image => RenderIntent::ImageContent {
            node_path: path.clone(),
        },
        FileType::Html | FileType::Markdown | FileType::Link | FileType::Unknown => {
            RenderIntent::PlainContent {
                node_path: path.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::domain::{
        AuthoredMetadata, BundleDefaultVariant, BundleMetadata, BundleVariant, DerivedMetadata,
        EntryExtensions, NodeKind, NodeMetadata, VirtualPath,
    };
    use crate::engine::filesystem::{
        GlobalFs, ResolvedKind, RouteRequest, RouteResolution, RouteRole, RouteSurface,
        resolve_route,
    };
    use crate::ports::{ScannedDirectory, ScannedFile, ScannedSubtree};

    use super::*;

    fn site(files: &[&str], directories: &[&str]) -> GlobalFs {
        let make_meta = |kind: NodeKind| NodeMetadata {
            kind,
            bundle: None,
            authored: AuthoredMetadata::default(),
            derived: DerivedMetadata::default(),
        };
        let make_dir_meta = |name: &str| NodeMetadata {
            kind: NodeKind::Directory,
            bundle: None,
            authored: AuthoredMetadata {
                title: Some(name.to_string()),
                ..AuthoredMetadata::default()
            },
            derived: DerivedMetadata::default(),
        };

        let snapshot = ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                })
                .collect(),
            directories: directories
                .iter()
                .map(|path| ScannedDirectory {
                    path: (*path).to_string(),
                    meta: make_dir_meta(path.rsplit('/').next().unwrap_or(path)),
                })
                .collect(),
        };

        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    fn bundle_site() -> GlobalFs {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Page,
                        bundle: None,
                        authored: AuthoredMetadata::default(),
                        derived: DerivedMetadata::default(),
                    },
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/ko.md".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Page,
                        bundle: None,
                        authored: AuthoredMetadata::default(),
                        derived: DerivedMetadata::default(),
                    },
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![ScannedDirectory {
                path: "writing/foo".to_string(),
                meta: NodeMetadata {
                    kind: NodeKind::Bundle,
                    bundle: Some(BundleMetadata {
                        default_variant: BundleDefaultVariant::Static {
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
                                id: "ko".to_string(),
                                path: "ko.md".to_string(),
                                label: "Korean".to_string(),
                                locale: Some("ko".to_string()),
                                media_type: None,
                            },
                        ],
                    }),
                    authored: AuthoredMetadata::default(),
                    derived: DerivedMetadata {
                        ..DerivedMetadata::default()
                    },
                },
            }],
        };
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    fn locale_bundle_site() -> GlobalFs {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Page,
                        bundle: None,
                        authored: AuthoredMetadata::default(),
                        derived: DerivedMetadata::default(),
                    },
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/ko.md".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Page,
                        bundle: None,
                        authored: AuthoredMetadata::default(),
                        derived: DerivedMetadata::default(),
                    },
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![ScannedDirectory {
                path: "writing/foo".to_string(),
                meta: NodeMetadata {
                    kind: NodeKind::Bundle,
                    bundle: Some(BundleMetadata {
                        default_variant: BundleDefaultVariant::Locale {
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
                    }),
                    authored: AuthoredMetadata::default(),
                    derived: DerivedMetadata {
                        ..DerivedMetadata::default()
                    },
                },
            }],
        };
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    fn asset_resolution(path: &str) -> RouteResolution {
        let node_path = VirtualPath::from_absolute(path).unwrap();
        RouteResolution {
            request_path: "/asset".to_string(),
            route_path: "/asset".to_string(),
            surface: RouteSurface::Content,
            route_owner_path: node_path.clone(),
            node_path,
            route_role: RouteRole::ContentNode,
            kind: ResolvedKind::Asset,
            params: BTreeMap::new(),
            bundle_variant: None,
        }
    }

    #[test]
    fn builds_directory_intent_for_root_route() {
        let fs = site(&[], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::DirectoryListing {
                node_path: VirtualPath::root(),
            }
        );
    }

    #[test]
    fn builds_terminal_app_intent() {
        let fs = site(&[], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/websh")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::TerminalApp {
                node_path: VirtualPath::root(),
            }
        );
    }

    #[test]
    fn builds_directory_listing_intent() {
        let fs = site(&["blog/hello.md"], &["blog"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/blog")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::DirectoryListing {
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
            }
        );
    }

    #[test]
    fn builds_html_content_intent_for_html_document() {
        let fs = site(&["blog/hello.html"], &["blog"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/blog/hello")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::HtmlContent {
                node_path: VirtualPath::from_absolute("/blog/hello.html").unwrap(),
            }
        );
    }

    #[test]
    fn builds_markdown_content_intent_for_md_document() {
        let fs = site(&["blog/hello.md"], &["blog"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/blog/hello")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::MarkdownContent {
                node_path: VirtualPath::from_absolute("/blog/hello.md").unwrap(),
            }
        );
    }

    #[test]
    fn builds_pdf_content_intent_for_pdf_document() {
        let fs = site(&["papers/draft.pdf"], &["papers"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/papers/draft.pdf")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::PdfContent {
                node_path: VirtualPath::from_absolute("/papers/draft.pdf").unwrap(),
            }
        );
    }

    #[test]
    fn builds_image_content_intent_for_image_document() {
        let fs = site(&["photos/cover.png"], &["photos"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/photos/cover.png")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::ImageContent {
                node_path: VirtualPath::from_absolute("/photos/cover.png").unwrap(),
            }
        );
    }

    #[test]
    fn builds_redirect_intent_for_link_document() {
        let fs = site(&["links/x.link"], &["links"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/links/x")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::Redirect {
                node_path: VirtualPath::from_absolute("/links/x.link").unwrap(),
            }
        );
    }

    #[test]
    fn builds_plain_content_intent_for_unknown_document() {
        let fs = site(&["notes/x.txt"], &["notes"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/notes/x.txt")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(
            intent,
            RenderIntent::PlainContent {
                node_path: VirtualPath::from_absolute("/notes/x.txt").unwrap(),
            }
        );
    }

    #[test]
    fn asset_html_markdown_link_and_unknown_paths_render_as_plain_content() {
        for path in [
            "/assets/page.html",
            "/assets/page.md",
            "/assets/jump.link",
            "/assets/raw.txt",
        ] {
            let resolution = asset_resolution(path);
            let intent = build_render_intent(&resolution);

            assert_eq!(
                intent,
                RenderIntent::PlainContent {
                    node_path: VirtualPath::from_absolute(path).unwrap(),
                }
            );
        }
    }

    #[test]
    fn asset_image_and_pdf_paths_keep_specialized_reader_intents() {
        let pdf = asset_resolution("/assets/paper.pdf");
        assert_eq!(
            build_render_intent(&pdf),
            RenderIntent::PdfContent {
                node_path: VirtualPath::from_absolute("/assets/paper.pdf").unwrap(),
            }
        );

        let image = asset_resolution("/assets/cover.png");
        assert_eq!(
            build_render_intent(&image),
            RenderIntent::ImageContent {
                node_path: VirtualPath::from_absolute("/assets/cover.png").unwrap(),
            }
        );
    }

    #[test]
    fn builds_default_bundle_variant_intent() {
        let fs = bundle_site();
        let resolution = resolve_route(&fs, &RouteRequest::new("/writing/foo")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(resolution.node_path.as_str(), "/writing/foo/en.md");
        assert_eq!(
            resolution
                .bundle_variant
                .as_ref()
                .map(|context| context.variant_id.as_str()),
            Some("en")
        );
        assert_eq!(
            intent,
            RenderIntent::MarkdownContent {
                node_path: VirtualPath::from_absolute("/writing/foo/en.md").unwrap(),
            }
        );
    }

    #[test]
    fn builds_locale_bundle_selector_intent() {
        let fs = locale_bundle_site();
        let resolution = resolve_route(&fs, &RouteRequest::new("/writing/foo")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(resolution.node_path.as_str(), "/writing/foo");
        assert_eq!(resolution.route_role, RouteRole::BundleLocaleSelector);
        assert_eq!(
            intent,
            RenderIntent::BundleLocaleSelector {
                bundle_path: VirtualPath::from_absolute("/writing/foo").unwrap(),
            }
        );
    }

    #[test]
    fn builds_explicit_bundle_variant_intent() {
        let fs = bundle_site();
        let resolution = resolve_route(&fs, &RouteRequest::new("/writing/foo/ko")).unwrap();
        let intent = build_render_intent(&resolution);

        assert_eq!(resolution.node_path.as_str(), "/writing/foo/ko.md");
        assert_eq!(
            resolution
                .bundle_variant
                .as_ref()
                .map(|context| context.variant_id.as_str()),
            Some("ko")
        );
        assert_eq!(
            intent,
            RenderIntent::MarkdownContent {
                node_path: VirtualPath::from_absolute("/writing/foo/ko.md").unwrap(),
            }
        );
    }
}
