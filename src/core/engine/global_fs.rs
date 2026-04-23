use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::core::filesystem::{DirEntry, VirtualFs};
use crate::models::{
    DirectoryMetadata, DisplayPermissions, FileMetadata, FsEntry, LoadedNodeMetadata,
    RouteIndexEntry, VirtualPath, WalletState,
};

use super::intent::{RenderIntent, build_render_intent};
use super::routing::{RouteRequest, RouteResolution, resolve_route};

/// Error returned when assembling a global tree from mounted subtrees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MountError {
    RootMustBeDirectory,
    ParentIsFile { path: VirtualPath },
    MountPointIsFile { path: VirtualPath },
    MountPointOccupied { path: VirtualPath },
}

/// Minimal engine trait for the canonical-path read surface.
pub trait FsEngine {
    fn stat(&self, path: &VirtualPath) -> Option<&FsEntry>;
    fn list(&self, path: &VirtualPath) -> Option<Vec<DirEntry>>;
    fn resolve_route(&self, request: &RouteRequest) -> Option<RouteResolution>;
    fn build_render_intent(&self, resolution: &RouteResolution) -> Option<RenderIntent>;
}

/// Global filesystem assembled from mounted subtrees plus local overlays.
#[derive(Clone, Debug)]
pub struct GlobalFs {
    root: FsEntry,
    mount_points: BTreeSet<VirtualPath>,
    pending_text: BTreeMap<VirtualPath, String>,
    node_metadata: BTreeMap<VirtualPath, LoadedNodeMetadata>,
    route_index: BTreeMap<String, RouteIndexEntry>,
}

impl GlobalFs {
    pub fn empty() -> Self {
        Self {
            root: FsEntry::Directory {
                children: Default::default(),
                meta: DirectoryMetadata::default(),
            },
            mount_points: BTreeSet::new(),
            pending_text: BTreeMap::new(),
            node_metadata: BTreeMap::new(),
            route_index: BTreeMap::new(),
        }
    }

    pub fn mount_points(&self) -> impl Iterator<Item = &VirtualPath> {
        self.mount_points.iter()
    }

    pub fn node_metadata(&self, path: &VirtualPath) -> Option<&LoadedNodeMetadata> {
        self.node_metadata.get(path)
    }

    pub fn metadata_entries(
        &self,
    ) -> impl Iterator<Item = (&VirtualPath, &LoadedNodeMetadata)> + '_ {
        self.node_metadata.iter()
    }

    pub fn set_node_metadata(&mut self, path: VirtualPath, meta: LoadedNodeMetadata) {
        self.node_metadata.insert(path, meta);
    }

    pub fn replace_route_index(&mut self, routes: impl IntoIterator<Item = RouteIndexEntry>) {
        self.route_index = routes
            .into_iter()
            .map(|entry| (entry.route.clone(), entry))
            .collect();
    }

    pub fn route_entry(&self, route: &str) -> Option<&RouteIndexEntry> {
        self.route_index.get(route)
    }

    pub fn read_pending_text(&self, path: &VirtualPath) -> Option<String> {
        self.pending_text.get(path).cloned()
    }

    pub(crate) fn mount_fs(
        &mut self,
        mount_at: VirtualPath,
        fs: &VirtualFs,
    ) -> Result<(), MountError> {
        self.mount_subtree(mount_at, fs.clone_root_entry())
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
        insert_tree_entry(&mut self.root, &mount_at, subtree);
        self.mount_points.insert(mount_at);
    }

    pub fn get_entry(&self, path: &VirtualPath) -> Option<&FsEntry> {
        if path.is_root() {
            return Some(&self.root);
        }

        let mut current = &self.root;
        for part in path.segments() {
            current = match current {
                FsEntry::Directory { children, .. } => children.get(part)?,
                FsEntry::File { .. } => return None,
            };
        }
        Some(current)
    }

    pub fn exists(&self, path: &VirtualPath) -> bool {
        self.get_entry(path).is_some()
    }

    pub fn is_directory(&self, path: &VirtualPath) -> bool {
        matches!(self.get_entry(path), Some(FsEntry::Directory { .. }))
    }

    pub fn has_children(&self, path: &VirtualPath) -> bool {
        matches!(
            self.get_entry(path),
            Some(FsEntry::Directory { children, .. }) if !children.is_empty()
        )
    }

    pub fn list_dir(&self, path: &VirtualPath) -> Option<Vec<DirEntry>> {
        match self.get_entry(path)? {
            FsEntry::Directory { children, .. } => Some(sorted_dir_entries(path, children)),
            FsEntry::File { .. } => None,
        }
    }

    pub fn get_permissions(
        &self,
        entry: &FsEntry,
        wallet: &WalletState,
        writable: bool,
    ) -> DisplayPermissions {
        let is_dir = entry.is_directory();
        let read = match entry {
            FsEntry::Directory { .. } => true,
            FsEntry::File { meta, .. } => match &meta.access {
                None => true,
                Some(filter) => match wallet {
                    WalletState::Connected { address, .. } => filter
                        .recipients
                        .iter()
                        .any(|r| r.address.eq_ignore_ascii_case(address)),
                    _ => false,
                },
            },
        };

        DisplayPermissions {
            is_dir,
            read,
            write: writable,
            execute: is_dir,
        }
    }

    pub fn upsert_file(&mut self, path: VirtualPath, content: String, meta: FileMetadata) {
        self.pending_text.insert(path.clone(), content);
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::content_file_with_meta("", "", meta),
        );
    }

    pub fn upsert_binary_placeholder(&mut self, path: VirtualPath, meta: FileMetadata) {
        self.pending_text.remove(&path);
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::content_file_with_meta("", "", meta),
        );
    }

    pub fn update_file_content(
        &mut self,
        path: &VirtualPath,
        content: String,
        description: Option<String>,
    ) {
        let Some(FsEntry::File { description: d, .. }) = get_tree_entry_mut(&mut self.root, path)
        else {
            return;
        };
        if let Some(new_d) = description {
            *d = new_d;
        }
        self.pending_text.insert(path.clone(), content);
    }

    pub fn upsert_directory(&mut self, path: VirtualPath, meta: DirectoryMetadata) {
        self.pending_text.retain(|k, _| !k.starts_with(&path));
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::Directory {
                children: HashMap::new(),
                meta,
            },
        );
    }

    pub fn remove_entry(&mut self, path: &VirtualPath) {
        self.pending_text.remove(path);
        self.node_metadata.remove(path);
        remove_tree_entry(&mut self.root, path);
    }

    pub fn remove_subtree(&mut self, path: &VirtualPath) {
        self.pending_text.retain(|k, _| !k.starts_with(path));
        self.node_metadata.retain(|k, _| !k.starts_with(path));
        remove_tree_entry(&mut self.root, path);
        self.mount_points.retain(|p| !p.starts_with(path));
    }
}

