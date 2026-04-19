//! Application routing for IPFS-compatible hash-based navigation.
//!
//! # URL Structure
//!
//! ```text
//! #/{alias}/{path}
//! ```
//!
//! | URL | Meaning |
//! |-----|---------|
//! | `#/~/` | Home directory (default mount) |
//! | `#/~/blog/` | Browse directory |
//! | `#/~/blog/post.md` | Read file |
//! | `#/work/docs/` | Custom mount with alias "work" |

use super::mount::Mount;
use crate::config::mounts;
use crate::utils::dom;

// ============================================================================
// AppRoute
// ============================================================================

/// Application route parsed from URL.
///
/// Routes are determined by URL structure:
/// - `/` or empty → Root (mount selection)
/// - `/{mount}/` → Browse (directory)
/// - `/{mount}/{path}/` → Browse (directory)
/// - `/{mount}/{path}` (with extension) → Read (file)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AppRoute {
    /// Root route - mount selection (`#/`)
    #[default]
    Root,

    /// Browse a directory
    Browse {
        /// Mount point
        mount: Mount,
        /// Path relative to mount root (empty string = root)
        path: String,
    },

    /// Read a file
    Read {
        /// Mount point
        mount: Mount,
        /// File path relative to mount root
        path: String,
    },
}

impl AppRoute {
    /// Create a home route (browse mount root).
    #[inline]
    pub fn home() -> Self {
        Self::Browse {
            mount: home_mount(),
            path: String::new(),
        }
    }

    /// Parse a URL path into an AppRoute.
    ///
    /// # Parsing Rules
    /// - `/` or empty → Root
    /// - `/{mount}/` → Browse (mount root)
    /// - `/{mount}/{path}/` → Browse (directory)
    /// - `/{mount}/{path}` (with `.` in filename) → Read (file)
    /// - `/{mount}/{path}` (no `.` in filename) → Browse (directory)
    ///
    /// # Examples
    /// ```ignore
    /// assert_eq!(AppRoute::from_path("/"), AppRoute::Root);
    /// assert_eq!(AppRoute::from_path("/~/"), AppRoute::Browse { ... });
    /// ```
    pub fn from_path(path: &str) -> Self {
        let path = path.trim_start_matches('/');

        if path.is_empty() {
            return Self::Root;
        }

        // Split into mount segment and rest
        let (mount_segment, rest) = match path.find('/') {
            Some(i) => (&path[..i], &path[i + 1..]),
            None => {
                // Just mount name without trailing slash (e.g., "~")
                return match resolve_mount(path) {
                    Some(mount) => Self::Browse {
                        mount,
                        path: String::new(),
                    },
                    None => Self::Root,
                };
            }
        };

        let mount = match resolve_mount(mount_segment) {
            Some(m) => m,
            None => return Self::Root,
        };

        // Check if path ends with slash (directory) or has no extension
        let has_trailing_slash = rest.ends_with('/');
        let rest = rest.trim_end_matches('/');

        if rest.is_empty() {
            // Mount root (e.g., "/~/")
            Self::Browse {
                mount,
                path: String::new(),
            }
        } else if has_trailing_slash {
            // Explicit directory (e.g., "/~/blog/")
            Self::Browse {
                mount,
                path: rest.to_string(),
            }
        } else {
            // Check if last segment has an extension
            let last_segment = rest.rsplit('/').next().unwrap_or(rest);
            if last_segment.contains('.') {
                // Has extension → file
                Self::Read {
                    mount,
                    path: rest.to_string(),
                }
            } else {
                // No extension → directory
                Self::Browse {
                    mount,
                    path: rest.to_string(),
                }
            }
        }
    }

    /// Convert route to URL path (without hash prefix).
    pub fn to_path(&self) -> String {
        match self {
            Self::Root => "/".to_string(),
            Self::Browse { mount, path } => {
                if path.is_empty() {
                    format!("/{}/", mount.alias())
                } else {
                    format!("/{}/{}/", mount.alias(), path)
                }
            }
            Self::Read { mount, path } => {
                format!("/{}/{}", mount.alias(), path)
            }
        }
    }

    /// Convert route to full hash URL.
    #[inline]
    pub fn to_hash(&self) -> String {
        format!("#{}", self.to_path())
    }

    /// Get the current route from browser URL hash.
    pub fn current() -> Self {
        Self::from_path(&dom::get_hash())
    }

    /// Navigate to this route by updating the browser hash.
    ///
    /// This adds a new entry to the browser history stack.
    pub fn push(&self) {
        dom::set_hash(&self.to_hash());
    }

