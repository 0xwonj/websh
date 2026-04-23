use crate::core::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};
use crate::models::{
    DirectoryMetadata, DisplayPermissions, FileMetadata, FsEntry, VirtualPath, WalletState,
};
use std::collections::HashMap;

/// Directory entry returned by list_dir
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub path: VirtualPath,
    pub is_dir: bool,
    pub title: String,
    pub file_meta: Option<FileMetadata>,
}

/// Virtual filesystem for a single mount.
///
/// Stores files using relative paths from the mount root.
/// For example, a file at URL `~/blog/post.md` is stored as `blog/post.md`.
///
/// # Path Convention
///
/// - Root of mount: empty string `""`
/// - File in root: `"post.md"`
/// - Nested file: `"blog/post.md"`
/// - No leading or trailing slashes
#[derive(Clone)]
pub struct VirtualFs {
    /// Root directory entry containing all files
    root: FsEntry,
    /// Locally-staged content for files with pending writes. Keyed by the
    /// relative path used throughout VirtualFs (no leading slash). Populated
    /// by `upsert_file` / `update_file_content`; cleared by `remove_entry` /
    /// `remove_subtree`. Not persisted — exists only during a local session.
    pending_content: HashMap<String, String>,
}

impl VirtualFs {
    /// Clone the root entry for subtree assembly in the new global engine.
    pub(crate) fn clone_root_entry(&self) -> FsEntry {
        self.root.clone()
    }

    /// Create a mounted subtree snapshot from backend scan rows.
    ///
    /// Scan paths are relative (e.g., `blog/post.md`).
    pub(crate) fn from_scanned_subtree(snapshot: &ScannedSubtree) -> Self {
        // Build directory metadata map for quick lookup
        let dir_meta_map: HashMap<String, &ScannedDirectory> = snapshot
            .directories
            .iter()
            .map(|d| (d.path.clone(), d))
            .collect();

        let mut content_tree: HashMap<String, FsEntry> = HashMap::new();

        // Create all files (this also creates parent directories)
        for file in &snapshot.files {
            Self::insert_path(
                &mut content_tree,
                &file.path,
                &file.path,
                &file.description,
                file.meta.clone(),
                &dir_meta_map,
            );
        }

        // Ensure directories from the scan exist (even if empty)
        for dir in &snapshot.directories {
            if !dir.path.is_empty() {
                Self::ensure_directory(&mut content_tree, &dir.path, &dir_meta_map);
            }
        }

        // Build root metadata
        let root_meta = dir_meta_map
            .get("")
            .map(|d| DirectoryMetadata {
                title: d.meta.title.clone(),
                description: d.meta.description.clone(),
                icon: d.meta.icon.clone(),
                thumbnail: d.meta.thumbnail.clone(),
                tags: d.meta.tags.clone(),
            })
            .unwrap_or_default();

        let root = FsEntry::Directory {
            children: content_tree,
            meta: root_meta,
        };

        Self {
            root,
            pending_content: HashMap::new(),
        }
    }

