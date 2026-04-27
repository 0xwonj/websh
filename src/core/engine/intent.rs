use crate::models::VirtualPath;

use super::global_fs::GlobalFs;
use super::routing::{ResolvedKind, RouteResolution};

/// Renderer-neutral output produced by the engine and consumed by the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderIntent {
    HtmlPage {
        node_path: VirtualPath,
        layout: Option<String>,
    },
    MarkdownPage {
        node_path: VirtualPath,
        layout: Option<String>,
    },
    DirectoryListing {
        node_path: VirtualPath,
        layout: Option<String>,
    },
    TerminalApp {
        node_path: VirtualPath,
        layout: Option<String>,
    },
    DocumentReader {
        node_path: VirtualPath,
    },
    Redirect {
        node_path: VirtualPath,
    },
    Asset {
        node_path: VirtualPath,
        media_type: String,
    },
}

pub fn build_render_intent(_fs: &GlobalFs, resolution: &RouteResolution) -> Option<RenderIntent> {
    let layout = _fs
        .node_metadata(&resolution.node_path)
        .and_then(|meta| meta.layout.clone());
    let file_name = resolution.node_path.file_name().unwrap_or_default();
    let ext = file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or_default();

    Some(match resolution.kind {
        ResolvedKind::Directory => RenderIntent::DirectoryListing {
            node_path: resolution.node_path.clone(),
            layout: layout.clone(),
        },
        ResolvedKind::App => RenderIntent::TerminalApp {
            node_path: resolution.node_path.clone(),
            layout,
        },
        ResolvedKind::Redirect => RenderIntent::Redirect {
            node_path: resolution.node_path.clone(),
        },
        ResolvedKind::Asset => RenderIntent::Asset {
            node_path: resolution.node_path.clone(),
            media_type: match ext {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                _ => "application/octet-stream",
            }
            .to_string(),
        },
        ResolvedKind::Page => match ext {
            "html" => RenderIntent::HtmlPage {
                node_path: resolution.node_path.clone(),
                layout: layout.clone(),
            },
            "md" => RenderIntent::MarkdownPage {
                node_path: resolution.node_path.clone(),
                layout: layout.clone(),
            },
            _ => RenderIntent::DocumentReader {
                node_path: resolution.node_path.clone(),
            },
        },
        ResolvedKind::Document => RenderIntent::DocumentReader {
            node_path: resolution.node_path.clone(),
        },
    })
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
    fn builds_html_page_intent() {
        let fs = site(&["index.html"], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/")).unwrap();
        let intent = build_render_intent(&fs, &resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::HtmlPage {
                node_path: VirtualPath::from_absolute("/index.html").unwrap(),
                layout: None,
            }
        );
    }

    #[test]
    fn builds_markdown_page_intent() {
        let fs = site(&["about.md"], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/about")).unwrap();
        let intent = build_render_intent(&fs, &resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::MarkdownPage {
                node_path: VirtualPath::from_absolute("/about.md").unwrap(),
                layout: None,
            }
        );
    }

    #[test]
    fn builds_terminal_app_intent() {
        let fs = site(&[], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/websh")).unwrap();
        let intent = build_render_intent(&fs, &resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::TerminalApp {
                node_path: VirtualPath::root(),
                layout: None,
            }
        );
    }

    #[test]
    fn builds_directory_listing_intent() {
        let fs = site(&["blog/hello.md"], &["blog"]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/blog")).unwrap();
        let intent = build_render_intent(&fs, &resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::DirectoryListing {
                node_path: VirtualPath::from_absolute("/blog").unwrap(),
                layout: None,
            }
        );
    }

    #[test]
    fn builds_redirect_intent_with_source_node_path() {
        let fs = site(&["jump.link"], &[]);
        let resolution = resolve_route(&fs, &RouteRequest::new("/jump")).unwrap();
        let intent = build_render_intent(&fs, &resolution).unwrap();

        assert_eq!(
            intent,
            RenderIntent::Redirect {
                node_path: VirtualPath::from_absolute("/jump.link").unwrap(),
            }
        );
    }
}
