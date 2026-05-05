use crate::domain::{FsEntry, VirtualPath};
use crate::ports::ScannedSubtree;

use super::super::snapshot::scanned_subtree_root;
use super::super::tree::{insert_tree_entry, synthetic_directory};
use super::{GlobalFs, MountError};

impl GlobalFs {
    pub fn mount_scanned_subtree(
        &mut self,
        mount_at: VirtualPath,
        snapshot: &ScannedSubtree,
    ) -> Result<(), MountError> {
        self.mount_subtree(mount_at, scanned_subtree_root(snapshot))
    }

    pub fn reserve_mount_point(&mut self, mount_at: VirtualPath) -> Result<(), MountError> {
        if mount_at.is_root() {
            self.mount_points.insert(mount_at);
            return Ok(());
        }

        if let Some(existing) = self.get_entry(&mount_at) {
            if existing.is_directory() {
                self.remove_subtree(&mount_at);
            } else {
                return Err(MountError::MountPointIsFile { path: mount_at });
            }
        }

        let name = mount_at.file_name().unwrap_or_default().to_string();
        self.mount_subtree(mount_at, synthetic_directory(&name))
    }

    pub fn replace_scanned_subtree(
        &mut self,
        mount_at: VirtualPath,
        snapshot: &ScannedSubtree,
    ) -> Result<(), MountError> {
        if mount_at.is_root() {
            return self.mount_scanned_subtree(mount_at, snapshot);
        }

        if let Some(existing) = self.get_entry(&mount_at)
            && !existing.is_directory()
        {
            return Err(MountError::MountPointIsFile { path: mount_at });
        }

        self.remove_subtree(&mount_at);
        self.mount_scanned_subtree(mount_at, snapshot)
    }

    pub fn mount_subtree(
        &mut self,
        mount_at: VirtualPath,
        subtree: FsEntry,
    ) -> Result<(), MountError> {
        if mount_at.is_root() {
            if !subtree.is_directory() {
                return Err(MountError::RootMustBeDirectory);
            }
            self.root = subtree;
            self.mount_points.clear();
            self.mount_points.insert(mount_at);
            return Ok(());
        }

        let parts: Vec<&str> = mount_at.segments().collect();
        let mut current = match &mut self.root {
            FsEntry::Directory { children, .. } => children,
            FsEntry::File { .. } => return Err(MountError::RootMustBeDirectory),
        };

        let mut current_path = VirtualPath::root();
        for (idx, part) in parts.iter().enumerate() {
            current_path = current_path.join(part);
            let is_last = idx == parts.len() - 1;

            if is_last {
                if let Some(existing) = current.get(*part) {
                    return Err(match existing {
                        FsEntry::File { .. } => MountError::MountPointIsFile { path: current_path },
                        FsEntry::Directory { .. } => {
                            MountError::MountPointOccupied { path: current_path }
                        }
                    });
                }
                current.insert((*part).to_string(), subtree);
                self.mount_points.insert(mount_at);
                return Ok(());
            }

            let slot = current
                .entry((*part).to_string())
                .or_insert_with(|| synthetic_directory(part));

            current = match slot {
                FsEntry::Directory { children, .. } => children,
                FsEntry::File { .. } => {
                    return Err(MountError::ParentIsFile { path: current_path });
                }
            };
        }

        Ok(())
    }

    pub fn replace_subtree(&mut self, mount_at: VirtualPath, subtree: FsEntry) {
        self.remove_subtree(&mount_at);
        if subtree.is_directory() {
            self.pending_text
                .retain(|path, _| !path.starts_with(&mount_at));
        }
        insert_tree_entry(&mut self.root, &mount_at, subtree)
            .expect("replace_subtree requires a valid mount path");
        self.mount_points.insert(mount_at);
    }
}