    /// Insert a path into the tree using iteration instead of recursion.
    fn insert_path(
        tree: &mut HashMap<String, FsEntry>,
        path: &str,
        full_path: &str,
        title: &str,
        meta: FileMetadata,
        dir_meta_map: &HashMap<String, &ScannedDirectory>,
    ) {
        let parts: Vec<&str> = path.split('/').collect();
        let mut current = tree;
        let mut current_path = String::new();

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;

            if is_last {
                current.insert(
                    part.to_string(),
                    FsEntry::content_file_with_meta(full_path, title, meta.clone()),
                );
            } else {
                // Build current directory path
                if !current_path.is_empty() {
                    current_path.push('/');
                }
                current_path.push_str(part);

                let entry = current.entry(part.to_string()).or_insert_with(|| {
                    // Create directory with metadata if available
                    let dir_meta = dir_meta_map
                        .get(&current_path)
                        .map(|d| DirectoryMetadata {
                            title: d.meta.title.clone(),
                            description: d.meta.description.clone(),
                            icon: d.meta.icon.clone(),
                            thumbnail: d.meta.thumbnail.clone(),
                            tags: d.meta.tags.clone(),
                        })
                        .unwrap_or_else(|| DirectoryMetadata {
                            title: part.to_string(),
                            ..Default::default()
                        });

                    FsEntry::Directory {
                        children: HashMap::new(),
                        meta: dir_meta,
                    }
                });

                current = match entry {
                    FsEntry::Directory { children, .. } => children,
                    FsEntry::File { .. } => {
                        // A file exists where we expect a directory - skip this entry.
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::warn_1(
                            &format!(
                                "Scanned subtree conflict: '{}' blocked by existing file",
                                full_path
                            )
                            .into(),
                        );
                        return;
                    }
                };
            }
        }
    }

    /// Ensure a directory exists at the given path.
    fn ensure_directory(
        tree: &mut HashMap<String, FsEntry>,
        path: &str,
        dir_meta_map: &HashMap<String, &ScannedDirectory>,
    ) {
        let parts: Vec<&str> = path.split('/').collect();
        let mut current = tree;
        let mut current_path = String::new();

        for part in parts {
            if !current_path.is_empty() {
                current_path.push('/');
            }
            current_path.push_str(part);

            let entry = current.entry(part.to_string()).or_insert_with(|| {
                let dir_meta = dir_meta_map
                    .get(&current_path)
                    .map(|d| DirectoryMetadata {
                        title: d.meta.title.clone(),
                        description: d.meta.description.clone(),
                        icon: d.meta.icon.clone(),
                        thumbnail: d.meta.thumbnail.clone(),
                        tags: d.meta.tags.clone(),
                    })
                    .unwrap_or_else(|| DirectoryMetadata {
                        title: part.to_string(),
                        ..Default::default()
                    });

                FsEntry::Directory {
                    children: HashMap::new(),
                    meta: dir_meta,
                }
            });

            current = match entry {
                FsEntry::Directory { children, .. } => children,
                FsEntry::File { .. } => return,
            };
        }
    }

    /// Create empty filesystem (fallback when manifest fails to load).
    pub fn empty() -> Self {
        let root = FsEntry::Directory {
            children: HashMap::new(),
            meta: DirectoryMetadata::default(),
        };

        Self {
            root,
            pending_content: HashMap::new(),
        }
    }

    /// Resolve a path relative to current directory.
    ///
    /// # Arguments
    /// - `current`: Current path (relative, e.g., `"blog"` or `""` for root)
    /// - `path`: Path to resolve (can be relative like `"posts"` or `".."`)
    ///
    /// # Returns
    /// The resolved relative path if the target exists, or `None`.
    ///
    /// # Path Convention
    /// - Root: `""`
    /// - Subdirectory: `"blog"`, `"blog/posts"`
    /// - `~` and `~/...` are treated as root-relative
    pub fn resolve_path(&self, current: &str, path: &str) -> Option<String> {
        let resolved = Self::resolve_path_string(current, path);

        // Verify path exists
        if self.get_entry(&resolved).is_some() {
            Some(resolved)
        } else {
            None
        }
    }

    /// Resolve a path string without filesystem validation.
    ///
    /// All paths are relative (no leading slash).
    /// - `~` means root (empty string)
    /// - `..` goes up one level
    /// - `.` stays in current directory
    pub fn resolve_path_string(current: &str, path: &str) -> String {
        // Handle home directory
        if path == "~" {
            return String::new();
        }
        if let Some(rest) = path.strip_prefix("~/") {
            return Self::normalize_path(rest);
        }

        // Handle parent directory
        if path == ".." {
            return Self::parent_path(current);
        }

        // Handle current directory
        if path == "." || path.is_empty() {
            return current.to_string();
        }

        // Handle relative path
        let combined = if current.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", current, path)
        };

        Self::normalize_path(&combined)
    }

    /// Get the parent directory of a path.
    ///
    /// Returns empty string for root or single-level paths.
    pub fn parent_path(path: &str) -> String {
        if path.is_empty() {
            return String::new();
        }

        match path.rsplit_once('/') {
            Some((parent, _)) => parent.to_string(),
            None => String::new(), // Single segment, parent is root
        }
    }

    /// Normalize a path by resolving `.` and `..` components.
    ///
    /// Returns a relative path (no leading or trailing slashes).
    pub fn normalize_path(path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/').filter(|s| !s.is_empty()) {
            match part {
                ".." => {
                    parts.pop();
                }
                "." => {}
                _ => parts.push(part),
            }
        }

        parts.join("/")
    }

    /// Get an entry by relative path.
    ///
    /// - Empty string `""` returns the root directory
    /// - `"blog"` returns the blog directory
    /// - `"blog/post.md"` returns the file
    pub fn get_entry(&self, path: &str) -> Option<&FsEntry> {
        if path.is_empty() {
            return Some(&self.root);
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let mut current = &self.root;

        for part in parts {
            match current {
                FsEntry::Directory { children, .. } => {
                    current = children.get(part)?;
                }
                FsEntry::File { .. } => return None,
            }
        }

        Some(current)
    }

    /// Get an entry by absolute [`VirtualPath`].
    pub fn get(&self, path: &VirtualPath) -> Option<&FsEntry> {
        self.get_entry(path.as_str().trim_start_matches('/'))
    }

    /// Read the locally-staged text content for a file with a pending write,
    /// if any. Returns `None` for files that have no staged content (including
    /// canonical manifest files whose content lives in remote storage).
    pub fn read_file(&self, path: &VirtualPath) -> Option<String> {
        let rel = path.as_str().trim_start_matches('/');
        self.pending_content.get(rel).cloned()
    }

    /// Create or replace a text file at `path`, recording the staged content
    /// in `pending_content`.
    pub fn upsert_file(&mut self, path: VirtualPath, content: String, meta: FileMetadata) {
        let rel = path.as_str().trim_start_matches('/').to_string();
        self.pending_content.insert(rel.clone(), content);
        insert_tree_entry(
            &mut self.root,
            &rel,
            FsEntry::content_file_with_meta(&rel, "", meta),
        );
    }

    /// Create or replace a binary file placeholder at `path`. The blob is
    /// carried out-of-band (in the ChangeSet); no content is stored in
    /// `pending_content`.
    pub fn upsert_binary_placeholder(&mut self, path: VirtualPath, meta: FileMetadata) {
        let rel = path.as_str().trim_start_matches('/').to_string();
        insert_tree_entry(
            &mut self.root,
            &rel,
            FsEntry::content_file_with_meta(&rel, "", meta),
        );
    }

    /// Update the staged content (and optionally description) of an existing
    /// file. No-op if `path` does not resolve to a file entry.
    pub fn update_file_content(
        &mut self,
        path: &VirtualPath,
        content: String,
        description: Option<String>,
    ) {
        let rel = path.as_str().trim_start_matches('/').to_string();
        let Some(FsEntry::File { description: d, .. }) = get_tree_entry_mut(&mut self.root, &rel)
        else {
            return;
        };
        if let Some(new_d) = description {
            *d = new_d;
        }
        self.pending_content.insert(rel, content);
    }

    /// Create or replace a directory at `path` with the given metadata.
    pub fn upsert_directory(&mut self, path: VirtualPath, meta: DirectoryMetadata) {
        let rel = path.as_str().trim_start_matches('/').to_string();
        insert_tree_entry(
            &mut self.root,
            &rel,
            FsEntry::Directory {
                children: HashMap::new(),
                meta,
            },
        );
    }

    /// Remove a single entry at `path` and its staged content (if any).
    pub fn remove_entry(&mut self, path: &VirtualPath) {
        let rel = path.as_str().trim_start_matches('/').to_string();
        self.pending_content.remove(&rel);
        remove_tree_entry(&mut self.root, &rel);
    }

    /// Remove a subtree rooted at `path`, including all descendants' staged
    /// content entries.
    pub fn remove_subtree(&mut self, path: &VirtualPath) {
        let rel = path.as_str().trim_start_matches('/').to_string();
        let prefix = if rel.is_empty() {
            String::new()
        } else {
            format!("{}/", rel)
        };
        self.pending_content
            .retain(|k, _| k != &rel && !k.starts_with(&prefix));
        remove_tree_entry(&mut self.root, &rel);
    }

    /// List directory contents with metadata.
    ///
    /// # Arguments
    /// - `path`: Relative path to directory (empty string for root)
    ///
    /// # Returns
    /// Sorted list of entries (directories first, then files, hidden last).
    pub fn list_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        let base = if path.is_empty() {
            VirtualPath::root()
        } else {
            VirtualPath::from_absolute(format!("/{}", path)).ok()?
        };

        match self.get_entry(path)? {
            FsEntry::Directory { children, .. } => {
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
                // Sort: directories first, then regular files, then hidden files
                // Within each group, sort alphabetically
                items.sort_by(|a, b| {
                    let a_hidden = a.name.starts_with('.');
                    let b_hidden = b.name.starts_with('.');

                    match (a.is_dir, b.is_dir, a_hidden, b_hidden) {
                        // Directories before files
                        (true, false, _, _) => std::cmp::Ordering::Less,
                        (false, true, _, _) => std::cmp::Ordering::Greater,
                        // Hidden files last (within same type)
                        (_, _, false, true) => std::cmp::Ordering::Less,
                        (_, _, true, false) => std::cmp::Ordering::Greater,
                        // Same category: alphabetical
                        _ => a.name.cmp(&b.name),
                    }
                });
                Some(items)
            }
            FsEntry::File { .. } => None,
        }
    }

    /// Get the content path for a file (for fetching from remote).
    ///
    /// Returns the path as stored in the manifest (relative).
    pub fn get_file_content_path(&self, path: &str) -> Option<String> {
        match self.get_entry(path)? {
            FsEntry::File { content_path, .. } => content_path.clone(),
            _ => None,
        }
    }

    /// Check if a path is a directory.
    pub fn is_directory(&self, path: &str) -> bool {
        matches!(self.get_entry(path), Some(FsEntry::Directory { .. }))
    }

    /// Check if a directory contains any children.
    ///
    /// Returns `false` if `path` does not exist or is not a directory, and
    /// `true` only if it is a non-empty directory.
    pub fn has_children(&self, path: &str) -> bool {
        match self.get_entry(path) {
            Some(FsEntry::Directory { children, .. }) => !children.is_empty(),
            _ => false,
        }
    }

    /// Compute display permissions for an entry at runtime.
    ///
    /// Permissions are computed based on:
    /// - `d`: Directory or file
    /// - `r`: Access-restricted files require wallet address in the recipients list
    /// - `w`: Admin login (not yet implemented, always false for now)
    /// - `x`: Directories only
    pub fn get_permissions(&self, entry: &FsEntry, wallet: &WalletState) -> DisplayPermissions {
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

        let write = false;
        let execute = is_dir;

        DisplayPermissions {
            is_dir,
            read,
            write,
            execute,
        }
    }
}

