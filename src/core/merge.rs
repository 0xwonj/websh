//! Merge a `ChangeSet` overlay on top of a base `VirtualFs` to produce a
//! "current view" VirtualFs. Pure, no signals.

use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::engine::GlobalFs;
use crate::core::filesystem::VirtualFs;
use crate::core::runtime;
use crate::models::VirtualPath;

pub fn merge_view(base: &VirtualFs, changes: &ChangeSet) -> VirtualFs {
    merge_view_for_root(base, changes, &VirtualPath::root())
}

pub fn merge_view_for_root(base: &VirtualFs, changes: &ChangeSet, root: &VirtualPath) -> VirtualFs {
    let mut merged = base.clone();
    for (path, entry) in changes.iter_all() {
        apply_change(&mut merged, path, root, &entry.change);
    }
    merged
}

pub fn merge_global_view(
    base: &GlobalFs,
    changes: &ChangeSet,
    wallet_state: &crate::models::WalletState,
) -> GlobalFs {
    let mut merged = base.clone();
    runtime::populate_runtime_state(&mut merged, changes, wallet_state);
    for (path, entry) in changes.iter_all() {
        apply_global_change(&mut merged, path, &entry.change);
    }
    merged
}

fn apply_change(fs: &mut VirtualFs, path: &VirtualPath, root: &VirtualPath, change: &ChangeType) {
    let Some(scoped_path) = scoped_path(path, root) else {
        return;
    };

    match change {
        ChangeType::CreateFile { content, meta } => {
            fs.upsert_file(scoped_path.clone(), content.clone(), meta.clone());
        }
        ChangeType::CreateBinary {
            blob_id: _,
            mime: _,
            meta,
        } => {
            fs.upsert_binary_placeholder(scoped_path.clone(), meta.clone());
        }
        ChangeType::UpdateFile {
            content,
            description,
        } => {
            fs.update_file_content(&scoped_path, content.clone(), description.clone());
        }
        ChangeType::DeleteFile => {
            fs.remove_entry(&scoped_path);
        }
        ChangeType::CreateDirectory { meta } => {
            fs.upsert_directory(scoped_path.clone(), meta.clone());
        }
        ChangeType::DeleteDirectory => {
            fs.remove_subtree(&scoped_path);
        }
    }
}

fn apply_global_change(fs: &mut GlobalFs, path: &VirtualPath, change: &ChangeType) {
    match change {
        ChangeType::CreateFile { content, meta } => {
            fs.upsert_file(path.clone(), content.clone(), meta.clone());
        }
        ChangeType::CreateBinary {
            blob_id: _,
            mime: _,
            meta,
        } => {
            fs.upsert_binary_placeholder(path.clone(), meta.clone());
        }
        ChangeType::UpdateFile {
            content,
            description,
        } => {
            fs.update_file_content(path, content.clone(), description.clone());
        }
        ChangeType::DeleteFile => {
            fs.remove_entry(path);
        }
        ChangeType::CreateDirectory { meta } => {
            fs.upsert_directory(path.clone(), meta.clone());
        }
        ChangeType::DeleteDirectory => {
            fs.remove_subtree(path);
        }
    }
}

