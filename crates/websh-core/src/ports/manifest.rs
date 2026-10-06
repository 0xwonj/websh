use std::collections::BTreeSet;

use crate::domain::{
    BundleValidationError, ContentManifestEntry, EntryExtensions, Manifest, NodeKind, NodeMetadata,
    VirtualPath, is_runtime_overlay_path, validate_bundle_metadata,
};
use crate::filesystem::{RouteCatalog, RouteCatalogError};

use super::{ScannedDirectory, ScannedFile, ScannedSubtree};

pub type ManifestSnapshotResult<T> = Result<T, ManifestSnapshotError>;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestPathError {
    #[error("path must not be empty")]
    EmptyFilePath,
    #[error("path must be repo-relative: {path}")]
    Absolute { path: String },
    #[error("path must use forward slashes only: {path}")]
    Backslash { path: String },
    #[error("path contains an empty segment: {path}")]
    EmptySegment { path: String },
    #[error("path contains traversal segment: {path}")]
    TraversalSegment { path: String },
    #[error("path contains a control character: {path}")]
    ControlCharacter { path: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestSnapshotError {
    #[error("manifest json is invalid: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
    #[error("duplicate manifest path: {path}")]
    DuplicatePath { path: String },
    #[error("content cannot occupy the runtime namespace: {path}")]
    RuntimePath { path: String },
    #[error("invalid manifest path `{path}`: {reason}")]
    InvalidPath {
        path: String,
        reason: ManifestPathError,
    },
    #[error("path {path} has bundle metadata but kind is not `bundle`")]
    BundleMetadataOnNonBundleKind { path: String },
    #[error("bundle {path} requires a bundle metadata block")]
    MissingBundleMetadata { path: String },
    #[error("bundle {bundle_path} variant `{variant_id}` points to nested bundle `{path}`")]
    BundleVariantTargetIsBundle {
        bundle_path: String,
        variant_id: String,
        path: String,
    },
    #[error("bundle {bundle_path} variant `{variant_id}` points to missing manifest file `{path}`")]
    MissingBundleVariantTarget {
        bundle_path: String,
        variant_id: String,
        path: String,
    },
    #[error("content route `{route}` is claimed by both `{first_path}` and `{second_path}`")]
    RouteCollision {
        route: String,
        first_path: String,
        second_path: String,
    },
    #[error(transparent)]
    RouteCatalog(#[from] RouteCatalogError),
    #[error(transparent)]
    Bundle(#[from] BundleValidationError),
}

pub fn parse_manifest_snapshot(body: &str) -> ManifestSnapshotResult<ScannedSubtree> {
    let manifest: Manifest = serde_json::from_str(body)?;

    let mut files = Vec::new();
    let mut directories = Vec::new();

    validate_unique_paths(&manifest.entries)?;

    for entry in &manifest.entries {
        let is_dir = entry.metadata.kind.is_directory_like();
        validate_manifest_path(&entry.path, is_dir)?;
        validate_manifest_metadata(&entry.path, &entry.metadata)?;
    }
    for entry in manifest.entries {
        let is_dir = entry.metadata.kind.is_directory_like();
        if is_dir {
            directories.push(ScannedDirectory {
                path: entry.path,
                meta: entry.metadata,
            });
        } else {
            files.push(ScannedFile {
                path: entry.path,
                meta: entry.metadata,
                extensions: EntryExtensions {
                    mempool: entry.mempool,
                },
            });
        }
    }

    let snapshot = ScannedSubtree { files, directories };
    RouteCatalog::validate_snapshot(&snapshot)?;
    Ok(snapshot)
}

pub fn serialize_manifest_snapshot(snapshot: &ScannedSubtree) -> ManifestSnapshotResult<String> {
    let mut entries = Vec::with_capacity(snapshot.files.len() + snapshot.directories.len());

    for dir in &snapshot.directories {
        validate_manifest_path(&dir.path, true)?;
        entries.push(ContentManifestEntry {
            path: dir.path.clone(),
            metadata: dir.meta.clone(),
            mempool: None,
        });
    }
    for file in &snapshot.files {
        validate_manifest_path(&file.path, false)?;
        entries.push(ContentManifestEntry {
            path: file.path.clone(),
            metadata: file.meta.clone(),
            mempool: file.extensions.mempool.clone(),
        });
    }
    RouteCatalog::validate_snapshot(snapshot)?;

    let manifest = Manifest {
        entries,
        release: None,
    };
    serde_json::to_string_pretty(&manifest).map_err(Into::into)
}

fn validate_unique_paths(entries: &[ContentManifestEntry]) -> ManifestSnapshotResult<()> {
    let mut paths = BTreeSet::new();
    for entry in entries {
        if !paths.insert(entry.path.as_str()) {
            return Err(ManifestSnapshotError::DuplicatePath {
                path: entry.path.clone(),
            });
        }
    }
    Ok(())
}

fn validate_manifest_metadata(path: &str, metadata: &NodeMetadata) -> ManifestSnapshotResult<()> {
    if metadata.bundle.is_some() && metadata.kind != NodeKind::Bundle {
        return Err(ManifestSnapshotError::BundleMetadataOnNonBundleKind {
            path: display_manifest_path(path).to_string(),
        });
    }

    if metadata.kind == NodeKind::Bundle {
        let bundle = metadata.bundle.as_ref().ok_or_else(|| {
            ManifestSnapshotError::MissingBundleMetadata {
                path: display_manifest_path(path).to_string(),
            }
        })?;
        validate_bundle_metadata(path, bundle)?;
    }

    Ok(())
}

fn display_manifest_path(path: &str) -> &str {
    if path.is_empty() { "/" } else { path }
}

fn validate_manifest_path(path: &str, allow_empty: bool) -> ManifestSnapshotResult<()> {
    if path.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(ManifestSnapshotError::InvalidPath {
                path: path.to_string(),
                reason: ManifestPathError::EmptyFilePath,
            })
        };
    }
    if path.starts_with('/') {
        return Err(ManifestSnapshotError::InvalidPath {
            path: path.to_string(),
            reason: ManifestPathError::Absolute {
                path: path.to_string(),
            },
        });
    }
    if path.contains('\\') {
        return Err(ManifestSnapshotError::InvalidPath {
            path: path.to_string(),
            reason: ManifestPathError::Backslash {
                path: path.to_string(),
            },
        });
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(ManifestSnapshotError::InvalidPath {
                path: path.to_string(),
                reason: ManifestPathError::EmptySegment {
                    path: path.to_string(),
                },
            });
        }
        if segment == "." || segment == ".." {
            return Err(ManifestSnapshotError::InvalidPath {
                path: path.to_string(),
                reason: ManifestPathError::TraversalSegment {
                    path: path.to_string(),
                },
            });
        }
        if segment.chars().any(char::is_control) {
            return Err(ManifestSnapshotError::InvalidPath {
                path: path.to_string(),
                reason: ManifestPathError::ControlCharacter {
                    path: path.to_string(),
                },
            });
        }
    }
    if VirtualPath::from_absolute(format!("/{path}"))
        .is_ok_and(|path| is_runtime_overlay_path(&path))
    {
        return Err(ManifestSnapshotError::RuntimePath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::{AuthoredMetadata, DerivedMetadata, NodeKind, NodeMetadata};

    use super::*;

    #[test]
    fn round_trips_manifest_document() {
        let snapshot = ScannedSubtree {
            files: vec![ScannedFile {
                path: "about.md".to_string(),
                meta: NodeMetadata {
                    kind: NodeKind::Page,
                    bundle: None,
                    authored: AuthoredMetadata {
                        title: Some("About".to_string()),
                        date: Some("2026-04-26".to_string()),
                        tags: Some(vec!["intro".to_string()]),
                        ..AuthoredMetadata::default()
                    },
                    derived: DerivedMetadata {
                        size_bytes: Some(7),
                        ..DerivedMetadata::default()
                    },
                },
                extensions: EntryExtensions::default(),
            }],
            directories: vec![ScannedDirectory {
                path: String::new(),
                meta: NodeMetadata {
                    kind: NodeKind::Directory,
                    bundle: None,
                    authored: AuthoredMetadata {
                        title: Some("Home".to_string()),
                        tags: Some(vec!["root".to_string()]),
                        ..AuthoredMetadata::default()
                    },
                    derived: DerivedMetadata::default(),
                },
            }],
        };

        let encoded = serialize_manifest_snapshot(&snapshot).expect("serialize");
        let decoded = parse_manifest_snapshot(&encoded).expect("parse");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn rejects_content_in_the_runtime_namespace() {
        for (path, kind) in [
            (".websh/state", "directory"),
            (".websh/state/env/THEME", "data"),
        ] {
            let manifest = serde_json::json!({"entries":[{"path":path,"metadata":{"kind":kind,"authored":{},"derived":{}}}]});
            assert!(matches!(
                parse_manifest_snapshot(&manifest.to_string()),
                Err(ManifestSnapshotError::RuntimePath { .. })
            ));
        }
    }

    #[test]
    fn rejects_manifest_paths_with_traversal_segments() {
        let manifest = r#"{
            "entries": [
                {"path":"../secret.md","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::InvalidPath {
                reason: ManifestPathError::TraversalSegment { .. },
                ..
            }
        ));
    }

    #[test]
    fn parses_bundle_manifest_when_declared_variant_files_exist() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"en"},
                            "variants":[
                                {"id":"en","path":"en.md","label":"English"},
                                {"id":"ko","path":"ko.md","label":"Korean"}
                            ]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/en.md","metadata":{"kind":"page","authored":{},"derived":{}}},
                {"path":"writing/foo/ko.md","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let snapshot = parse_manifest_snapshot(manifest).expect("parse bundle manifest");
        assert_eq!(snapshot.directories.len(), 1);
        assert_eq!(snapshot.files.len(), 2);
        assert!(snapshot.directories[0].meta.is_bundle());
    }

    #[test]
    fn rejects_bundle_metadata_on_non_bundle_kind() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"directory",
                        "bundle":{"default_variant":{"strategy":"static","id":"en"},"variants":[]},
                        "authored":{},
                        "derived":{}
                    }
                }
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::BundleMetadataOnNonBundleKind { .. }
        ));
    }

    #[test]
    fn rejects_bundle_manifest_without_metadata_block() {
        let manifest = r#"{
            "entries": [
                {"path":"writing/foo","metadata":{"kind":"bundle","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::MissingBundleMetadata { .. }
        ));
    }

    #[test]
    fn rejects_bundle_manifest_with_missing_variant_file() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"en"},
                            "variants":[{"id":"en","path":"en.md","label":"English"}]
                        },
                        "authored":{},
                        "derived":{}
                    }
                }
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(
                RouteCatalogError::MissingBundleVariantTarget { .. }
            )
        ));
    }

    #[test]
    fn accepts_bundle_manifest_with_directory_variant_target() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"notes"},
                            "variants":[{"id":"notes","path":"notes","label":"Notes"}]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/notes","metadata":{"kind":"directory","authored":{},"derived":{}}}
            ]
        }"#;

        let snapshot = parse_manifest_snapshot(manifest).expect("parse directory variant bundle");
        assert_eq!(snapshot.directories.len(), 2);
    }

    #[test]
    fn rejects_bundle_manifest_with_nested_bundle_variant_target() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"nested"},
                            "variants":[{"id":"nested","path":"nested","label":"Nested"}]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {
                    "path":"writing/foo/nested",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"en"},
                            "variants":[{"id":"en","path":"en.md","label":"English"}]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/nested/en.md","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(
                RouteCatalogError::BundleVariantTargetIsBundle { .. }
            )
        ));
    }

    #[test]
    fn rejects_bundle_manifest_with_route_unsafe_variant_id() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"ko.md"},
                            "variants":[{"id":"ko.md","path":"ko.md","label":"Korean"}]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/ko.md","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::Bundle(BundleValidationError::InvalidVariantId { .. })
        ));
    }

    #[test]
    fn rejects_bundle_manifest_with_root_route_collision() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"en"},
                            "variants":[{"id":"en","path":"en.md","label":"English"}]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/en.md","metadata":{"kind":"page","authored":{},"derived":{}}},
                {"path":"writing/foo.md","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(RouteCatalogError::RouteCollision { route, .. })
                if route == "/writing/foo"
        ));
    }

    #[test]
    fn rejects_normal_content_route_collisions() {
        let manifest = r#"{
            "entries": [
                {"path":"writing/foo/ko.md","metadata":{"kind":"page","authored":{},"derived":{}}},
                {"path":"writing/foo/ko.html","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(RouteCatalogError::RouteCollision { route, .. })
                if route == "/writing/foo/ko"
        ));
    }

    #[test]
    fn serialize_rejects_invalid_outgoing_snapshot() {
        let snapshot = ScannedSubtree {
            files: vec![
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
                ScannedFile {
                    path: "writing/foo/ko.html".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Page,
                        bundle: None,
                        authored: AuthoredMetadata::default(),
                        derived: DerivedMetadata::default(),
                    },
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: Vec::new(),
        };

        let err = serialize_manifest_snapshot(&snapshot).unwrap_err();

        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(RouteCatalogError::RouteCollision { route, .. })
                if route == "/writing/foo/ko"
        ));
    }

    #[test]
    fn rejects_duplicate_bundle_variant_public_routes() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"en"},
                            "variants":[
                                {"id":"en","path":"en.md","label":"English"},
                                {"id":"en_html","path":"en.html","label":"English HTML"}
                            ]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/en.md","metadata":{"kind":"page","authored":{},"derived":{}}},
                {"path":"writing/foo/en.html","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(RouteCatalogError::RouteCollision { route, .. })
                if route == "/writing/foo/en"
        ));
    }

    #[test]
    fn rejects_non_default_variant_target_collision() {
        let manifest = r#"{
            "entries": [
                {
                    "path":"writing/foo",
                    "metadata":{
                        "kind":"bundle",
                        "bundle":{
                            "default_variant":{"strategy":"static","id":"en"},
                            "variants":[
                                {"id":"en","path":"en.md","label":"English"},
                                {"id":"print_pdf","path":"print.pdf","label":"Print"}
                            ]
                        },
                        "authored":{},
                        "derived":{}
                    }
                },
                {"path":"writing/foo/en.md","metadata":{"kind":"page","authored":{},"derived":{}}},
                {"path":"writing/foo/print.pdf","metadata":{"kind":"document","authored":{},"derived":{}}},
                {"path":"writing/foo/print.pdf.md","metadata":{"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(
            err,
            ManifestSnapshotError::RouteCatalog(RouteCatalogError::RouteCollision { route, .. })
                if route == "/writing/foo/print.pdf"
        ));
    }

    #[test]
    fn rejects_authored_route_fields() {
        let manifest = r#"{
            "entries": [
                {"path":"about.md","metadata":{"kind":"page","authored":{"route":"/custom"},"derived":{}}}
            ]
        }"#;

        assert!(matches!(
            parse_manifest_snapshot(manifest).unwrap_err(),
            ManifestSnapshotError::Json { .. }
        ));
    }
}
