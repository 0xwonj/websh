use crate::models::{
    DirectoryEntry, DirectoryMetadata, DisplayPermissions, FileMetadata, FsEntry, Manifest,
    WalletState,
};
use std::collections::HashMap;

/// Directory entry returned by list_dir
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
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
}

impl VirtualFs {
    /// Create filesystem from manifest.
    ///
    /// Manifest paths are relative (e.g., `blog/post.md`).
    pub fn from_manifest(manifest: &Manifest) -> Self {
        // Build directory metadata map for quick lookup
        let dir_meta_map: HashMap<String, &DirectoryEntry> = manifest
            .directories
            .iter()
            .map(|d| (d.path.clone(), d))
            .collect();

        let mut content_tree: HashMap<String, FsEntry> = HashMap::new();

        // Create all files (this also creates parent directories)
        for file in &manifest.files {
            Self::insert_path(
                &mut content_tree,
                &file.path,
                &file.path,
                &file.title,
                file.to_metadata(),
                &dir_meta_map,
            );
        }

        // Ensure directories from manifest exist (even if empty)
        for dir in &manifest.directories {
            if !dir.path.is_empty() {
                Self::ensure_directory(&mut content_tree, &dir.path, &dir_meta_map);
            }
        }

        // Add static files
        content_tree.insert(
            ".profile".to_string(),
            FsEntry::file("User profile configuration"),
        );

        // Build root metadata
        let root_meta = dir_meta_map
            .get("")
            .map(|d| DirectoryMetadata {
                title: d.title.clone(),
                description: d.description.clone(),
                icon: d.icon.clone(),
                thumbnail: d.thumbnail.clone(),
                tags: d.tags.clone(),
            })
            .unwrap_or_default();

        let root = FsEntry::Directory {
            children: content_tree,
            meta: root_meta,
        };

        Self { root }
    }

    /// Insert a path into the tree using iteration instead of recursion.
    fn insert_path(
        tree: &mut HashMap<String, FsEntry>,
        path: &str,
        full_path: &str,
        title: &str,
        meta: FileMetadata,
        dir_meta_map: &HashMap<String, &DirectoryEntry>,
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
                            title: d.title.clone(),
                            description: d.description.clone(),
                            icon: d.icon.clone(),
                            thumbnail: d.thumbnail.clone(),
                            tags: d.tags.clone(),
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
                                "Manifest conflict: '{}' blocked by existing file",
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
        dir_meta_map: &HashMap<String, &DirectoryEntry>,
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
                        title: d.title.clone(),
                        description: d.description.clone(),
                        icon: d.icon.clone(),
                        thumbnail: d.thumbnail.clone(),
                        tags: d.tags.clone(),
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
        let mut content_tree: HashMap<String, FsEntry> = HashMap::new();
        content_tree.insert(
            ".profile".to_string(),
            FsEntry::file("User profile configuration"),
        );

        let root = FsEntry::Directory {
            children: content_tree,
            meta: DirectoryMetadata::default(),
        };

        Self { root }
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

    /// List directory contents with metadata.
    ///
    /// # Arguments
    /// - `path`: Relative path to directory (empty string for root)
    ///
    /// # Returns
    /// Sorted list of entries (directories first, then files, hidden last).
    pub fn list_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
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

    /// Compute display permissions for an entry at runtime.
    ///
    /// Permissions are computed based on:
    /// - `d`: Directory or file
    /// - `r`: Encrypted files require wallet address in wrapped_keys
    /// - `w`: Admin login (not yet implemented, always false for now)
    /// - `x`: Directories only
    pub fn get_permissions(&self, entry: &FsEntry, wallet: &WalletState) -> DisplayPermissions {
        let is_dir = entry.is_directory();

        // Read permission: unencrypted = always readable, encrypted = check wrapped_keys
        let read = match entry {
            FsEntry::Directory { .. } => true,
            FsEntry::File { meta, .. } => {
                if let Some(ref enc) = meta.encryption {
                    // Encrypted: check if wallet address is in recipients
                    match wallet {
                        WalletState::Connected { address, .. } => enc
                            .wrapped_keys
                            .iter()
                            .any(|k| k.recipient.eq_ignore_ascii_case(address)),
                        _ => false,
                    }
                } else {
                    // Unencrypted: always readable
                    true
                }
            }
        };

        // Write permission: TODO - implement admin check, permissionless mount check
        // For now, always false (read-only)
        let write = false;

        // Execute permission: directories only
        let execute = is_dir;

        DisplayPermissions {
            is_dir,
            read,
            write,
            execute,
        }
    }
}

impl Default for VirtualFs {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FileEntry;

