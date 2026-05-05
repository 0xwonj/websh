use std::collections::HashMap;

use crate::domain::{EntryExtensions, FsEntry, NodeMetadata, VirtualPath};

use super::super::tree::{get_tree_entry_mut, insert_tree_entry, remove_tree_entry};
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
            FsEntry::content_file_with_meta("", meta, extensions),
        )?;
        self.pending_text.insert(path, content);
        Ok(())
    }

    pub fn upsert_binary_placeholder(
        &mut self,
        path: VirtualPath,
        meta: NodeMetadata,
        extensions: EntryExtensions,
    ) {
        self.try_upsert_binary_placeholder(path, meta, extensions)
            .expect("upsert_binary_placeholder requires a valid filesystem path");
    }

    pub fn try_upsert_binary_placeholder(
        &mut self,
        path: VirtualPath,
        meta: NodeMetadata,
        extensions: EntryExtensions,
    ) -> Result<(), FsMutationError> {
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::content_file_with_meta("", meta, extensions),
        )?;
        self.pending_text.remove(&path);
        Ok(())
    }

    /// Apply an in-place edit to an existing file. `meta` / `extensions`,
    /// when `Some`, replace the file's manifest-side state — required so
    /// subsequent `export_mount_snapshot` calls see fresh values. `None`
    /// preserves whatever the base scan had (used by terminal `edit` /
    /// `echo >` commands that legitimately have no new metadata).
    pub fn update_file(
        &mut self,
        path: &VirtualPath,
        content: String,
        meta: Option<NodeMetadata>,
        extensions: Option<EntryExtensions>,
    ) {
        self.try_update_file(path, content, meta, extensions)
            .expect("update_file requires an existing file target");
    }

    pub fn try_update_file(
        &mut self,
        path: &VirtualPath,
        content: String,
        meta: Option<NodeMetadata>,
        extensions: Option<EntryExtensions>,
    ) -> Result<(), FsMutationError> {
        let Some(FsEntry::File {
            meta: existing_meta,
            extensions: existing_ext,
            ..
        }) = get_tree_entry_mut(&mut self.root, path)
        else {
            return match self.get_entry(path) {
                Some(FsEntry::Directory { .. }) => {
                    Err(FsMutationError::TargetIsDirectory { path: path.clone() })
                }
                _ => Err(FsMutationError::TargetMissing { path: path.clone() }),
            };
        };
        if let Some(new_meta) = meta {
            *existing_meta = new_meta;
        }
        if let Some(new_ext) = extensions {
            *existing_ext = new_ext;
        }
        self.pending_text.insert(path.clone(), content);
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
        self.pending_text.retain(|k, _| !k.starts_with(&path));
        Ok(())
    }

    pub fn remove_entry(&mut self, path: &VirtualPath) {
        self.pending_text.remove(path);
        remove_tree_entry(&mut self.root, path);
    }

    pub fn remove_subtree(&mut self, path: &VirtualPath) {
        self.pending_text.retain(|k, _| !k.starts_with(path));
        remove_tree_entry(&mut self.root, path);
        self.mount_points.retain(|p| !p.starts_with(path));
    }
}
