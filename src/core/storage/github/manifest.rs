use serde::{Deserialize, Serialize};

use crate::core::storage::{ScannedDirectory, ScannedFile, ScannedSubtree, StorageError, StorageResult};
use crate::models::{AccessFilter, DirectoryMetadata, FileMetadata};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct ManifestDocument {
    files: Vec<ManifestFile>,
    directories: Vec<ManifestDirectory>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestFile {
    path: String,
    title: String,
    size: Option<u64>,
    modified: Option<u64>,
    tags: Vec<String>,
    access: Option<AccessFilter>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ManifestDirectory {
    path: String,
    title: String,
    tags: Vec<String>,
    description: Option<String>,
    icon: Option<String>,
    thumbnail: Option<String>,
}

pub(crate) fn parse_snapshot(body: &str) -> StorageResult<ScannedSubtree> {
    let manifest: ManifestDocument =
        serde_json::from_str(body).map_err(|error| StorageError::ValidationFailed(error.to_string()))?;

    Ok(ScannedSubtree {
        files: manifest
            .files
            .into_iter()
            .map(|file| ScannedFile {
                path: file.path,
                description: file.title,
                meta: FileMetadata {
                    size: file.size,
                    modified: file.modified,
                    tags: file.tags,
                    access: file.access,
                },
            })
            .collect(),
        directories: manifest
            .directories
            .into_iter()
            .map(|dir| ScannedDirectory {
                path: dir.path,
                meta: DirectoryMetadata {
                    title: dir.title,
                    description: dir.description,
                    icon: dir.icon,
                    thumbnail: dir.thumbnail,
                    tags: dir.tags,
                },
            })
            .collect(),
    })
}

pub(crate) fn serialize_snapshot(snapshot: &ScannedSubtree) -> StorageResult<String> {
    let manifest = ManifestDocument {
        files: snapshot
            .files
            .iter()
            .map(|file| ManifestFile {
                path: file.path.clone(),
                title: file.description.clone(),
                size: file.meta.size,
                modified: file.meta.modified,
                tags: file.meta.tags.clone(),
                access: file.meta.access.clone(),
            })
            .collect(),
        directories: snapshot
            .directories
            .iter()
            .map(|dir| ManifestDirectory {
                path: dir.path.clone(),
                title: dir.meta.title.clone(),
                tags: dir.meta.tags.clone(),
                description: dir.meta.description.clone(),
                icon: dir.meta.icon.clone(),
                thumbnail: dir.meta.thumbnail.clone(),
            })
            .collect(),
    };

    serde_json::to_string_pretty(&manifest)
        .map_err(|error| StorageError::BadRequest(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_manifest_document() {
        let snapshot = ScannedSubtree {
            files: vec![ScannedFile {
                path: "about.md".to_string(),
                description: "About".to_string(),
                meta: FileMetadata {
                    size: Some(7),
                    modified: Some(42),
                    tags: vec!["intro".to_string()],
                    access: None,
                },
            }],
            directories: vec![ScannedDirectory {
                path: String::new(),
                meta: DirectoryMetadata {
                    title: "Home".to_string(),
                    description: Some("Root".to_string()),
                    icon: None,
                    thumbnail: None,
                    tags: vec!["root".to_string()],
                },
            }],
        };

        let encoded = serialize_snapshot(&snapshot).expect("serialize");
        let decoded = parse_snapshot(&encoded).expect("parse");
        assert_eq!(decoded, snapshot);
    }
}