    fn create_test_fs() -> VirtualFs {
        let manifest = Manifest {
            files: vec![
                FileEntry {
                    path: "blog/hello.md".to_string(),
                    title: "Hello World".to_string(),
                    size: Some(1234),
                    modified: Some(1704153600),
                    tags: vec!["rust".to_string(), "intro".to_string()],
                    encryption: None,
                },
                FileEntry {
                    path: "blog/rust.md".to_string(),
                    title: "Learning Rust".to_string(),
                    size: Some(2048),
                    modified: None,
                    tags: vec![],
                    encryption: None,
                },
                FileEntry {
                    path: "projects/web/app.md".to_string(),
                    title: "Web App".to_string(),
                    size: None,
                    modified: None,
                    tags: vec![],
                    encryption: None,
                },
            ],
            directories: vec![
                DirectoryEntry {
                    path: "blog".to_string(),
                    title: "Blog Posts".to_string(),
                    tags: vec!["posts".to_string()],
                    description: None,
                    icon: None,
                    thumbnail: None,
                },
                DirectoryEntry {
                    path: String::new(),
                    title: "Home".to_string(),
                    tags: vec!["root".to_string()],
                    description: None,
                    icon: None,
                    thumbnail: None,
                },
            ],
        };
        VirtualFs::from_manifest(&manifest)
    }

    #[test]
    fn test_empty_fs() {
        let fs = VirtualFs::empty();
        // Root is empty string
        assert!(fs.get_entry("").is_some());
        assert!(fs.get_entry(".profile").is_some());
    }

    #[test]
    fn test_from_manifest() {
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

        // Check directory title was set from manifest
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

        // Should have blog, projects, .profile
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"blog"));
        assert!(names.contains(&"projects"));
        assert!(names.contains(&".profile"));
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
    fn test_permissions_encrypted_no_access() {
        use crate::models::EncryptionInfo;

        // Create encrypted file entry
        let entry = FsEntry::content_file_with_meta(
            "secret.enc",
            "Encrypted file",
            FileMetadata {
                encryption: Some(EncryptionInfo {
                    algorithm: "AES-256-GCM".to_string(),
                    wrapped_keys: vec![],
                }),
                ..Default::default()
            },
        );

        let fs = VirtualFs::empty();
        let perms = fs.get_permissions(&entry, &WalletState::Disconnected);

        assert!(!perms.read);
        assert_eq!(perms.to_string(), "----");
    }

    #[test]
    fn test_permissions_encrypted_with_access() {
        use crate::models::{EncryptionInfo, WrappedKey};

        let wallet = WalletState::Connected {
            address: "0x1234abcd".to_string(),
            ens_name: None,
            chain_id: Some(1),
        };

        // Create encrypted file entry with our address in recipients
        let entry = FsEntry::content_file_with_meta(
            "secret.enc",
            "Encrypted file",
            FileMetadata {
                encryption: Some(EncryptionInfo {
                    algorithm: "AES-256-GCM".to_string(),
                    wrapped_keys: vec![WrappedKey {
                        recipient: "0x1234ABCD".to_string(),
                        encrypted_key: "base64key".to_string(),
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
}
