//! Merge a `ChangeSet` overlay on top of a canonical `GlobalFs` view. Pure, no signals.

use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::engine::GlobalFs;
use crate::models::VirtualPath;

pub fn merge_global_view(base: &GlobalFs, changes: &ChangeSet) -> GlobalFs {
    let mut merged = base.clone();
    apply_all_changes_to_global(&mut merged, changes);
    merged
}

pub fn apply_all_changes_to_global(fs: &mut GlobalFs, changes: &ChangeSet) {
    for (path, entry) in changes.iter_all() {
        apply_global_change(fs, path, &entry.change);
    }
}

pub fn apply_staged_changes_to_global_for_root(
    fs: &mut GlobalFs,
    changes: &ChangeSet,
    root: &VirtualPath,
) {
    for (path, entry) in changes.iter_staged() {
        if path.starts_with(root) {
            apply_global_change(fs, path, &entry.change);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DirectoryMetadata, FileMetadata, FsEntry};

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    fn base() -> GlobalFs {
        GlobalFs::empty()
    }

    fn merged(base: &GlobalFs, changes: &ChangeSet) -> GlobalFs {
        merge_global_view(base, changes)
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
        let merged = merged(&base(), &cs);
        assert!(merged.get_entry(&p("/note.md")).is_some());
    }

    #[test]
    fn delete_removes_from_merged() {
        let mut fs = base();
        fs.upsert_file(p("/a.md"), "a".into(), FileMetadata::default());
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), ChangeType::DeleteFile);
        let merged = merged(&fs, &cs);
        assert!(merged.get_entry(&p("/a.md")).is_none());
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
        let merged = merged(&fs, &cs);
        let content = merged.read_pending_text(&p("/a.md")).unwrap();
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
        let merged = merged(&base(), &cs);
        let entry = merged
            .get_entry(&p("/notes"))
            .expect("directory should exist");
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
        assert!(fs.get_entry(&p("/a/b.md")).is_some());
        assert_eq!(
            fs.read_pending_text(&p("/a/b.md")).as_deref(),
            Some("inner")
        );

        let mut cs = ChangeSet::new();
        cs.upsert(p("/a"), ChangeType::DeleteDirectory);
        let merged = merged(&fs, &cs);

        assert!(merged.get_entry(&p("/a")).is_none());
        assert!(merged.read_pending_text(&p("/a/b.md")).is_none());
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
        let merged = merged(&base(), &cs);
        let entry = merged.get_entry(&p("/img.png")).expect("file should exist");
        assert!(matches!(entry, FsEntry::File { .. }));
        assert!(merged.read_pending_text(&p("/img.png")).is_none());
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
        let merged = merged(&base(), &cs);

        assert!(matches!(
            merged.get_entry(&p("/a")),
            Some(FsEntry::Directory { .. })
        ));
        assert!(matches!(
            merged.get_entry(&p("/a/b")),
            Some(FsEntry::Directory { .. })
        ));
        assert!(matches!(
            merged.get_entry(&p("/a/b/c.md")),
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
        let merged = merged(&fs, &cs);
        let entry = merged.get_entry(&p("/a.md")).expect("file should exist");
        match entry {
            FsEntry::File { description, .. } => {
                assert_eq!(description, "updated desc");
            }
            _ => panic!("expected File entry at /a.md"),
        }
    }

    #[test]
    fn staged_root_apply_keeps_root_paths() {
        let mut fs = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/note.md"),
            ChangeType::CreateFile {
                content: "hi".into(),
                meta: FileMetadata::default(),
            },
        );
        apply_staged_changes_to_global_for_root(&mut fs, &cs, &VirtualPath::root());
        assert!(fs.get_entry(&p("/note.md")).is_some());
    }

    #[test]
    fn staged_root_apply_ignores_other_mounts() {
        let mut fs = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/work/note.md"),
            ChangeType::CreateFile {
                content: "hi".into(),
                meta: FileMetadata::default(),
            },
        );
        apply_staged_changes_to_global_for_root(&mut fs, &cs, &p("/db"));
        assert!(fs.get_entry(&p("/work/note.md")).is_none());
    }

    #[test]
    fn staged_root_apply_ignores_unstaged_changes() {
        let mut fs = GlobalFs::empty();
        let mut cs = ChangeSet::new();
        let path = p("/draft.md");
        cs.upsert(
            path.clone(),
            ChangeType::CreateFile {
                content: "draft".into(),
                meta: FileMetadata::default(),
            },
        );
        cs.unstage(&path);

        apply_staged_changes_to_global_for_root(&mut fs, &cs, &VirtualPath::root());
        assert!(fs.get_entry(&path).is_none());
    }
}
