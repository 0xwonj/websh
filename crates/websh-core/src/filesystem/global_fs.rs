use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::domain::{
    DirEntry, DisplayPermissions, EntryExtensions, Fields, FsEntry, NodeKind, NodeMetadata,
    RouteIndexEntry, SCHEMA_VERSION, VirtualPath, WalletState,
};
use crate::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};

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
    route_index: BTreeMap<String, RouteIndexEntry>,
}

impl GlobalFs {
    pub fn empty() -> Self {
        Self {
            root: FsEntry::Directory {
                children: Default::default(),
                meta: directory_metadata(""),
            },
            mount_points: BTreeSet::new(),
            pending_text: BTreeMap::new(),
            route_index: BTreeMap::new(),
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

    pub fn mount_scanned_subtree(
        &mut self,
        mount_at: VirtualPath,
        snapshot: &ScannedSubtree,
    ) -> Result<(), MountError> {
        self.mount_subtree(mount_at, scanned_subtree_root(snapshot))
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
            FsEntry::File { meta, .. } => match meta.access() {
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

    pub fn upsert_file(
        &mut self,
        path: VirtualPath,
        content: String,
        meta: NodeMetadata,
        extensions: EntryExtensions,
    ) {
        self.pending_text.insert(path.clone(), content);
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::content_file_with_meta("", meta, extensions),
        );
    }

    pub fn upsert_binary_placeholder(
        &mut self,
        path: VirtualPath,
        meta: NodeMetadata,
        extensions: EntryExtensions,
    ) {
        self.pending_text.remove(&path);
        insert_tree_entry(
            &mut self.root,
            &path,
            FsEntry::content_file_with_meta("", meta, extensions),
        );
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
        let Some(FsEntry::File {
            meta: existing_meta,
            extensions: existing_ext,
            ..
        }) = get_tree_entry_mut(&mut self.root, path)
        else {
            return;
        };
        if let Some(new_meta) = meta {
            *existing_meta = new_meta;
        }
        if let Some(new_ext) = extensions {
            *existing_ext = new_ext;
        }
        self.pending_text.insert(path.clone(), content);
    }

    pub fn upsert_directory(&mut self, path: VirtualPath, meta: NodeMetadata) {
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
        remove_tree_entry(&mut self.root, path);
    }

    pub fn remove_subtree(&mut self, path: &VirtualPath) {
        self.pending_text.retain(|k, _| !k.starts_with(path));
        remove_tree_entry(&mut self.root, path);
        self.mount_points.retain(|p| !p.starts_with(path));
    }

    pub fn export_mount_snapshot(&self, mount_root: &VirtualPath) -> Option<ScannedSubtree> {
        let FsEntry::Directory { children, meta } = self.get_entry(mount_root)? else {
            return None;
        };
        let excluded_roots = self.export_excluded_roots(mount_root);

        let mut files = Vec::new();
        collect_scanned_files(mount_root, "", children, &excluded_roots, &mut files);
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let mut directories = Vec::new();
        if has_manifest_metadata("", meta) {
            directories.push(ScannedDirectory {
                path: String::new(),
                meta: meta.clone(),
            });
        }
        collect_scanned_directories(mount_root, "", children, &excluded_roots, &mut directories);
        directories.sort_by(|a, b| a.path.cmp(&b.path));

        Some(ScannedSubtree { files, directories })
    }

    /// Iterate over `(path, &NodeMetadata)` for every node in the tree.
    /// Walks the canonical filesystem so it always reflects the live state.
    pub fn metadata_entries(&self) -> Vec<(VirtualPath, &NodeMetadata)> {
        let mut out = Vec::new();
        collect_metadata_entries(&VirtualPath::root(), &self.root, &mut out);
        out
    }

    fn export_excluded_roots(&self, mount_root: &VirtualPath) -> Vec<VirtualPath> {
        let synthetic_state =
            VirtualPath::from_absolute("/.websh/state").expect("constant synthetic path");
        self.mount_points
            .iter()
            .filter(|path| *path != mount_root && path.starts_with(mount_root))
            .cloned()
            .chain(
                synthetic_state
                    .starts_with(mount_root)
                    .then_some(synthetic_state),
            )
            .collect()
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
        build_render_intent(resolution)
    }
}

fn synthetic_directory(name: &str) -> FsEntry {
    FsEntry::Directory {
        children: Default::default(),
        meta: directory_metadata(name),
    }
}

/// Build a `NodeMetadata` describing a directory whose only authored
/// information is its display title.
fn directory_metadata(name: &str) -> NodeMetadata {
    let title = if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    };
    NodeMetadata {
        schema: SCHEMA_VERSION,
        kind: NodeKind::Directory,
        authored: Fields {
            title,
            ..Fields::default()
        },
        derived: Fields::default(),
    }
}

fn collect_metadata_entries<'a>(
    base: &VirtualPath,
    entry: &'a FsEntry,
    out: &mut Vec<(VirtualPath, &'a NodeMetadata)>,
) {
    out.push((base.clone(), entry.meta()));
    if let FsEntry::Directory { children, .. } = entry {
        for (name, child) in children {
            collect_metadata_entries(&base.join(name), child, out);
        }
    }
}

fn scanned_subtree_root(snapshot: &ScannedSubtree) -> FsEntry {
    let dir_meta_map: HashMap<String, &ScannedDirectory> = snapshot
        .directories
        .iter()
        .map(|dir| (dir.path.clone(), dir))
        .collect();

    let mut children = HashMap::new();

    for file in &snapshot.files {
        insert_scanned_file(&mut children, file, &dir_meta_map);
    }

    for dir in &snapshot.directories {
        if !dir.path.is_empty() {
            ensure_scanned_directory(&mut children, &dir.path, &dir_meta_map);
        }
    }

    let root_meta = dir_meta_map
        .get("")
        .map(|dir| dir.meta.clone())
        .unwrap_or_default();

    FsEntry::Directory {
        children,
        meta: root_meta,
    }
}

fn insert_scanned_file(
    tree: &mut HashMap<String, FsEntry>,
    file: &ScannedFile,
    dir_meta_map: &HashMap<String, &ScannedDirectory>,
) {
    let parts: Vec<&str> = file
        .path
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return;
    }

    let mut current = tree;
    let mut current_path = String::new();
    for (idx, part) in parts.iter().enumerate() {
        let is_last = idx == parts.len() - 1;
        if is_last {
            current.insert(
                (*part).to_string(),
                FsEntry::content_file_with_meta(
                    &file.path,
                    file.meta.clone(),
                    file.extensions.clone(),
                ),
            );
            return;
        }

        if !current_path.is_empty() {
            current_path.push('/');
        }
        current_path.push_str(part);

        let slot = current
            .entry((*part).to_string())
            .or_insert_with(|| scanned_directory_entry(&current_path, part, dir_meta_map));

        current = match slot {
            FsEntry::Directory { children, .. } => children,
            FsEntry::File { .. } => return,
        };
    }
}

fn ensure_scanned_directory(
    tree: &mut HashMap<String, FsEntry>,
    path: &str,
    dir_meta_map: &HashMap<String, &ScannedDirectory>,
) {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let mut current = tree;
    let mut current_path = String::new();

    for part in parts {
        if !current_path.is_empty() {
            current_path.push('/');
        }
        current_path.push_str(part);

        let slot = current
            .entry(part.to_string())
            .or_insert_with(|| scanned_directory_entry(&current_path, part, dir_meta_map));

        current = match slot {
            FsEntry::Directory { children, .. } => children,
            FsEntry::File { .. } => return,
        };
    }
}

fn scanned_directory_entry(
    path: &str,
    name: &str,
    dir_meta_map: &HashMap<String, &ScannedDirectory>,
) -> FsEntry {
    FsEntry::Directory {
        children: HashMap::new(),
        meta: dir_meta_map
            .get(path)
            .map(|dir| dir.meta.clone())
            .unwrap_or_else(|| directory_metadata(name)),
    }
}

fn collect_scanned_files(
    mount_root: &VirtualPath,
    prefix: &str,
    children: &HashMap<String, FsEntry>,
    excluded_roots: &[VirtualPath],
    out: &mut Vec<ScannedFile>,
) {
    let mut names: Vec<&String> = children.keys().collect();
    names.sort();
    for name in names {
        let entry = &children[name];
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        let abs = mount_root.join(&rel);
        if path_is_excluded(&abs, excluded_roots) {
            continue;
        }
        match entry {
            FsEntry::File {
                content_path,
                meta,
                extensions,
            } => {
                if content_path.is_none() {
                    continue;
                }
                out.push(ScannedFile {
                    path: content_path
                        .as_ref()
                        .filter(|path| !path.is_empty())
                        .cloned()
                        .unwrap_or(rel),
                    meta: meta.clone(),
                    extensions: extensions.clone(),
                });
            }
            FsEntry::Directory { children, .. } => {
                collect_scanned_files(mount_root, &rel, children, excluded_roots, out);
            }
        }
    }
}

fn collect_scanned_directories(
    mount_root: &VirtualPath,
    prefix: &str,
    children: &HashMap<String, FsEntry>,
    excluded_roots: &[VirtualPath],
    out: &mut Vec<ScannedDirectory>,
) {
    let mut names: Vec<&String> = children.keys().collect();
    names.sort();
    for name in names {
        let entry = &children[name];
        if let FsEntry::Directory {
            children: sub,
            meta,
        } = entry
        {
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{}/{}", prefix, name)
            };
            let abs = mount_root.join(&rel);
            if path_is_excluded(&abs, excluded_roots) {
                continue;
            }
            if exportable_children_empty(mount_root, &rel, sub, excluded_roots)
                || has_manifest_metadata(&rel, meta)
            {
                out.push(ScannedDirectory {
                    path: rel.clone(),
                    meta: meta.clone(),
                });
            }
            collect_scanned_directories(mount_root, &rel, sub, excluded_roots, out);
        }
    }
}

