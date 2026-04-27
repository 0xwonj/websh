use crate::core::storage::{
    ScannedDirectory, ScannedFile, ScannedSubtree, StorageError, StorageResult,
};
use crate::models::manifest::{
    ContentManifestDirectory as ManifestDirectory, ContentManifestDocument as ManifestDocument,
    ContentManifestFile as ManifestFile,
};
use crate::models::{DirectoryMetadata, FileMetadata};

use super::path::validate_repo_relative_path;

pub(crate) fn parse_snapshot(body: &str) -> StorageResult<ScannedSubtree> {
    let manifest: ManifestDocument = serde_json::from_str(body)
        .map_err(|error| StorageError::ValidationFailed(error.to_string()))?;

    Ok(ScannedSubtree {
        files: manifest
            .files
            .into_iter()
            .map(|file| {
                validate_repo_relative_path(&file.path, false)
                    .map_err(StorageError::ValidationFailed)?;
                Ok(ScannedFile {
                    path: file.path,
                    description: file.title,
                    meta: FileMetadata {
                        size: file.size,
                        modified: file.modified,
                        date: file.date,
                        tags: file.tags,
                        access: file.access,
                    },
                })
            })
            .collect::<StorageResult<Vec<_>>>()?,
        directories: manifest
            .directories
            .into_iter()
            .map(|dir| {
                validate_repo_relative_path(&dir.path, true)
                    .map_err(StorageError::ValidationFailed)?;
                Ok(ScannedDirectory {
                    path: dir.path,
                    meta: DirectoryMetadata {
                        title: dir.title,
                        description: dir.description,
                        icon: dir.icon,
                        thumbnail: dir.thumbnail,
                        tags: dir.tags,
                    },
                })
            })
            .collect::<StorageResult<Vec<_>>>()?,
    })
}

pub(crate) fn serialize_snapshot(snapshot: &ScannedSubtree) -> StorageResult<String> {
    let manifest = ManifestDocument {
        files: snapshot
            .files
            .iter()
            .map(|file| {
                validate_repo_relative_path(&file.path, false).map_err(StorageError::BadRequest)?;
                Ok(ManifestFile {
                    path: file.path.clone(),
                    title: file.description.clone(),
                    size: file.meta.size,
                    modified: file.meta.modified,
                    date: file.meta.date.clone(),
                    tags: file.meta.tags.clone(),
                    access: file.meta.access.clone(),
                })
            })
            .collect::<StorageResult<Vec<_>>>()?,
        directories: snapshot
            .directories
            .iter()
            .map(|dir| {
                validate_repo_relative_path(&dir.path, true).map_err(StorageError::BadRequest)?;
                Ok(ManifestDirectory {
                    path: dir.path.clone(),
                    title: dir.meta.title.clone(),
                    tags: dir.meta.tags.clone(),
                    description: dir.meta.description.clone(),
                    icon: dir.meta.icon.clone(),
                    thumbnail: dir.meta.thumbnail.clone(),
                })
            })
            .collect::<StorageResult<Vec<_>>>()?,
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
                    date: Some("2026-04-26".to_string()),
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

    #[test]
    fn rejects_manifest_paths_with_traversal_segments() {
        let manifest = r#"{
            "files": [
                {"path":"../secret.md","title":"Secret","size":null,"modified":null,"tags":[],"access":null}
            ],
            "directories": []
        }"#;

        let err = parse_snapshot(manifest).unwrap_err();
        assert!(matches!(err, StorageError::ValidationFailed(_)));
    }
}