impl Default for GlobalFs {
    fn default() -> Self {
        Self::empty()
    }
}

impl FsEngine for GlobalFs {
    fn stat(&self, path: &VirtualPath) -> Option<&FsEntry> {
        self.get_entry(path)
    }

    fn list(&self, path: &VirtualPath) -> Option<Vec<DirEntry>> {
        self.list_dir(path)
    }

    fn resolve_route(&self, request: &RouteRequest) -> Option<RouteResolution> {
        resolve_route(self, request)
    }

    fn build_render_intent(&self, resolution: &RouteResolution) -> Option<RenderIntent> {
        build_render_intent(self, resolution)
    }
}

fn synthetic_directory(name: &str) -> FsEntry {
    FsEntry::Directory {
        children: Default::default(),
        meta: DirectoryMetadata {
            title: name.to_string(),
            ..Default::default()
        },
    }
}

fn sorted_dir_entries(base: &VirtualPath, children: &HashMap<String, FsEntry>) -> Vec<DirEntry> {
    let mut items: Vec<_> = children
        .iter()
        .map(|(name, entry)| {
            let is_dir = entry.is_directory();
            let (title, file_meta) = match entry {
                FsEntry::Directory { meta, .. } => (meta.title.clone(), None),
                FsEntry::File {
                    description, meta, ..
                } => (description.clone(), Some(meta.clone())),
            };
            DirEntry {
                name: name.clone(),
                path: base.join(name),
                is_dir,
                title,
                file_meta,
            }
        })
        .collect();

    items.sort_by(|a, b| {
        let a_hidden = a.name.starts_with('.');
        let b_hidden = b.name.starts_with('.');

        match (a.is_dir, b.is_dir, a_hidden, b_hidden) {
            (true, false, _, _) => std::cmp::Ordering::Less,
            (false, true, _, _) => std::cmp::Ordering::Greater,
            (_, _, false, true) => std::cmp::Ordering::Less,
            (_, _, true, false) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    items
}

fn insert_tree_entry(root: &mut FsEntry, path: &VirtualPath, entry: FsEntry) {
    let parts: Vec<&str> = path.segments().collect();
    let mut current = match root {
        FsEntry::Directory { children, .. } => children,
        FsEntry::File { .. } => return,
    };

    for (idx, part) in parts.iter().enumerate() {
        let is_last = idx == parts.len() - 1;

        if is_last {
            current.insert((*part).to_string(), entry);
            return;
        }

        let slot = current
            .entry((*part).to_string())
            .or_insert_with(|| synthetic_directory(part));
        if matches!(slot, FsEntry::File { .. }) {
            *slot = synthetic_directory(part);
        }
        current = match slot {
            FsEntry::Directory { children, .. } => children,
            FsEntry::File { .. } => unreachable!(),
        };
    }
}

fn remove_tree_entry(root: &mut FsEntry, path: &VirtualPath) {
    let parts: Vec<&str> = path.segments().collect();
    if parts.is_empty() {
        if let FsEntry::Directory { children, .. } = root {
            children.clear();
        }
        return;
    }

    let mut current = match root {
        FsEntry::Directory { children, .. } => children,
        FsEntry::File { .. } => return,
    };

    for part in &parts[..parts.len() - 1] {
        current = match current.get_mut(*part) {
            Some(FsEntry::Directory { children, .. }) => children,
            _ => return,
        };
    }

    current.remove(parts.last().copied().unwrap_or_default());
}

fn get_tree_entry_mut<'a>(root: &'a mut FsEntry, path: &VirtualPath) -> Option<&'a mut FsEntry> {
    if path.is_root() {
        return Some(root);
    }

    let mut current = root;
    for part in path.segments() {
        current = match current {
            FsEntry::Directory { children, .. } => children.get_mut(part)?,
            FsEntry::File { .. } => return None,
        };
    }

    Some(current)
}

#[cfg(test)]
mod tests {
    use crate::core::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};
    use crate::models::FileMetadata;

    use super::*;

    fn fs(files: &[&str], directories: &[&str]) -> VirtualFs {
        let snapshot = ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    description: (*path).to_string(),
                    meta: FileMetadata::default(),
                })
                .collect(),
            directories: directories
                .iter()
                .map(|path| ScannedDirectory {
                    path: (*path).to_string(),
                    meta: DirectoryMetadata {
                        title: path.rsplit('/').next().unwrap_or(path).to_string(),
                        ..Default::default()
                    },
                })
                .collect(),
        };
        VirtualFs::from_scanned_subtree(&snapshot)
    }

    #[test]
    fn mounts_virtual_filesystems_under_canonical_prefixes() {
        let mut global = GlobalFs::empty();
        let site = fs(&["index.html", "about.md"], &["blog"]);
        let db = fs(&["notes/todo.md"], &["notes"]);

        global
            .mount_fs(VirtualPath::from_absolute("/site").unwrap(), &site)
            .unwrap();
        global
            .mount_fs(VirtualPath::from_absolute("/mnt/db").unwrap(), &db)
            .unwrap();

        assert!(
            global
                .get_entry(&VirtualPath::from_absolute("/site/index.html").unwrap())
                .is_some()
        );
        assert!(
            global
                .get_entry(&VirtualPath::from_absolute("/mnt/db/notes/todo.md").unwrap())
                .is_some()
        );
    }

    #[test]
    fn refuses_to_replace_existing_directory_mountpoint() {
        let mut global = GlobalFs::empty();
        global
            .mount_fs(
                VirtualPath::from_absolute("/site").unwrap(),
                &fs(&["index.md"], &[]),
            )
            .unwrap();

        let err = global
            .mount_fs(
                VirtualPath::from_absolute("/site").unwrap(),
                &fs(&["other.md"], &[]),
            )
            .unwrap_err();

        assert_eq!(
            err,
            MountError::MountPointOccupied {
                path: VirtualPath::from_absolute("/site").unwrap()
            }
        );
    }

    #[test]
    fn remounting_root_replaces_mount_registry() {
        let mut global = GlobalFs::empty();
        global
            .mount_subtree(
                VirtualPath::root(),
                FsEntry::Directory {
                    children: HashMap::new(),
                    meta: DirectoryMetadata::default(),
                },
            )
            .unwrap();

        let points: Vec<_> = global
            .mount_points()
            .map(|p| p.as_str().to_string())
            .collect();
        assert_eq!(points, vec!["/"]);
    }

    #[test]
    fn list_dir_uses_global_absolute_paths() {
        let mut global = GlobalFs::empty();
        global
            .mount_fs(
                VirtualPath::from_absolute("/site").unwrap(),
                &fs(&["blog/hello.md"], &["blog"]),
            )
            .unwrap();

        let entries = global
            .list_dir(&VirtualPath::from_absolute("/site/blog").unwrap())
            .unwrap();

        assert_eq!(entries[0].path.as_str(), "/site/blog/hello.md");
    }

    #[test]
    fn pending_text_tracks_upserts() {
        let mut global = GlobalFs::empty();
        let path = VirtualPath::from_absolute("/site/new.md").unwrap();
        global.upsert_file(path.clone(), "hello".to_string(), FileMetadata::default());

        assert_eq!(global.read_pending_text(&path).as_deref(), Some("hello"));
    }
}
