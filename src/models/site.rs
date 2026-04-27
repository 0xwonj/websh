//! Filesystem-first site metadata models.
//!
//! These types describe sidecar metadata, mount declarations, and derived
//! indexes consumed by the runtime loader. They remain separate from
//! backend-private scan serialization rows.

use serde::{Deserialize, Serialize};

/// High-level semantic role of a node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Page,
    Document,
    App,
    Asset,
    Redirect,
    Data,
}

/// Renderer families the engine may ask the UI to instantiate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererKind {
    HtmlPage,
    MarkdownPage,
    DirectoryListing,
    TerminalApp,
    DocumentReader,
    Image,
    Pdf,
    Redirect,
    RawText,
}

/// Trust level associated with a node or subtree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Trusted,
    Untrusted,
}

/// Sidecar metadata for a file node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSidecarMetadata {
    pub kind: Option<NodeKind>,
    pub renderer: Option<RendererKind>,
    pub route: Option<String>,
    pub layout: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub date: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub trust: Option<TrustLevel>,
}

/// Sidecar metadata for a directory node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorySidecarMetadata {
    pub kind: Option<NodeKind>,
    pub renderer: Option<RendererKind>,
    pub route: Option<String>,
    pub layout: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub trust: Option<TrustLevel>,
    pub sort: Option<String>,
}

/// Filesystem-declared mount definition loaded after bootstrap.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountDeclaration {
    pub backend: String,
    pub mount_at: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub root: Option<String>,
    pub gateway: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub writable: bool,
}

/// One route entry in the derived index.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteIndexEntry {
    pub route: String,
    pub node_path: String,
    pub kind: Option<NodeKind>,
    pub renderer: Option<RendererKind>,
}

/// Derived route/search index generated from the canonical tree plus sidecars.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedIndex {
    #[serde(default)]
    pub routes: Vec<RouteIndexEntry>,
}

/// Loaded node metadata after sidecars/bootstrap defaults are normalized.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadedNodeMetadata {
    pub kind: Option<NodeKind>,
    pub renderer: Option<RendererKind>,
    pub route: Option<String>,
    pub layout: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub date: Option<String>,
    pub tags: Vec<String>,
}

impl From<FileSidecarMetadata> for LoadedNodeMetadata {
    fn from(value: FileSidecarMetadata) -> Self {
        Self {
            kind: value.kind,
            renderer: value.renderer,
            route: value.route,
            layout: value.layout,
            title: value.title,
            description: value.description,
            date: value.date,
            tags: value.tags,
        }
    }
}

impl From<DirectorySidecarMetadata> for LoadedNodeMetadata {
    fn from(value: DirectorySidecarMetadata) -> Self {
        Self {
            kind: value.kind,
            renderer: value.renderer,
            route: value.route,
            layout: value.layout,
            title: value.title,
            description: value.description,
            date: None,
            tags: value.tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip_in_snake_case() {
        let renderer = serde_json::to_string(&RendererKind::HtmlPage).unwrap();
        let kind = serde_json::to_string(&NodeKind::Page).unwrap();
        assert_eq!(renderer, "\"html_page\"");
        assert_eq!(kind, "\"page\"");
    }

    #[test]
    fn file_sidecar_defaults_tags_when_missing() {
        let meta: FileSidecarMetadata = serde_json::from_str("{}").unwrap();
        assert_eq!(meta.tags, Vec::<String>::new());
        assert_eq!(meta.kind, None);
    }

    #[test]
    fn mount_declaration_parses_expected_shape() {
        let decl: MountDeclaration = serde_json::from_str(
            r#"{
                "backend": "github",
                "mount_at": "/db",
                "repo": "0xwonj/db",
                "branch": "main",
                "writable": true
            }"#,
        )
        .unwrap();

        assert_eq!(decl.backend, "github");
        assert_eq!(decl.mount_at, "/db");
        assert_eq!(decl.repo.as_deref(), Some("0xwonj/db"));
        assert_eq!(decl.branch.as_deref(), Some("main"));
        assert!(decl.writable);
    }

    #[test]
    fn derived_index_defaults_empty_routes() {
        let index: DerivedIndex = serde_json::from_str("{}").unwrap();
        assert!(index.routes.is_empty());
    }
}
