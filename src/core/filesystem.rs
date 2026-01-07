use crate::models::{
    DisplayPermissions, FileMetadata, FsEntry, ManifestEntry, VirtualPath, WalletState,
};
use std::collections::HashMap;

/// Directory entry returned by list_dir
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub description: String,
    pub meta: FileMetadata,
}

/// Virtual filesystem for the terminal
#[derive(Clone)]
pub struct VirtualFs {
    root: FsEntry,
}

impl VirtualFs {
    /// Create filesystem from manifest entries (dynamic content)
    pub fn from_manifest(entries: &[ManifestEntry]) -> Self {
        let mut content_tree: HashMap<String, FsEntry> = HashMap::new();

        for entry in entries {
            Self::insert_path(
                &mut content_tree,
                &entry.path,
                &entry.path,
                &entry.title,
                entry.to_metadata(),
            );
        }

        // Add static files
        content_tree.insert(
            ".profile".to_string(),
            FsEntry::file("User profile configuration"),
        );

        let root = FsEntry::dir(vec![
            (
                "home",
                FsEntry::dir(vec![(
                    "wonjae",
                    FsEntry::Directory {
                        children: content_tree,
                        description: String::new(),
                        meta: FileMetadata::default(),
                    },
                )]),
            ),
            (
                "etc",
                FsEntry::dir(vec![("motd", FsEntry::file("Message of the day"))]),
            ),
        ]);

        Self { root }
    }

