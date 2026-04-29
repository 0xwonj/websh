use crate::models::{FileType, VirtualPath};
use crate::utils::media_type_for_path;

use super::routing::{ResolvedKind, RouteResolution};

/// Renderer-neutral output produced by the engine and consumed by the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderIntent {
    DirectoryListing { node_path: VirtualPath },
    TerminalApp { node_path: VirtualPath },
    HtmlContent { node_path: VirtualPath },
    MarkdownContent { node_path: VirtualPath },
    PlainContent { node_path: VirtualPath },
    Asset {
        node_path: VirtualPath,
        media_type: String,
    },
    Redirect { node_path: VirtualPath },
}

pub fn build_render_intent(resolution: &RouteResolution) -> Option<RenderIntent> {
    let path = &resolution.node_path;

    Some(match resolution.kind {
        ResolvedKind::Directory => RenderIntent::DirectoryListing {
            node_path: path.clone(),
        },
        ResolvedKind::App => RenderIntent::TerminalApp {
            node_path: path.clone(),
        },
        ResolvedKind::Redirect => RenderIntent::Redirect {
            node_path: path.clone(),
        },
        ResolvedKind::Asset => RenderIntent::Asset {
            node_path: path.clone(),
            media_type: media_type_for_path(path.as_str()).to_string(),
        },
        ResolvedKind::Page | ResolvedKind::Document => content_intent_for_node(path),
    })
}

fn content_intent_for_node(path: &VirtualPath) -> RenderIntent {
    match FileType::from_path(path.as_str()) {
        FileType::Html => RenderIntent::HtmlContent {
            node_path: path.clone(),
        },
        FileType::Markdown => RenderIntent::MarkdownContent {
            node_path: path.clone(),
        },
        FileType::Pdf | FileType::Image => RenderIntent::Asset {
            node_path: path.clone(),
            media_type: media_type_for_path(path.as_str()).to_string(),
        },
        FileType::Link => RenderIntent::Redirect {
            node_path: path.clone(),
        },
        FileType::Unknown => RenderIntent::PlainContent {
            node_path: path.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::core::engine::{GlobalFs, RouteRequest, resolve_route};
    use crate::core::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};
    use crate::models::{DirectoryMetadata, FileMetadata, VirtualPath};

    use super::*;

    fn site(files: &[&str], directories: &[&str]) -> GlobalFs {
        let snapshot = ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    description: (*path).to_string(),
                    meta: FileMetadata::default(),
                })
                .collect(),
            directories: directories
                .iter()
                .map(|path| ScannedDirectory {
                    path: (*path).to_string(),
                    meta: DirectoryMetadata {
                        title: path.rsplit('/').next().unwrap_or(path).to_string(),
                        ..Default::default()
                    },
                })
                .collect(),
        };

        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    #[test]
    fn builds_html_content_intent_for_root_index() {
        let fs = site(&["index.html"], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::HtmlContent {
                node_path: VirtualPath::from_absolute("/index.html").unwrap(),
            }
        );
    }

    #[test]
    fn builds_markdown_content_intent_for_top_level_page() {
        let fs = site(&["about.md"], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/about")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::MarkdownContent {
                node_path: VirtualPath::from_absolute("/about.md").unwrap(),
            }
        );
    }

    #[test]
    fn builds_terminal_app_intent() {
        let fs = site(&[], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/websh")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

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
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::DirectoryListing {
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
            }
        );
    }

    #[test]
    fn builds_redirect_intent_with_source_node_path() {
        let fs = site(&["jump.link"], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/jump")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::Redirect {
                node_path: VirtualPath::from_absolute("/jump.link").unwrap(),
            }
        );
    }

    #[test]
    fn builds_html_content_intent_for_html_document() {
        let fs = site(&["blog/hello.html"], &["blog"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/blog/hello.html")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

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
        let resolution = resolve_route(&fs, &RouteRequest::new("/blog/hello.md")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::MarkdownContent {
                node_path: VirtualPath::from_absolute("/blog/hello.md").unwrap(),
            }
        );
    }

    #[test]
    fn builds_asset_intent_for_pdf_document() {
        let fs = site(&["papers/draft.pdf"], &["papers"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/papers/draft.pdf")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::Asset {
                node_path: VirtualPath::from_absolute("/papers/draft.pdf").unwrap(),
                media_type: "application/pdf".to_string(),
            }
        );
    }

    #[test]
    fn builds_asset_intent_for_image_document() {
        let fs = site(&["photos/cover.png"], &["photos"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/photos/cover.png")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::Asset {
                node_path: VirtualPath::from_absolute("/photos/cover.png").unwrap(),
                media_type: "image/png".to_string(),
            }
        );
    }

    #[test]
    fn builds_redirect_intent_for_link_document() {
        let fs = site(&["links/x.link"], &["links"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/links/x.link")).unwrap();
        let intent = build_render_intent(&resolution).unwrap();

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
        let intent = build_render_intent(&resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::PlainContent {
                node_path: VirtualPath::from_absolute("/notes/x.txt").unwrap(),
            }
        );
    }
}
