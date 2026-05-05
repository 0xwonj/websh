use crate::domain::{ContentManifestDocument, ContentManifestEntry, EntryExtensions, NodeKind};

use super::{ScannedDirectory, ScannedFile, ScannedSubtree, StorageError, StorageResult};

pub fn parse_manifest_snapshot(body: &str) -> StorageResult<ScannedSubtree> {
    let manifest: ContentManifestDocument = serde_json::from_str(body)
        .map_err(|error| StorageError::ValidationFailed(error.to_string()))?;

    let mut files = Vec::new();
    let mut directories = Vec::new();

    for entry in manifest.entries {
        let is_dir = matches!(entry.metadata.effective_kind(), NodeKind::Directory);
        validate_manifest_path(&entry.path, is_dir).map_err(StorageError::ValidationFailed)?;
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

    Ok(ScannedSubtree { files, directories })
}

pub fn serialize_manifest_snapshot(snapshot: &ScannedSubtree) -> StorageResult<String> {
    let mut entries = Vec::with_capacity(snapshot.files.len() + snapshot.directories.len());

    for dir in &snapshot.directories {
        validate_manifest_path(&dir.path, true).map_err(StorageError::BadRequest)?;
        entries.push(ContentManifestEntry {
            path: dir.path.clone(),
            metadata: dir.meta.clone(),
            mempool: None,
        });
    }
    for file in &snapshot.files {
        validate_manifest_path(&file.path, false).map_err(StorageError::BadRequest)?;
        entries.push(ContentManifestEntry {
            path: file.path.clone(),
            metadata: file.meta.clone(),
            mempool: file.extensions.mempool.clone(),
        });
    }

    let manifest = ContentManifestDocument { entries };
    serde_json::to_string_pretty(&manifest)
        .map_err(|error| StorageError::BadRequest(error.to_string()))
}

fn validate_manifest_path(path: &str, allow_empty: bool) -> Result<(), String> {
    if path.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err("path must not be empty".to_string())
        };
    }
    if path.starts_with('/') {
        return Err(format!("path must be repo-relative: {path}"));
    }
    if path.contains('\\') {
        return Err(format!("path must use forward slashes only: {path}"));
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(format!("path contains an empty segment: {path}"));
        }
        if segment == "." || segment == ".." {
            return Err(format!("path contains traversal segment: {path}"));
        }
        if segment.chars().any(char::is_control) {
            return Err(format!("path contains a control character: {path}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::{Fields, NodeMetadata, SCHEMA_VERSION};

    use super::*;

    #[test]
    fn round_trips_manifest_document() {
        let snapshot = ScannedSubtree {
            files: vec![ScannedFile {
                path: "about.md".to_string(),
                meta: NodeMetadata {
                    schema: SCHEMA_VERSION,
                    kind: NodeKind::Page,
                    authored: Fields {
                        title: Some("About".to_string()),
                        date: Some("2026-04-26".to_string()),
                        tags: Some(vec!["intro".to_string()]),
                        ..Fields::default()
                    },
                    derived: Fields {
                        size_bytes: Some(7),
                        modified_at: Some(42),
                        ..Fields::default()
                    },
                },
                extensions: EntryExtensions::default(),
            }],
            directories: vec![ScannedDirectory {
                path: String::new(),
                meta: NodeMetadata {
                    schema: SCHEMA_VERSION,
                    kind: NodeKind::Directory,
                    authored: Fields {
                        title: Some("Home".to_string()),
                        tags: Some(vec!["root".to_string()]),
                        ..Fields::default()
                    },
                    derived: Fields::default(),
                },
            }],
        };

        let encoded = serialize_manifest_snapshot(&snapshot).expect("serialize");
        let decoded = parse_manifest_snapshot(&encoded).expect("parse");
        assert_eq!(decoded, snapshot);
    }

    #[test]
    fn rejects_manifest_paths_with_traversal_segments() {
        let manifest = r#"{
            "entries": [
                {"path":"../secret.md","metadata":{"schema":1,"kind":"page","authored":{},"derived":{}}}
            ]
        }"#;

        let err = parse_manifest_snapshot(manifest).unwrap_err();
        assert!(matches!(err, StorageError::ValidationFailed(_)));
    }
}
