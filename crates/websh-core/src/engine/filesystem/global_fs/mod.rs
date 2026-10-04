use std::collections::{BTreeMap, BTreeSet};

use thiserror::Error;

use crate::domain::{FsEntry, NodeMetadata, VirtualPath};

use super::tree::directory_metadata;

mod mount;
mod mutation;
mod query;
#[cfg(test)]
mod tests;

/// Error returned when assembling a global tree from mounted subtrees.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum MountError {
    #[error("mount root must be a directory")]
    RootMustBeDirectory,
    #[error("mount parent is a file: {path}")]
    ParentIsFile { path: VirtualPath },
    #[error("mount point is a file: {path}")]
    MountPointIsFile { path: VirtualPath },
    #[error("mount point is already occupied: {path}")]
    MountPointOccupied { path: VirtualPath },
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum FsMutationError {
    #[error("filesystem root must be a directory")]
    RootMustBeDirectory,
    #[error("parent is a file: {path}")]
    ParentIsFile { path: VirtualPath },
}

/// Global filesystem assembled from mounted subtrees plus local overlays.
#[derive(Clone, Debug)]
pub struct GlobalFs {
    root: FsEntry,
    mount_points: BTreeSet<VirtualPath>,
    inline_text: BTreeMap<VirtualPath, String>,
}

impl GlobalFs {
    pub fn empty() -> Self {
        Self {
            root: FsEntry::Directory {
                children: Default::default(),
                meta: directory_metadata(""),
            },
            mount_points: BTreeSet::new(),
            inline_text: BTreeMap::new(),
        }
    }

    pub fn mount_points(&self) -> impl Iterator<Item = &VirtualPath> {
        self.mount_points.iter()
    }

    /// Returns the unified metadata for the node at `path`, if any. The
    /// metadata lives directly inside the [`FsEntry`] so this is a tree
    /// lookup rather than a separate map.
    pub fn node_metadata(&self, path: &VirtualPath) -> Option<&NodeMetadata> {
        self.get_entry(path).map(|entry| entry.meta())
    }

    pub fn read_inline_text(&self, path: &VirtualPath) -> Option<String> {
        self.inline_text.get(path).cloned()
    }
}

impl Default for GlobalFs {
    fn default() -> Self {
        Self::empty()
    }
}
