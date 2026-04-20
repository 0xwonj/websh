//! Merge a `ChangeSet` overlay on top of a base `VirtualFs` to produce a
//! "current view" VirtualFs. Pure, no signals.

use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::filesystem::VirtualFs;
use crate::models::VirtualPath;

pub fn merge_view(base: &VirtualFs, changes: &ChangeSet) -> VirtualFs {
    let mut merged = base.clone();
    for (path, entry) in changes.iter_all() {
        apply_change(&mut merged, path, &entry.change);
    }
    merged
}

fn apply_change(fs: &mut VirtualFs, path: &VirtualPath, change: &ChangeType) {
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
    use crate::models::FileMetadata;

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
}
