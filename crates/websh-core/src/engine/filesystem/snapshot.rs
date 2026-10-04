use std::collections::HashMap;

use crate::domain::FsEntry;
use crate::ports::{ScannedDirectory, ScannedFile, ScannedSubtree};

use super::tree::directory_metadata;
use super::{
    GlobalFs, RouteCatalog, RouteCatalogError, RouteRequest, RouteResolution,
    resolve_route_with_catalog,
};

/// An immutable content tree and the route catalog validated against that tree.
#[derive(Clone, Debug)]
pub struct Snapshot {
    fs: GlobalFs,
    routes: RouteCatalog,
}

impl Snapshot {
    pub fn new(fs: GlobalFs) -> Result<Self, RouteCatalogError> {
        let routes = RouteCatalog::from_global_fs(&fs)?;
        Ok(Self { fs, routes })
    }

    pub fn fs(&self) -> &GlobalFs {
        &self.fs
    }

    pub fn routes(&self) -> &RouteCatalog {
        &self.routes
    }

    pub fn resolve(&self, request: &RouteRequest) -> Option<RouteResolution> {
        resolve_route_with_catalog(&self.fs, &self.routes, request)
    }
}

pub(super) fn scanned_subtree_root(snapshot: &ScannedSubtree) -> FsEntry {
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
        .unwrap_or_else(|| directory_metadata(""));

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
                FsEntry::file_with_meta(file.meta.clone(), file.extensions.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{EntryExtensions, NodeKind, NodeMetadata, VirtualPath};

    fn add_page(fs: &mut GlobalFs, path: &str) {
        fs.upsert_file(
            VirtualPath::from_absolute(path).unwrap(),
            String::new(),
            NodeMetadata {
                kind: NodeKind::Page,
                ..NodeMetadata::default()
            },
            EntryExtensions::default(),
        );
    }

    #[test]
    fn snapshot_resolves_its_validated_content_and_shell_routes() {
        let mut fs = GlobalFs::empty();
        add_page(&mut fs, "/article.md");
        let snapshot = Snapshot::new(fs).unwrap();
        let route = snapshot.resolve(&RouteRequest::new("/article")).unwrap();
        assert_eq!(route.node_path.as_str(), "/article.md");
        assert!(snapshot.fs().exists(&route.node_path));
        assert!(snapshot.routes().resolve("/article").is_some());
        assert!(snapshot.resolve(&RouteRequest::new("/websh")).is_some());
    }

    #[test]
    fn conflicting_routes_cannot_be_published_as_a_snapshot() {
        let mut fs = GlobalFs::empty();
        add_page(&mut fs, "/article.md");
        add_page(&mut fs, "/article.html");
        assert!(matches!(
            Snapshot::new(fs),
            Err(RouteCatalogError::RouteCollision { .. })
        ));
    }
}