impl VirtualFs {
    /// Iterate all *content-backed* files in the VFS in depth-first order.
    ///
    /// Yields `(absolute_virtual_path, entry)` pairs. Synthetic files (those
    /// with `content_path: None`, e.g. `.profile`) are filtered out — they
    /// have no manifest origin and would corrupt a round-trip.
    pub fn iter_files(&self) -> Vec<(VirtualPath, &FsEntry)> {
        let mut out = Vec::new();
        if let FsEntry::Directory { children, .. } = &self.root {
            collect_files("", children, &mut out);
        }
        out
    }

    /// Iterate all directories in the VFS, starting with the root (path `""`).
    ///
    /// Yields `(relative_path, metadata)` pairs. The root directory is
    /// included as `("", meta)`.
    pub fn iter_directories(&self) -> Vec<(String, &DirectoryMetadata)> {
        let mut out = Vec::new();
        if let FsEntry::Directory { children, meta } = &self.root {
            out.push((String::new(), meta));
            collect_directories("", children, &mut out);
        }
        out
    }

    /// Re-serialize the current subtree state into backend-neutral scan rows.
    ///
    /// **Byte-stable** (see spec §4.2): the same logical state must produce
    /// identical bytes across sessions/machines/rust versions. We guarantee
    /// this by:
    /// - collecting files in a deterministic walk and sorting the result
    ///   lexicographically by `path`;
    /// - collecting directories and sorting the same way;
    /// - filtering out synthetic files (no `content_path`) and "implicit"
    ///   directories (no manifest-origin metadata);
    /// - leaving Option fields (`size`, `modified`, `access`, ...) for serde
    ///   to emit as `null` consistently — no `skip_serializing_if`.
    pub(crate) fn to_scanned_subtree(&self) -> ScannedSubtree {
        let mut files: Vec<ScannedFile> = self
            .iter_files()
            .into_iter()
            .map(|(path, entry)| match entry {
                FsEntry::File {
                    content_path,
                    description,
                    meta,
                } => ScannedFile {
                    path: content_path
                        .clone()
                        .unwrap_or_else(|| path.as_str().trim_start_matches('/').to_string()),
                    description: description.clone(),
                    meta: meta.clone(),
                },
                FsEntry::Directory { .. } => unreachable!("iter_files only yields files"),
            })
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        let mut directories: Vec<ScannedDirectory> = self
            .iter_directories()
            .into_iter()
            .filter(|(path, meta)| has_manifest_metadata(path, meta))
            .map(|(path, meta)| ScannedDirectory {
                path,
                meta: meta.clone(),
            })
            .collect();
        directories.sort_by(|a, b| a.path.cmp(&b.path));

        ScannedSubtree { files, directories }
    }
}