    /// Replace the current route without adding to history.
    ///
    /// Useful for redirects that shouldn't be in the back button history.
    #[allow(dead_code)]
    pub fn replace(&self) {
        dom::replace_hash(&self.to_hash());
    }

    /// Get the filesystem path for VirtualFs operations.
    ///
    /// Returns a relative path for use with VirtualFs methods.
    /// This is the path within the mount, not an absolute path.
    ///
    /// # Examples
    /// - Root → ""
    /// - Browse { Home, "" } → ""
    /// - Browse { Home, "blog" } → "blog"
    /// - Read { Home, "blog/post.md" } → "blog/post.md"
    pub fn fs_path(&self) -> &str {
        match self {
            Self::Root => "",
            Self::Browse { path, .. } | Self::Read { path, .. } => path,
        }
    }

    /// Check if this route represents a file (Read).
    #[inline]
    pub fn is_file(&self) -> bool {
        matches!(self, Self::Read { .. })
    }

    /// Get content fetch URL for file routes.
    ///
    /// Returns `None` for non-file routes.
    pub fn content_url(&self) -> Option<String> {
        match self {
            Self::Read { mount, path } => Some(format!("{}/{}", mount.content_base_url(), path)),
            _ => None,
        }
    }

    /// Get the mount point for this route.
    pub fn mount(&self) -> Option<&Mount> {
        match self {
            Self::Root => None,
            Self::Browse { mount, .. } | Self::Read { mount, .. } => Some(mount),
        }
    }

    /// Get the path within the mount.
    pub fn path(&self) -> &str {
        match self {
            Self::Root => "",
            Self::Browse { path, .. } | Self::Read { path, .. } => path,
        }
    }