fn scoped_path(path: &VirtualPath, root: &VirtualPath) -> Option<VirtualPath> {
    if root.is_root() {
        return Some(path.clone());
    }

    let rel = path.strip_prefix(root)?;
    let scoped = if rel.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", rel)
    };
    VirtualPath::from_absolute(scoped).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DirectoryMetadata, FileMetadata, FsEntry};

    fn base() -> VirtualFs {
        VirtualFs::empty()
    }

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    #[test]
    fn create_file_appears_in_merged() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/note.md"),
            ChangeType::CreateFile {
                content: "hi".into(),
                meta: FileMetadata::default(),
            },
        );
        let merged = merge_view(&base(), &cs);
        assert!(merged.get(&p("/note.md")).is_some());
    }

    #[test]
    fn delete_removes_from_merged() {
        let mut fs = base();
        fs.upsert_file(p("/a.md"), "a".into(), FileMetadata::default());
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), ChangeType::DeleteFile);
        let merged = merge_view(&fs, &cs);
        assert!(merged.get(&p("/a.md")).is_none());
    }

    #[test]
    fn update_replaces_content() {
        let mut fs = base();
        fs.upsert_file(p("/a.md"), "old".into(), FileMetadata::default());
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/a.md"),
            ChangeType::UpdateFile {
                content: "new".into(),
                description: None,
            },
        );
        let merged = merge_view(&fs, &cs);
        let content = merged.read_file(&p("/a.md")).unwrap();
        assert_eq!(content, "new");
    }

    #[test]
    fn create_directory_appears_in_merged() {
        let mut cs = ChangeSet::new();
        let meta = DirectoryMetadata {
            title: "Notes".into(),
            description: None,
            icon: None,
            thumbnail: None,
            tags: Vec::new(),
        };
        cs.upsert(
            p("/notes"),
            ChangeType::CreateDirectory { meta: meta.clone() },
        );
        let merged = merge_view(&base(), &cs);
        let entry = merged.get(&p("/notes")).expect("directory should exist");
        match entry {
            FsEntry::Directory { meta: m, .. } => {
                assert_eq!(m.title, "Notes");
            }
            _ => panic!("expected Directory entry at /notes"),
        }
    }

    #[test]
    fn delete_directory_removes_subtree_and_pending_content() {
        let mut fs = base();
        fs.upsert_directory(
            p("/a"),
            DirectoryMetadata {
                title: "a".into(),
                description: None,
                icon: None,
                thumbnail: None,
                tags: Vec::new(),
            },
        );
        fs.upsert_file(p("/a/b.md"), "inner".into(), FileMetadata::default());
        // Sanity-check the seed.
        assert!(fs.get(&p("/a/b.md")).is_some());
        assert_eq!(fs.read_file(&p("/a/b.md")).as_deref(), Some("inner"));

        let mut cs = ChangeSet::new();
        cs.upsert(p("/a"), ChangeType::DeleteDirectory);
        let merged = merge_view(&fs, &cs);

        assert!(merged.get(&p("/a")).is_none());
        assert!(merged.read_file(&p("/a/b.md")).is_none());
    }

    #[test]
    fn create_binary_does_not_populate_pending_content() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/img.png"),
            ChangeType::CreateBinary {
                blob_id: "blob-xyz".into(),
                mime: "image/png".into(),
                meta: FileMetadata::default(),
            },
        );
        let merged = merge_view(&base(), &cs);
        let entry = merged.get(&p("/img.png")).expect("file should exist");
        assert!(matches!(entry, FsEntry::File { .. }));
        assert!(merged.read_file(&p("/img.png")).is_none());
    }

    #[test]
    fn create_file_at_nested_path_creates_parents() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/a/b/c.md"),
            ChangeType::CreateFile {
                content: "nested".into(),
                meta: FileMetadata::default(),
            },
        );
        let merged = merge_view(&base(), &cs);

        assert!(matches!(
            merged.get(&p("/a")),
            Some(FsEntry::Directory { .. })
        ));
        assert!(matches!(
            merged.get(&p("/a/b")),
            Some(FsEntry::Directory { .. })
        ));
        assert!(matches!(
            merged.get(&p("/a/b/c.md")),
            Some(FsEntry::File { .. })
        ));
    }

    #[test]
    fn update_file_updates_description() {
        let mut fs = base();
        fs.upsert_file(p("/a.md"), "old".into(), FileMetadata::default());
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/a.md"),
            ChangeType::UpdateFile {
                content: "new".into(),
                description: Some("updated desc".into()),
            },
        );
        let merged = merge_view(&fs, &cs);
        let entry = merged.get(&p("/a.md")).expect("file should exist");
        match entry {
            FsEntry::File { description, .. } => {
                assert_eq!(description, "updated desc");
            }
            _ => panic!("expected File entry at /a.md"),
        }
    }

    #[test]
    fn merge_view_for_root_strips_mount_prefix() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/site/note.md"),
            ChangeType::CreateFile {
                content: "hi".into(),
                meta: FileMetadata::default(),
            },
        );
        let merged = merge_view_for_root(&base(), &cs, &p("/site"));
        assert!(merged.get(&p("/note.md")).is_some());
        assert!(merged.get(&p("/site/note.md")).is_none());
    }

    #[test]
    fn merge_view_for_root_ignores_other_mounts() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/mnt/work/note.md"),
            ChangeType::CreateFile {
                content: "hi".into(),
                meta: FileMetadata::default(),
            },
        );
        let merged = merge_view_for_root(&base(), &cs, &p("/site"));
        assert!(merged.get(&p("/note.md")).is_none());
    }
}