impl Default for VirtualFs {
    fn default() -> Self {
        Self::empty()
    }
}

/// Recursive helper: append every content-backed file under `children` to `out`.
/// `prefix` is the parent directory's relative path (empty at root); we build
/// `{prefix}/{name}` for each entry and normalize through
/// `VirtualPath::from_absolute(&format!("/{rel}"))`.
fn collect_files<'a>(
    prefix: &str,
    children: &'a HashMap<String, FsEntry>,
    out: &mut Vec<(VirtualPath, &'a FsEntry)>,
) {
    // Sort children by name so the output order is stable at every level.
    let mut names: Vec<&String> = children.keys().collect();
    names.sort();
    for name in names {
        let entry = &children[name];
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        match entry {
            FsEntry::File { content_path, .. } => {
                // Filter VFS-synthetic files (no manifest origin).
                if content_path.is_none() {
                    continue;
                }
                let vp = VirtualPath::from_absolute(format!("/{}", rel))
                    .expect("non-empty relative path produces absolute");
                out.push((vp, entry));
            }
            FsEntry::Directory { children, .. } => {
                collect_files(&rel, children, out);
            }
        }
    }
}

fn collect_directories<'a>(
    prefix: &str,
    children: &'a HashMap<String, FsEntry>,
    out: &mut Vec<(String, &'a DirectoryMetadata)>,
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
            out.push((rel.clone(), meta));
            collect_directories(&rel, sub, out);
        }
    }
}