    /// Get the parent directory route.
    ///
    /// - Root → Root
    /// - Browse at mount root → Root (go to mount selection)
    /// - Browse/Read with path → Browse at parent directory
    pub fn parent(&self) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Browse { mount, path } | Self::Read { mount, path } => {
                if path.is_empty() {
                    // At mount root, go up to Root (mount selection)
                    Self::Root
                } else if let Some((parent, _)) = path.rsplit_once('/') {
                    Self::Browse {
                        mount: mount.clone(),
                        path: parent.to_string(),
                    }
                } else {
                    Self::Browse {
                        mount: mount.clone(),
                        path: String::new(),
                    }
                }
            }
        }
    }

    /// Get display path for terminal prompt.
    ///
    /// # Examples
    /// - Root → "/"
    /// - Browse { Home, "" } → "~"
    /// - Browse { Home, "blog" } → "~/blog"
    /// - Read { Home, "blog/post.md" } → "~/blog/post.md"
    pub fn display_path(&self) -> String {
        match self {
            Self::Root => "/".to_string(),
            Self::Browse { mount, path } | Self::Read { mount, path } => {
                let alias = mount.alias();
                let prefix = if alias == "~" { "~" } else { alias };
                if path.is_empty() {
                    prefix.to_string()
                } else {
                    format!("{}/{}", prefix, path)
                }
            }
        }
    }

    /// Refine route against the filesystem.
    ///
    /// `AppRoute::from_path` uses an extension heuristic: it cannot distinguish
    /// a file named `Makefile` (no extension) from a directory, or a directory
    /// named `archive.2024` (dot in name) from a file. `resolve` queries the
    /// actual `VirtualFs` and corrects the variant.
    ///
    /// When the path is not known to the filesystem, the route is returned
    /// unchanged (heuristic fallback).
    pub fn resolve(self, fs: &crate::core::VirtualFs) -> Self {
        match self {
            Self::Root => Self::Root,
            Self::Browse { mount, path } | Self::Read { mount, path } => {
                if path.is_empty() {
                    return Self::Browse { mount, path };
                }
                if fs.is_directory(&path) {
                    Self::Browse { mount, path }
                } else if fs.get_entry(&path).is_some() {
                    Self::Read { mount, path }
                } else {
                    // FS has no info — fall back to heuristic
                    // by reconstructing from_path's decision.
                    let last = path.rsplit('/').next().unwrap_or(&path);
                    if last.contains('.') {
                        Self::Read { mount, path }
                    } else {
                        Self::Browse { mount, path }
                    }
                }
            }
        }
    }

    /// Join a relative path to this route (for navigation).
    ///
    /// # Arguments
    /// * `relative` - Relative path to join
    ///
    /// # Examples
    /// - Browse("blog") + "posts" → Browse("blog/posts")
    /// - Browse("blog") + ".." → Browse("")
    /// - Browse("blog") + "post.md" → Read("blog/post.md")
    pub fn join(&self, relative: &str) -> Self {
        let (mount, current_path) = match self {
            Self::Root => (home_mount(), ""),
            Self::Browse { mount, path } => (mount.clone(), path.as_str()),
            Self::Read { mount, path } => {
                // For files, join relative to parent directory
                let parent = path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
                (mount.clone(), parent)
            }
        };

        // Handle special cases
        match relative {
            "" | "." => {
                return Self::Browse {
                    mount,
                    path: current_path.to_string(),
                };
            }
            "~" => return Self::home(),
            ".." => return self.parent(),
            _ => {}
        }

        // Handle ".." prefix
        let mut segments: Vec<&str> = if current_path.is_empty() {
            Vec::new()
        } else {
            current_path.split('/').collect()
        };

        for part in relative.split('/') {
            match part {
                "" | "." => continue,
                ".." => {
                    segments.pop();
                }
                _ => segments.push(part),
            }
        }

        let new_path = segments.join("/");

        // Check if result is a file (has extension in last segment)
        let last_segment = segments.last().copied().unwrap_or("");
        if last_segment.contains('.') {
            Self::Read {
                mount,
                path: new_path,
            }
        } else {
            Self::Browse {
                mount,
                path: new_path,
            }
        }
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the home mount from the global registry.
fn home_mount() -> Mount {
    mounts().home().clone()
}

/// Resolve an alias to a mount via the global registry.
fn resolve_mount(alias: &str) -> Option<Mount> {
    mounts().resolve(alias).cloned()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mount() -> Mount {
        Mount::github("~", "https://example.com")
    }

    // ------------------------------------------------------------------------
    // AppRoute::from_path tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_route_from_path_root() {
        assert_eq!(AppRoute::from_path(""), AppRoute::Root);
        assert_eq!(AppRoute::from_path("/"), AppRoute::Root);
    }

    #[test]
    fn test_route_from_path_mount_root() {
        let route = AppRoute::from_path("/~/");
        match route {
            AppRoute::Browse { mount, path } => {
                assert_eq!(mount.alias(), "~");
                assert_eq!(path, "");
            }
            _ => panic!("Expected Browse"),
        }
    }

    #[test]
    fn test_route_from_path_browse() {
        let route = AppRoute::from_path("/~/blog/");
        match route {
            AppRoute::Browse { mount, path } => {
                assert_eq!(mount.alias(), "~");
                assert_eq!(path, "blog");
            }
            _ => panic!("Expected Browse"),
        }
    }

    #[test]
    fn test_route_from_path_read() {
        let route = AppRoute::from_path("/~/blog/post.md");
        match route {
            AppRoute::Read { mount, path } => {
                assert_eq!(mount.alias(), "~");
                assert_eq!(path, "blog/post.md");
            }
            _ => panic!("Expected Read"),
        }
    }

    #[test]
    fn test_route_from_path_unknown_mount() {
        assert_eq!(AppRoute::from_path("/unknown/"), AppRoute::Root);
    }

    // ------------------------------------------------------------------------
    // AppRoute::to_path tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_route_to_path() {
        assert_eq!(AppRoute::Root.to_path(), "/");

        let browse_root = AppRoute::Browse {
            mount: test_mount(),
            path: String::new(),
        };
        assert_eq!(browse_root.to_path(), "/~/");

        let browse_dir = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        assert_eq!(browse_dir.to_path(), "/~/blog/");

        let read_file = AppRoute::Read {
            mount: test_mount(),
            path: "blog/post.md".to_string(),
        };
        assert_eq!(read_file.to_path(), "/~/blog/post.md");
    }

    #[test]
    fn test_route_to_hash() {
        assert_eq!(AppRoute::Root.to_hash(), "#/");
    }

    // ------------------------------------------------------------------------
    // Helper method tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_route_is_file() {
        assert!(!AppRoute::Root.is_file());

        let browse = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        assert!(!browse.is_file());

        let read = AppRoute::Read {
            mount: test_mount(),
            path: "blog/post.md".to_string(),
        };
        assert!(read.is_file());
    }

    #[test]
    fn test_route_parent() {
        let mount_root = AppRoute::Browse {
            mount: test_mount(),
            path: String::new(),
        };
        // Mount root's parent is Root (mount selection)
        assert_eq!(mount_root.parent(), AppRoute::Root);

        let blog = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        assert_eq!(blog.parent(), mount_root);

        let file = AppRoute::Read {
            mount: test_mount(),
            path: "blog/post.md".to_string(),
        };
        assert_eq!(file.parent(), blog);
    }

    #[test]
    fn test_route_display_path() {
        assert_eq!(AppRoute::Root.display_path(), "/");

        let browse = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        assert_eq!(browse.display_path(), "~/blog");
    }

    #[test]
    fn test_route_content_url() {
        let browse = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        assert_eq!(browse.content_url(), None);

        let read = AppRoute::Read {
            mount: test_mount(),
            path: "blog/post.md".to_string(),
        };
        assert_eq!(
            read.content_url(),
            Some("https://example.com/blog/post.md".to_string())
        );
    }

    // ------------------------------------------------------------------------
    // AppRoute::resolve tests
    // ------------------------------------------------------------------------

    use crate::core::VirtualFs;
    use crate::models::{DirectoryEntry, FileEntry, Manifest};

    fn fs_with_entries() -> VirtualFs {
        // Build a manifest with:
        //   /Makefile                 (file, no extension)
        //   /archive.2024/            (directory, dot in name)
        //   /archive.2024/index.md    (file)
        //   /blog/                    (directory)
        //   /blog/post.md             (file)
        let manifest = Manifest {
            files: vec![
                FileEntry {
                    path: "Makefile".into(),
                    title: "Makefile".into(),
                    size: None,
                    modified: None,
                    tags: vec![],
                    encryption: None,
                },
                FileEntry {
                    path: "archive.2024/index.md".into(),
                    title: "Archive".into(),
                    size: None,
                    modified: None,
                    tags: vec![],
                    encryption: None,
                },
                FileEntry {
                    path: "blog/post.md".into(),
                    title: "Post".into(),
                    size: None,
                    modified: None,
                    tags: vec![],
                    encryption: None,
                },
            ],
            directories: vec![
                DirectoryEntry {
                    path: "archive.2024".into(),
                    title: "Archive".into(),
                    tags: vec![],
                    description: None,
                    icon: None,
                    thumbnail: None,
                },
                DirectoryEntry {
                    path: "blog".into(),
                    title: "Blog".into(),
                    tags: vec![],
                    description: None,
                    icon: None,
                    thumbnail: None,
                },
            ],
        };
        VirtualFs::from_manifest(&manifest)
    }

    #[test]
    fn test_resolve_root_stays_root() {
        let fs = fs_with_entries();
        assert_eq!(AppRoute::Root.resolve(&fs), AppRoute::Root);
    }

    #[test]
    fn test_resolve_promotes_makefile_to_read() {
        // `Makefile` has no extension → heuristic parses as Browse, but it IS a file.
        let fs = fs_with_entries();
        let parsed = AppRoute::Browse {
            mount: test_mount(),
            path: "Makefile".to_string(),
        };
        let resolved = parsed.resolve(&fs);
        assert!(matches!(resolved, AppRoute::Read { ref path, .. } if path == "Makefile"));
    }

    #[test]
    fn test_resolve_demotes_archive_dir_to_browse() {
        // `archive.2024` has a `.` → heuristic parses as Read, but it IS a directory.
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: "archive.2024".to_string(),
        };
        let resolved = parsed.resolve(&fs);
        assert!(matches!(resolved, AppRoute::Browse { ref path, .. } if path == "archive.2024"));
    }

    #[test]
    fn test_resolve_keeps_correct_browse() {
        let fs = fs_with_entries();
        let parsed = AppRoute::Browse {
            mount: test_mount(),
            path: "blog".to_string(),
        };
        let resolved = parsed.clone().resolve(&fs);
        assert_eq!(resolved, parsed);
    }

    #[test]
    fn test_resolve_keeps_correct_read() {
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: "blog/post.md".to_string(),
        };
        let resolved = parsed.clone().resolve(&fs);
        assert_eq!(resolved, parsed);
    }

    #[test]
    fn test_resolve_unknown_path_keeps_heuristic() {
        // When fs has no info about the path, resolve must be a no-op —
        // falls back to whatever from_path produced.
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: "nonexistent.md".to_string(),
        };
        let resolved = parsed.clone().resolve(&fs);
        assert_eq!(resolved, parsed);
    }

    #[test]
    fn test_resolve_empty_path_is_browse_root() {
        let fs = fs_with_entries();
        let parsed = AppRoute::Read {
            mount: test_mount(),
            path: String::new(),
        };
        let resolved = parsed.resolve(&fs);
        assert!(matches!(
            resolved,
            AppRoute::Browse { ref path, .. } if path.is_empty()
        ));
    }
}