fn exportable_children_empty(
    mount_root: &VirtualPath,
    prefix: &str,
    children: &HashMap<String, FsEntry>,
    excluded_roots: &[VirtualPath],
) -> bool {
    !children.iter().any(|(name, _)| {
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        !path_is_excluded(&mount_root.join(&rel), excluded_roots)
    })
}

fn path_is_excluded(path: &VirtualPath, excluded_roots: &[VirtualPath]) -> bool {
    excluded_roots
        .iter()
        .any(|excluded| path.starts_with(excluded))
}

fn has_manifest_metadata(path: &str, meta: &NodeMetadata) -> bool {
    if meta.description().is_some()
        || meta.icon().is_some()
        || meta.thumbnail().is_some()
        || meta.tags().map(|t| !t.is_empty()).unwrap_or(false)
    {
        return true;
    }
    let title = meta.title().unwrap_or("");
    if path.is_empty() {
        return !title.is_empty();
    }
    let last_segment = path.rsplit('/').next().unwrap_or("");
    !title.is_empty() && title != last_segment
}

fn sorted_dir_entries(base: &VirtualPath, children: &HashMap<String, FsEntry>) -> Vec<DirEntry> {
    let mut items: Vec<_> = children
        .iter()
        .map(|(name, entry)| {
            let is_dir = entry.is_directory();
            let title = match entry {
                FsEntry::Directory { meta, .. } => {
                    meta.title().unwrap_or(name.as_str()).to_string()
                }
                FsEntry::File { meta, .. } => meta.title().unwrap_or(name.as_str()).to_string(),
            };
            DirEntry {
                name: name.clone(),
                path: base.join(name),
                is_dir,
                title,
                meta: Some(entry.meta().clone()),
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
    use crate::domain::{Fields, NodeKind, NodeMetadata, SCHEMA_VERSION};
    use crate::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};

    use super::*;

    fn file_meta(kind: NodeKind) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }

    fn dir_meta(name: &str) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Directory,
            authored: Fields {
                title: if name.is_empty() {
                    None
                } else {
                    Some(name.to_string())
                },
                ..Fields::default()
            },
            derived: Fields::default(),
        }
    }

    fn snapshot(files: &[&str], directories: &[&str]) -> ScannedSubtree {
        ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    meta: file_meta(NodeKind::Asset),
                    extensions: EntryExtensions::default(),
                })
                .collect(),
            directories: directories
                .iter()
                .map(|path| ScannedDirectory {
                    path: (*path).to_string(),
                    meta: dir_meta(path.rsplit('/').next().unwrap_or(path)),
                })
                .collect(),
        }
    }

    #[test]
    fn mounts_scanned_subtrees_under_canonical_prefixes() {
        let mut global = GlobalFs::empty();
        let site = snapshot(&["index.html", "about.md"], &["blog"]);
        let db = snapshot(&["notes/todo.md"], &["notes"]);

        global
            .mount_scanned_subtree(VirtualPath::root(), &site)
            .unwrap();
        global
            .mount_scanned_subtree(VirtualPath::from_absolute("/db").unwrap(), &db)
            .unwrap();

        assert!(
            global
                .get_entry(&VirtualPath::from_absolute("/index.html").unwrap())
                .is_some()
        );
        assert!(
            global
                .get_entry(&VirtualPath::from_absolute("/db/notes/todo.md").unwrap())
                .is_some()
        );
    }

    #[test]
    fn refuses_to_replace_existing_directory_mountpoint() {
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(
                VirtualPath::from_absolute("/db").unwrap(),
                &snapshot(&["index.md"], &[]),
            )
            .unwrap();

        let err = global
            .mount_scanned_subtree(
                VirtualPath::from_absolute("/db").unwrap(),
                &snapshot(&["other.md"], &[]),
            )
            .unwrap_err();

        assert_eq!(
            err,
            MountError::MountPointOccupied {
                path: VirtualPath::from_absolute("/db").unwrap()
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
                    meta: dir_meta(""),
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
            .mount_scanned_subtree(
                VirtualPath::root(),
                &snapshot(&["blog/hello.md"], &["blog"]),
            )
            .unwrap();

        let entries = global
            .list_dir(&VirtualPath::from_absolute("/blog").unwrap())
            .unwrap();

        assert_eq!(entries[0].path.as_str(), "/blog/hello.md");
    }

    #[test]
    fn pending_text_tracks_upserts() {
        let mut global = GlobalFs::empty();
        let path = VirtualPath::from_absolute("/new.md").unwrap();
        global.upsert_file(
            path.clone(),
            "hello".to_string(),
            file_meta(NodeKind::Page),
            EntryExtensions::default(),
        );

        assert_eq!(global.read_pending_text(&path).as_deref(), Some("hello"));
    }

    #[test]
    fn scanned_subtree_roundtrip_is_byte_stable() {
        let golden = include_str!("../../../../tests/fixtures/manifest_golden.json");
        let snapshot =
            crate::storage::github::manifest::parse_snapshot(golden).expect("golden parses");

        let mut global = GlobalFs::empty();
        let root = VirtualPath::root();
        global
            .mount_scanned_subtree(root.clone(), &snapshot)
            .unwrap();
        let reserialized = global.export_mount_snapshot(&root).unwrap();
        let out =
            crate::storage::github::manifest::serialize_snapshot(&reserialized).expect("serialize");

        assert_eq!(out.trim_end(), golden.trim_end());
    }

    #[test]
    fn exported_mount_snapshot_sorts_regardless_of_input_order() {
        let tagged_dir = |title: &str, tag: &str| NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Directory,
            authored: Fields {
                title: Some(title.to_string()),
                tags: Some(vec![tag.to_string()]),
                ..Fields::default()
            },
            derived: Fields::default(),
        };

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "z.md".to_string(),
                    meta: file_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "m.md".to_string(),
                    meta: file_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "a.md".to_string(),
                    meta: file_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: "z-dir".to_string(),
                    meta: tagged_dir("Z", "zone"),
                },
                ScannedDirectory {
                    path: "a-dir".to_string(),
                    meta: tagged_dir("A", "area"),
                },
            ],
        };

        let mut global = GlobalFs::empty();
        let root = VirtualPath::root();
        global
            .mount_scanned_subtree(root.clone(), &snapshot)
            .unwrap();
        let out = global.export_mount_snapshot(&root).unwrap();
        let file_paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(file_paths, vec!["a.md", "m.md", "z.md"]);
        let dir_paths: Vec<&str> = out.directories.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(dir_paths, vec!["a-dir", "z-dir"]);
    }

    #[test]
    fn exported_mount_snapshot_uses_relative_paths_for_pending_files() {
        let mut global = GlobalFs::empty();
        let root = VirtualPath::root();
        global
            .mount_scanned_subtree(root.clone(), &ScannedSubtree::default())
            .unwrap();
        global.upsert_file(
            root.join("notes.md"),
            "notes".into(),
            file_meta(NodeKind::Page),
            EntryExtensions::default(),
        );

        let snapshot = global.export_mount_snapshot(&root).unwrap();
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.files[0].path, "notes.md");
    }

    #[test]
    fn exported_mount_snapshot_preserves_empty_directories() {
        let mut global = GlobalFs::empty();
        let root = VirtualPath::root();
        global
            .mount_scanned_subtree(root.clone(), &ScannedSubtree::default())
            .unwrap();
        global.upsert_directory(root.join("empty"), dir_meta("empty"));

        let snapshot = global.export_mount_snapshot(&root).unwrap();
        let paths: Vec<_> = snapshot
            .directories
            .iter()
            .map(|dir| dir.path.as_str())
            .collect();
        assert_eq!(paths, vec!["empty"]);
    }

    #[test]
    fn root_export_excludes_descendant_mounts_and_runtime_state() {
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(
                VirtualPath::root(),
                &snapshot(
                    &[
                        "index.md",
                        ".websh/site.json",
                        ".websh/mounts/db.mount.json",
                    ],
                    &[".websh", ".websh/mounts"],
                ),
            )
            .unwrap();
        global
            .mount_scanned_subtree(
                VirtualPath::from_absolute("/db").unwrap(),
                &snapshot(&["fresh.md"], &[]),
            )
            .unwrap();
        global.upsert_directory(
            VirtualPath::from_absolute("/.websh/state").unwrap(),
            dir_meta("state"),
        );
        global.upsert_file(
            VirtualPath::from_absolute("/.websh/state/session/wallet_session").unwrap(),
            "1".into(),
            file_meta(NodeKind::Data),
            EntryExtensions::default(),
        );

        let snapshot = global.export_mount_snapshot(&VirtualPath::root()).unwrap();
        let files: Vec<_> = snapshot
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();

        assert!(files.contains(&"index.md"));
        assert!(files.contains(&".websh/site.json"));
        assert!(files.contains(&".websh/mounts/db.mount.json"));
        assert!(!files.iter().any(|path| path.starts_with("db/")));
        assert!(!files.iter().any(|path| path.starts_with(".websh/state/")));
    }

    #[test]
    fn descendant_mount_export_includes_only_mount_relative_files() {
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot(&["index.md"], &[]))
            .unwrap();
        let db_root = VirtualPath::from_absolute("/db").unwrap();
        global
            .mount_scanned_subtree(db_root.clone(), &snapshot(&["fresh.md"], &[]))
            .unwrap();

        let snapshot = global.export_mount_snapshot(&db_root).unwrap();
        let files: Vec<_> = snapshot
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();

        assert_eq!(files, vec!["fresh.md"]);
    }
}