/// Heuristic: does this directory's metadata look like it came from an
/// explicit manifest entry (vs. being auto-created from a file path)?
///
/// We emit directories in `to_scanned_subtree` only when this returns `true`,
/// so round-trips don't invent explicit directory rows for every implicit
/// parent directory. Rules:
///
/// - Any non-empty `description`, `icon`, `thumbnail`, or `tags` → manifest.
/// - Non-empty `title` that differs from the last path segment → manifest.
/// - Root (`path == ""`) with any non-default field → manifest.
///
/// Note: a pathological manifest entry that sets only `title` equal to the
/// segment name is indistinguishable from an implicit one; that's a known
/// round-trip edge case documented in the spec.
fn has_manifest_metadata(path: &str, meta: &DirectoryMetadata) -> bool {
    if meta.description.is_some()
        || meta.icon.is_some()
        || meta.thumbnail.is_some()
        || !meta.tags.is_empty()
    {
        return true;
    }
    if path.is_empty() {
        return !meta.title.is_empty();
    }
    let last_segment = path.rsplit('/').next().unwrap_or("");
    !meta.title.is_empty() && meta.title != last_segment
}

/// Walk/create parents, then insert `entry` at the final path segment.
/// If any intermediate segment is a File, overwrite it with a Directory.
/// No-op if `rel_path` is empty (cannot replace root via this helper).
fn insert_tree_entry(root: &mut FsEntry, rel_path: &str, entry: FsEntry) {
    let parts: Vec<&str> = rel_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }
    let mut current = match root {
        FsEntry::Directory { children, .. } => children,
        FsEntry::File { .. } => return,
    };
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        if is_last {
            current.insert((*part).to_string(), entry);
            return;
        }
        let slot = current
            .entry((*part).to_string())
            .or_insert_with(|| FsEntry::Directory {
                children: HashMap::new(),
                meta: DirectoryMetadata {
                    title: (*part).to_string(),
                    ..Default::default()
                },
            });
        if !slot.is_directory() {
            *slot = FsEntry::Directory {
                children: HashMap::new(),
                meta: DirectoryMetadata {
                    title: (*part).to_string(),
                    ..Default::default()
                },
            };
        }
        current = match slot {
            FsEntry::Directory { children, .. } => children,
            FsEntry::File { .. } => unreachable!(),
        };
    }
}

