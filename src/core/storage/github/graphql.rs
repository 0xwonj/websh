//! Pure building of GraphQL commit payloads. No HTTP, no signals.
//! See spec §4.2.
//!
//! Note: `#[allow(dead_code)]` is applied at module scope because the public
//! API (`build_file_changes`, `CreateCommitInput`, ...) is consumed by
//! `client.rs` in Task 2.3. Tests cover every item, so dead-code lint fires
//! only in the non-test build until the client lands.

#![allow(dead_code)]

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::Serialize;

use crate::core::changes::{ChangeSet, ChangeType};
use crate::models::VirtualPath;

#[derive(Debug, Serialize)]
pub struct CreateCommitInput {
    pub branch: BranchRef,
    pub message: CommitMessage,
    #[serde(rename = "expectedHeadOid", skip_serializing_if = "Option::is_none")]
    pub expected_head_oid: Option<String>,
    #[serde(rename = "fileChanges")]
    pub file_changes: FileChanges,
}

#[derive(Debug, Serialize)]
pub struct BranchRef {
    #[serde(rename = "repositoryNameWithOwner")]
    pub repo_with_owner: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
}

#[derive(Debug, Serialize)]
pub struct CommitMessage {
    pub headline: String,
}

#[derive(Debug, Default, Serialize)]
pub struct FileChanges {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub additions: Vec<FileAddition>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deletions: Vec<FileDeletion>,
}

#[derive(Debug, Serialize)]
pub struct FileAddition {
    pub path: String,
    pub contents: String, // base64
}

#[derive(Debug, Serialize)]
pub struct FileDeletion {
    pub path: String,
}

/// Build the fileChanges payload from the STAGED subset of the ChangeSet.
///
/// `mount_root` is stripped from canonical filesystem paths before emission,
/// then `repo_prefix` is prepended to produce repo-relative GitHub paths.
/// Paths that do not live under `mount_root` fall back to plain absolute-tail
/// stripping; this is used for backend-private files such as `/manifest.json`.
pub fn build_file_changes(
    changes: &ChangeSet,
    mount_root: &VirtualPath,
    repo_prefix: &str,
    serialized_manifest: Option<(&str, &str)>, // (repo_path, body_bytes_utf8)
) -> FileChanges {
    let mut fc = FileChanges::default();

    for (path, entry) in changes.iter_staged() {
        let repo_path = join_repo_path(mount_root, repo_prefix, path);
        match &entry.change {
            ChangeType::CreateFile { content, .. } | ChangeType::UpdateFile { content, .. } => {
                fc.additions.push(FileAddition {
                    path: repo_path,
                    contents: B64.encode(content.as_bytes()),
                });
            }
            ChangeType::CreateBinary { .. } => {
                // 3c — not reachable in 3a
                continue;
            }
            ChangeType::DeleteFile => {
                fc.deletions.push(FileDeletion { path: repo_path });
            }
            ChangeType::CreateDirectory { .. } | ChangeType::DeleteDirectory => {
                // GitHub has no empty directories; implicit. Design §4.2.
                continue;
            }
        }
    }

    if let Some((path, body)) = serialized_manifest {
        fc.additions.push(FileAddition {
            path: path.to_string(),
            contents: B64.encode(body.as_bytes()),
        });
    }

    // Sort both lists by path for deterministic GraphQL bodies.
    fc.additions.sort_by(|a, b| a.path.cmp(&b.path));
    fc.deletions.sort_by(|a, b| a.path.cmp(&b.path));

    fc
}

fn join_repo_path(mount_root: &VirtualPath, prefix: &str, path: &VirtualPath) -> String {
    let tail = path
        .strip_prefix(mount_root)
        .unwrap_or_else(|| path.as_str().trim_start_matches('/'));
    if prefix.is_empty() {
        tail.to_string()
    } else {
        format!("{}/{}", prefix.trim_matches('/'), tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::changes::ChangeType;
    use crate::models::FileMetadata;

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    #[test]
    fn additions_are_sorted_and_base64() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/site/z.md"),
            ChangeType::CreateFile {
                content: "zz".into(),
                meta: FileMetadata::default(),
            },
        );
        cs.upsert(
            p("/site/a.md"),
            ChangeType::CreateFile {
                content: "aa".into(),
                meta: FileMetadata::default(),
            },
        );
        let fc = build_file_changes(&cs, &p("/site"), "~", None);
        assert_eq!(fc.additions.len(), 2);
        assert_eq!(fc.additions[0].path, "~/a.md");
        assert_eq!(fc.additions[1].path, "~/z.md");
        assert_eq!(fc.additions[0].contents, B64.encode(b"aa"));
    }

    #[test]
    fn deletions_are_emitted() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/site/gone.md"), ChangeType::DeleteFile);
        let fc = build_file_changes(&cs, &p("/site"), "", None);
        assert_eq!(fc.deletions.len(), 1);
        assert_eq!(fc.deletions[0].path, "gone.md");
        assert!(fc.additions.is_empty());
    }

    #[test]
    fn unstaged_is_excluded() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/site/a.md"),
            ChangeType::CreateFile {
                content: "a".into(),
                meta: FileMetadata::default(),
            },
        );
        cs.unstage(&p("/site/a.md"));
        let fc = build_file_changes(&cs, &p("/site"), "", None);
        assert!(fc.additions.is_empty());
    }

    #[test]
    fn manifest_is_appended_and_sorted_in() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/site/b.md"),
            ChangeType::CreateFile {
                content: "b".into(),
                meta: FileMetadata::default(),
            },
        );
        let fc = build_file_changes(&cs, &p("/site"), "", Some(("manifest.json", "{}")));
        let paths: Vec<_> = fc.additions.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(paths, vec!["b.md", "manifest.json"]);
    }

    #[test]
    fn directory_creates_are_dropped() {
        use crate::models::DirectoryMetadata;
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/site/newdir"),
            ChangeType::CreateDirectory {
                meta: DirectoryMetadata::default(),
            },
        );
        let fc = build_file_changes(&cs, &p("/site"), "", None);
        assert!(fc.additions.is_empty());
        assert!(fc.deletions.is_empty());
    }

    #[test]
    fn mount_root_is_stripped_before_repo_prefix_is_applied() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/mnt/work/note.md"),
            ChangeType::CreateFile {
                content: "hello".into(),
                meta: FileMetadata::default(),
            },
        );

        let fc = build_file_changes(&cs, &p("/mnt/work"), "content", None);
        assert_eq!(fc.additions[0].path, "content/note.md");
    }
}
