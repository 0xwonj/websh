use std::collections::HashMap;
use crate::models::{FsEntry, ManifestEntry, VirtualPath};

/// Virtual filesystem for the terminal
#[derive(Clone)]
pub struct VirtualFs {
    root: FsEntry,
}

impl VirtualFs {
    /// Create filesystem from manifest entries (dynamic content)
    /// Supports nested paths like "projects/games/zkdungeon.md"
    pub fn from_manifest(entries: &[ManifestEntry]) -> Self {
        let mut content_tree: HashMap<String, FsEntry> = HashMap::new();

        for entry in entries {
            Self::insert_path(&mut content_tree, &entry.path, &entry.path, &entry.title);
        }

        // Add static files
        content_tree.insert(
            ".profile".to_string(),
            FsEntry::file("User profile configuration"),
        );

        let root = FsEntry::dir(vec![
            ("home", FsEntry::dir(vec![
                ("wonjae", FsEntry::Directory(content_tree)),
            ])),
            ("etc", FsEntry::dir(vec![
                ("motd", FsEntry::file("Message of the day")),
            ])),
        ]);

        Self { root }
    }

    /// Insert a path into the tree using iteration instead of recursion.
    fn insert_path(tree: &mut HashMap<String, FsEntry>, path: &str, full_path: &str, title: &str) {
        let parts: Vec<&str> = path.split('/').collect();
        let mut current = tree;

        for (i, part) in parts.iter().enumerate() {
            let is_last = i == parts.len() - 1;

            if is_last {
                current.insert(part.to_string(), FsEntry::content_file(full_path, title));
            } else {
                let entry = current
                    .entry(part.to_string())
                    .or_insert_with(|| FsEntry::Directory(HashMap::new()));

                current = match entry {
                    FsEntry::Directory(subtree) => subtree,
                    FsEntry::File { .. } => {
                        // A file exists where we expect a directory - skip this entry.
                        // This indicates a manifest conflict (e.g., "a/b" and "a/b/c").
                        #[cfg(target_arch = "wasm32")]
                        web_sys::console::warn_1(
                            &format!("Manifest conflict: '{}' blocked by existing file", full_path).into()
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
            ("home", FsEntry::dir(vec![
                ("wonjae", FsEntry::dir(vec![
                    (".profile", FsEntry::file("User profile configuration")),
                ])),
            ])),
            ("etc", FsEntry::dir(vec![
                ("motd", FsEntry::file("Message of the day")),
            ])),
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
                FsEntry::Directory(entries) => {
                    current = entries.get(part)?;
                }
                FsEntry::File { .. } => return None,
            }
        }

        Some(current)
    }

    pub fn list_dir(&self, path: &str) -> Option<Vec<(String, bool, String)>> {
        match self.get_entry(path)? {
            FsEntry::Directory(entries) => {
                let mut items: Vec<_> = entries
                    .iter()
                    .map(|(name, entry)| {
                        let is_dir = matches!(entry, FsEntry::Directory(_));
                        let desc = match entry {
                            FsEntry::Directory(_) => "directory".to_string(),
                            FsEntry::File { description, .. } => description.clone(),
                        };
                        (name.clone(), is_dir, desc)
                    })
                    .collect();
                items.sort_by(|a, b| {
                    match (a.1, b.1) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => a.0.cmp(&b.0),
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
        matches!(self.get_entry(path), Some(FsEntry::Directory(_)))
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
            },
            ManifestEntry {
                path: "blog/rust.md".to_string(),
                title: "Learning Rust".to_string(),
            },
            ManifestEntry {
                path: "projects/web/app.md".to_string(),
                title: "Web App".to_string(),
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
        let names: Vec<_> = entries.iter().map(|(n, _, _)| n.as_str()).collect();
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
            .filter(|(_, (_, is_dir, _))| *is_dir)
            .map(|(i, _)| i)
            .collect();
        let file_indices: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, (_, is_dir, _))| !*is_dir)
            .map(|(i, _)| i)
            .collect();

        if let (Some(&last_dir), Some(&first_file)) = (dir_indices.last(), file_indices.first()) {
            assert!(last_dir < first_file, "Directories should come before files");
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
        assert_eq!(resolved.map(|p| p.to_string()), Some("/home/wonjae/blog".to_string()));

        // Relative path
        let resolved = fs.resolve_path(&home, "blog");
        assert_eq!(resolved.map(|p| p.to_string()), Some("/home/wonjae/blog".to_string()));

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
}