/// Remove the entry at `rel_path`. No-op if any segment is missing or if `rel_path` is empty.
fn remove_tree_entry(root: &mut FsEntry, rel_path: &str) {
    let parts: Vec<&str> = rel_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return;
    }
    let mut current = match root {
        FsEntry::Directory { children, .. } => children,
        FsEntry::File { .. } => return,
    };
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        if is_last {
            current.remove(*part);
            return;
        }
        let next = match current.get_mut(*part) {
            Some(FsEntry::Directory { children, .. }) => children,
            _ => return,
        };
        current = next;
    }
}

/// Get a mutable reference to the entry at `rel_path`. Returns `Some(root)` when `rel_path` is empty.
fn get_tree_entry_mut<'a>(root: &'a mut FsEntry, rel_path: &str) -> Option<&'a mut FsEntry> {
    let parts: Vec<&str> = rel_path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.is_empty() {
        return Some(root);
    }
    let mut current = match root {
        FsEntry::Directory { children, .. } => children,
        FsEntry::File { .. } => return None,
    };
    for (i, part) in parts.iter().enumerate() {
        let is_last = i == parts.len() - 1;
        if is_last {
            return current.get_mut(*part);
        }
        match current.get_mut(*part)? {
            FsEntry::Directory { children, .. } => current = children,
            FsEntry::File { .. } => return None,
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};

    fn create_test_fs() -> VirtualFs {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "blog/hello.md".to_string(),
                    description: "Hello World".to_string(),
                    meta: FileMetadata {
                        size: Some(1234),
                        modified: Some(1704153600),
                        tags: vec!["rust".to_string(), "intro".to_string()],
                        access: None,
                    },
                },
                ScannedFile {
                    path: "blog/rust.md".to_string(),
                    description: "Learning Rust".to_string(),
                    meta: FileMetadata {
                        size: Some(2048),
                        modified: None,
                        tags: vec![],
                        access: None,
                    },
                },
                ScannedFile {
                    path: "projects/web/app.md".to_string(),
                    description: "Web App".to_string(),
                    meta: FileMetadata::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: "blog".to_string(),
                    meta: DirectoryMetadata {
                        title: "Blog Posts".to_string(),
                        tags: vec!["posts".to_string()],
                        ..Default::default()
                    },
                },
                ScannedDirectory {
                    path: String::new(),
                    meta: DirectoryMetadata {
                        title: "Home".to_string(),
                        tags: vec!["root".to_string()],
                        ..Default::default()
                    },
                },
            ],
        };
        VirtualFs::from_scanned_subtree(&snapshot)
    }

    #[test]
    fn test_empty_fs() {
        let fs = VirtualFs::empty();
        // Root is empty string
        assert!(fs.get_entry("").is_some());
        assert!(fs.get_entry(".profile").is_none());
    }

    #[test]
    fn test_from_scanned_subtree() {
        let fs = create_test_fs();

        // Check root exists (empty string)
        assert!(fs.get_entry("").is_some());

        // Check blog directory was created
        assert!(fs.is_directory("blog"));

        // Check files were created
        assert!(fs.get_entry("blog/hello.md").is_some());
        assert!(!fs.is_directory("blog/hello.md"));
    }

    #[test]
    fn test_directory_metadata() {
        let fs = create_test_fs();

        // Check root directory title
        let root_entry = fs.get_entry("").expect("root should exist");
        assert_eq!(root_entry.dir_meta().unwrap().title, "Home");

        // Check directory title was set from the scan rows
        let blog_entry = fs.get_entry("blog").expect("blog should exist");
        assert_eq!(blog_entry.dir_meta().unwrap().title, "Blog Posts");

        // Directory without metadata should use directory name as title
        let projects_entry = fs.get_entry("projects").expect("projects should exist");
        assert_eq!(projects_entry.dir_meta().unwrap().title, "projects");
    }

    #[test]
    fn test_nested_paths() {
        let fs = create_test_fs();

        // Check deeply nested path
        assert!(fs.is_directory("projects"));
        assert!(fs.is_directory("projects/web"));
        assert!(fs.get_entry("projects/web/app.md").is_some());
    }

    #[test]
    fn test_list_dir() {
        let fs = create_test_fs();

        // List root
        let entries = fs.list_dir("").expect("Should list directory");

        // Should have blog, projects
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"blog"));
        assert!(names.contains(&"projects"));
        assert!(!names.contains(&".profile"));
    }

    #[test]
    fn test_list_dir_sorting() {
        let fs = create_test_fs();

        let entries = fs.list_dir("").expect("Should list directory");

        // Directories should come before files
        let dir_indices: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_dir)
            .map(|(i, _)| i)
            .collect();
        let file_indices: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !e.is_dir)
            .map(|(i, _)| i)
            .collect();

        if let (Some(&last_dir), Some(&first_file)) = (dir_indices.last(), file_indices.first()) {
            assert!(
                last_dir < first_file,
                "Directories should come before files"
            );
        }
    }

    #[test]
    fn test_list_dir_on_file() {
        let fs = create_test_fs();
        let result = fs.list_dir("blog/hello.md");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_file_content_path() {
        let fs = create_test_fs();

        let content_path = fs.get_file_content_path("blog/hello.md");
        assert_eq!(content_path, Some("blog/hello.md".to_string()));

        // Directory should return None
        let dir_path = fs.get_file_content_path("blog");
        assert!(dir_path.is_none());
    }

    #[test]
    fn test_resolve_path() {
        let fs = create_test_fs();

        // Relative path from root
        let resolved = fs.resolve_path("", "blog");
        assert_eq!(resolved, Some("blog".to_string()));

        // Relative path from subdirectory
        let resolved = fs.resolve_path("blog", "hello.md");
        assert_eq!(resolved, Some("blog/hello.md".to_string()));

        // Non-existent path
        let resolved = fs.resolve_path("", "nonexistent");
        assert!(resolved.is_none());
    }

    #[test]
    fn test_resolve_path_string() {
        // Home expansion (~ means root = empty string)
        assert_eq!(VirtualFs::resolve_path_string("anywhere", "~"), "");
        assert_eq!(VirtualFs::resolve_path_string("anywhere", "~/blog"), "blog");

        // Relative path
        assert_eq!(VirtualFs::resolve_path_string("", "blog"), "blog");
        assert_eq!(
            VirtualFs::resolve_path_string("blog", "posts"),
            "blog/posts"
        );

        // Parent path
        assert_eq!(VirtualFs::resolve_path_string("blog/posts", ".."), "blog");
        assert_eq!(VirtualFs::resolve_path_string("blog", ".."), "");

        // Current path
        assert_eq!(VirtualFs::resolve_path_string("blog", "."), "blog");

        // Nested .. handling
        assert_eq!(VirtualFs::resolve_path_string("a/b/c", "../../d"), "a/d");
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(VirtualFs::normalize_path("home/./wonjae"), "home/wonjae");
        assert_eq!(VirtualFs::normalize_path("home/wonjae/../etc"), "home/etc");
        assert_eq!(VirtualFs::normalize_path("a/b/c/../../d"), "a/d");
        assert_eq!(VirtualFs::normalize_path(""), "");
        assert_eq!(VirtualFs::normalize_path("/../.."), "");
    }

    #[test]
    fn test_parent_path() {
        assert_eq!(VirtualFs::parent_path("home/wonjae"), "home");
        assert_eq!(VirtualFs::parent_path("home"), "");
        assert_eq!(VirtualFs::parent_path(""), "");
    }

    #[test]
    fn test_is_directory() {
        let fs = create_test_fs();

        assert!(fs.is_directory("")); // root
        assert!(fs.is_directory("blog"));
        assert!(!fs.is_directory("blog/hello.md"));
        assert!(!fs.is_directory("nonexistent"));
    }

    #[test]
    fn test_get_entry_nonexistent() {
        let fs = create_test_fs();

        assert!(fs.get_entry("nonexistent").is_none());
        assert!(fs.get_entry("blog/nonexistent.md").is_none());
    }

    #[test]
    fn test_permissions_directory() {
        let fs = create_test_fs();
        let entry = fs.get_entry("blog").unwrap();
        let perms = fs.get_permissions(entry, &WalletState::Disconnected);

        assert!(perms.is_dir);
        assert!(perms.read);
        assert!(!perms.write);
        assert!(perms.execute);
        assert_eq!(perms.to_string(), "dr-x");
    }

    #[test]
    fn test_permissions_file_unencrypted() {
        let fs = create_test_fs();
        let entry = fs.get_entry("blog/hello.md").unwrap();
        let perms = fs.get_permissions(entry, &WalletState::Disconnected);

        assert!(!perms.is_dir);
        assert!(perms.read);
        assert!(!perms.write);
        assert!(!perms.execute);
        assert_eq!(perms.to_string(), "-r--");
    }

    #[test]
    fn test_permissions_restricted_no_access() {
        use crate::models::AccessFilter;

        let entry = FsEntry::content_file_with_meta(
            "secret.enc",
            "Restricted file",
            FileMetadata {
                access: Some(AccessFilter { recipients: vec![] }),
                ..Default::default()
            },
        );

        let fs = VirtualFs::empty();
        let perms = fs.get_permissions(&entry, &WalletState::Disconnected);

        assert!(!perms.read);
        assert_eq!(perms.to_string(), "----");
    }

    #[test]
    fn test_permissions_restricted_with_access() {
        use crate::models::{AccessFilter, Recipient};

        let wallet = WalletState::Connected {
            address: "0x1234abcd".to_string(),
            ens_name: None,
            chain_id: Some(1),
        };

        let entry = FsEntry::content_file_with_meta(
            "secret.enc",
            "Restricted file",
            FileMetadata {
                access: Some(AccessFilter {
                    recipients: vec![Recipient {
                        address: "0x1234ABCD".to_string(),
                    }],
                }),
                ..Default::default()
            },
        );

        let fs = VirtualFs::empty();
        let perms = fs.get_permissions(&entry, &wallet);

        assert!(perms.read);
        assert_eq!(perms.to_string(), "-r--");
    }

    #[test]
    fn snapshot_roundtrip_is_byte_stable() {
        let golden = include_str!("../../tests/fixtures/manifest_golden.json");
        let snapshot =
            crate::core::storage::github::manifest::parse_snapshot(golden).expect("golden parses");

        let fs = VirtualFs::from_scanned_subtree(&snapshot);
        let reserialized = fs.to_scanned_subtree();
        let out = crate::core::storage::github::manifest::serialize_snapshot(&reserialized)
            .expect("serialize");

        assert_eq!(out.trim_end(), golden.trim_end());
    }

    #[test]
    fn scanned_subtree_sorts_regardless_of_input_order() {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "z.md".to_string(),
                    description: "Z".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "m.md".to_string(),
                    description: "M".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "a.md".to_string(),
                    description: "A".to_string(),
                    meta: FileMetadata::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: "z-dir".to_string(),
                    meta: DirectoryMetadata {
                        title: "Z".to_string(),
                        tags: vec!["zone".to_string()],
                        ..Default::default()
                    },
                },
                ScannedDirectory {
                    path: "a-dir".to_string(),
                    meta: DirectoryMetadata {
                        title: "A".to_string(),
                        tags: vec!["area".to_string()],
                        ..Default::default()
                    },
                },
            ],
        };

        let fs = VirtualFs::from_scanned_subtree(&snapshot);
        let out = fs.to_scanned_subtree();
        let file_paths: Vec<&str> = out.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(file_paths, vec!["a.md", "m.md", "z.md"]);
        let dir_paths: Vec<&str> = out.directories.iter().map(|d| d.path.as_str()).collect();
        assert_eq!(dir_paths, vec!["a-dir", "z-dir"]);
    }

    #[test]
    fn scanned_subtree_omits_synthetic_dotprofile() {
        let fs = VirtualFs::empty();
        let snapshot = fs.to_scanned_subtree();
        assert!(snapshot.files.iter().all(|f| f.path != ".profile"));

        let with_content = ScannedSubtree {
            files: vec![ScannedFile {
                path: "notes.md".to_string(),
                description: "Notes".to_string(),
                meta: FileMetadata::default(),
            }],
            directories: vec![],
        };
        let fs2 = VirtualFs::from_scanned_subtree(&with_content);
        let serialized = fs2.to_scanned_subtree();
        assert!(serialized.files.iter().all(|f| f.path != ".profile"));
        assert_eq!(serialized.files.len(), 1);
        assert_eq!(serialized.files[0].path, "notes.md");
    }
}