    /// Insert a path into the tree using iteration instead of recursion.
    fn insert_path(
        tree: &mut HashMap<String, FsEntry>,
        path: &str,
        full_path: &str,
        title: &str,
        meta: FileMetadata,
    ) {
        let parts: Vec<&str> = path.split('/').collect();
        let mut current = tree;

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;

            if is_last {
                current.insert(
                    part.to_string(),
                    FsEntry::content_file_with_meta(full_path, title, meta.clone()),
                );
            } else {
                let entry = current
                    .entry(part.to_string())
                    .or_insert_with(|| FsEntry::Directory {
                        children: HashMap::new(),
                        description: String::new(),
                        meta: FileMetadata::default(),
                    });

                current = match entry {
                    FsEntry::Directory { children, .. } => children,
                    FsEntry::File { .. } => {
                        // A file exists where we expect a directory - skip this entry.
                        // This indicates a manifest conflict (e.g., "a/b" and "a/b/c").
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

    /// Create empty filesystem (fallback when manifest fails to load)
    pub fn empty() -> Self {
        let root = FsEntry::dir(vec![
            (
                "home",
                FsEntry::dir(vec![(
                    "wonjae",
                    FsEntry::dir(vec![(
                        ".profile",
                        FsEntry::file("User profile configuration"),
                    )]),
                )]),
            ),
            (
                "etc",
                FsEntry::dir(vec![("motd", FsEntry::file("Message of the day"))]),
            ),
        ]);

        Self { root }
    }

    /// Resolve a path relative to current directory.
    /// Returns the resolved VirtualPath if the target exists in the filesystem.
    pub fn resolve_path(&self, current: &VirtualPath, path: &str) -> Option<VirtualPath> {
        let resolved = current.resolve(path);

        // Verify path exists
        if self.get_entry(resolved.as_str()).is_some() {
            Some(resolved)
        } else {
            None
        }
    }

    pub fn get_entry(&self, path: &str) -> Option<&FsEntry> {
        if path == "/" {
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
    /// Returns: Vec<(name, is_dir, description, metadata)>
    pub fn list_dir(&self, path: &str) -> Option<Vec<DirEntry>> {
        match self.get_entry(path)? {
            FsEntry::Directory { children, .. } => {
                let mut items: Vec<_> = children
                    .iter()
                    .map(|(name, entry)| {
                        let is_dir = entry.is_directory();
                        let (desc, meta) = match entry {
                            FsEntry::Directory { meta, .. } => ("directory".to_string(), meta),
                            FsEntry::File {
                                description, meta, ..
                            } => (description.clone(), meta),
                        };
                        DirEntry {
                            name: name.clone(),
                            is_dir,
                            description: desc,
                            meta: meta.clone(),
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

    pub fn get_file_content_path(&self, path: &str) -> Option<String> {
        match self.get_entry(path)? {
            FsEntry::File { content_path, .. } => content_path.clone(),
            _ => None,
        }
    }

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
    use crate::models::VirtualPath;

    fn create_test_fs() -> VirtualFs {
        let entries = vec![
            ManifestEntry {
                path: "blog/hello.md".to_string(),
                title: "Hello World".to_string(),
                size: Some(1234),
                created: Some(1704067200),
                modified: Some(1704153600),
                encryption: None,
            },
            ManifestEntry {
                path: "blog/rust.md".to_string(),
                title: "Learning Rust".to_string(),
                size: Some(2048),
                created: None,
                modified: None,
                encryption: None,
            },
            ManifestEntry {
                path: "projects/web/app.md".to_string(),
                title: "Web App".to_string(),
                size: None,
                created: None,
                modified: None,
                encryption: None,
            },
        ];
        VirtualFs::from_manifest(&entries)
    }

    #[test]
    fn test_empty_fs() {
        let fs = VirtualFs::empty();
        assert!(fs.get_entry("/").is_some());
        assert!(fs.get_entry("/home").is_some());
        assert!(fs.get_entry("/home/wonjae").is_some());
    }

    #[test]
    fn test_from_manifest() {
        let fs = create_test_fs();

        // Check root exists
        assert!(fs.get_entry("/").is_some());

        // Check home directory
        assert!(fs.is_directory("/home"));
        assert!(fs.is_directory("/home/wonjae"));

        // Check blog directory was created
        assert!(fs.is_directory("/home/wonjae/blog"));

        // Check files were created
        assert!(fs.get_entry("/home/wonjae/blog/hello.md").is_some());
        assert!(!fs.is_directory("/home/wonjae/blog/hello.md"));
    }

    #[test]
    fn test_nested_paths() {
        let fs = create_test_fs();

        // Check deeply nested path
        assert!(fs.is_directory("/home/wonjae/projects"));
        assert!(fs.is_directory("/home/wonjae/projects/web"));
        assert!(fs.get_entry("/home/wonjae/projects/web/app.md").is_some());
    }

    #[test]
    fn test_list_dir() {
        let fs = create_test_fs();

        // List home/wonjae
        let entries = fs.list_dir("/home/wonjae").expect("Should list directory");

        // Should have blog, projects, .profile
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"blog"));
        assert!(names.contains(&"projects"));
        assert!(names.contains(&".profile"));
    }

    #[test]
    fn test_list_dir_sorting() {
        let fs = create_test_fs();

        let entries = fs.list_dir("/home/wonjae").expect("Should list directory");

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
        let result = fs.list_dir("/home/wonjae/blog/hello.md");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_file_content_path() {
        let fs = create_test_fs();

        let content_path = fs.get_file_content_path("/home/wonjae/blog/hello.md");
        assert_eq!(content_path, Some("blog/hello.md".to_string()));

        // Directory should return None
        let dir_path = fs.get_file_content_path("/home/wonjae/blog");
        assert!(dir_path.is_none());
    }

    #[test]
    fn test_resolve_path() {
        let fs = create_test_fs();
        let home = VirtualPath::new("/home/wonjae");

        // Absolute path
        let resolved = fs.resolve_path(&home, "/home/wonjae/blog");
        assert_eq!(
            resolved.map(|p| p.to_string()),
            Some("/home/wonjae/blog".to_string())
        );

        // Relative path
        let resolved = fs.resolve_path(&home, "blog");
        assert_eq!(
            resolved.map(|p| p.to_string()),
            Some("/home/wonjae/blog".to_string())
        );

        // Non-existent path
        let resolved = fs.resolve_path(&home, "nonexistent");
        assert!(resolved.is_none());
    }

    #[test]
    fn test_is_directory() {
        let fs = create_test_fs();

        assert!(fs.is_directory("/"));
        assert!(fs.is_directory("/home"));
        assert!(fs.is_directory("/home/wonjae/blog"));
        assert!(!fs.is_directory("/home/wonjae/blog/hello.md"));
        assert!(!fs.is_directory("/nonexistent"));
    }

    #[test]
    fn test_get_entry_nonexistent() {
        let fs = create_test_fs();

        assert!(fs.get_entry("/nonexistent").is_none());
        assert!(fs.get_entry("/home/wonjae/nonexistent").is_none());
        assert!(fs.get_entry("/home/wonjae/blog/nonexistent.md").is_none());
    }

    #[test]
    fn test_permissions_directory() {
        let fs = create_test_fs();
        let entry = fs.get_entry("/home/wonjae/blog").unwrap();
        let perms = fs.get_permissions(entry, &WalletState::Disconnected);

        assert!(perms.is_dir);
        assert!(perms.read);
        assert!(!perms.write); // Always false for now (Phase 2)
        assert!(perms.execute);
        assert_eq!(perms.to_string(), "dr-x"); // No write permission yet
    }

    #[test]
    fn test_permissions_file_unencrypted() {
        let fs = create_test_fs();
        let entry = fs.get_entry("/home/wonjae/blog/hello.md").unwrap();
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
                        recipient: "0x1234ABCD".to_string(), // case-insensitive match
                        encrypted_key: "base64key".to_string(),
                    }],
                }),
                ..Default::default()
            },
        );

        let fs = VirtualFs::empty();
        let perms = fs.get_permissions(&entry, &wallet);

        assert!(perms.read); // We have access!
        assert_eq!(perms.to_string(), "-r--");
    }
}
