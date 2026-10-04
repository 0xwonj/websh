use std::collections::HashMap;

use crate::domain::{EntryExtensions, FsEntry, NodeMetadata, VirtualPath};

use super::super::tree::{insert_tree_entry, remove_tree_entry};
use super::{FsMutationError, GlobalFs};

impl GlobalFs {
    pub fn upsert_file(
        &mut self,
        path: VirtualPath,
        content: String,
        meta: NodeMetadata,
        extensions: EntryExtensions,
    ) {
        self.try_upsert_file(path, content, meta, extensions)
            .expect("upsert_file requires a valid filesystem path");
    }

    pub fn try_upsert_file(
        &mut self,
        path: VirtualPath,
        content: String,
        meta: NodeMetadata,
        extensions: EntryExtensions,
    ) -> Result<(), FsMutationError> {
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::file_with_meta(meta, extensions),
        )?;
        self.inline_text.insert(path, content);
        Ok(())
    }

    pub fn upsert_directory(&mut self, path: VirtualPath, meta: NodeMetadata) {
        self.try_upsert_directory(path, meta)
            .expect("upsert_directory requires a valid filesystem path");
    }

    pub fn try_upsert_directory(
        &mut self,
        path: VirtualPath,
        meta: NodeMetadata,
    ) -> Result<(), FsMutationError> {
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::Directory {
                children: HashMap::new(),
                meta,
            },
        )?;
        self.inline_text.retain(|k, _| !k.starts_with(&path));
        Ok(())
    }

    pub fn remove_entry(&mut self, path: &VirtualPath) {
        self.inline_text.remove(path);
        remove_tree_entry(&mut self.root, path);
    }

    pub fn remove_subtree(&mut self, path: &VirtualPath) {
        self.inline_text.retain(|k, _| !k.starts_with(path));
        remove_tree_entry(&mut self.root, path);
        self.mount_points.retain(|p| !p.starts_with(path));
    }
}
