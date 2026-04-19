//! Staged changes for commit.
//!
//! Staged changes are selected from pending changes for the next commit.
//! This provides a Git-like staging area workflow.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Set of paths staged for commit.
///
/// Staged paths reference pending changes - only paths that exist in
/// PendingChanges can be staged. When committed, only staged paths
/// are pushed to the storage backend.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StagedChanges {
    /// Set of paths staged for commit
    paths: HashSet<String>,
}

impl StagedChanges {
    /// Create empty staged changes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stage a path for commit.
    pub fn add(&mut self, path: String) {
        self.paths.insert(path);
    }

    /// Unstage a path.
    pub fn remove(&mut self, path: &str) {
        self.paths.remove(path);
    }

    /// Check if a path is staged.
    pub fn is_staged(&self, path: &str) -> bool {
        self.paths.contains(path)
    }

    /// Get all staged paths.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.iter().map(|s| s.as_str())
    }

    /// Count of staged paths.
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Clear all staged paths.
    pub fn clear(&mut self) {
        self.paths.clear();
    }

    /// Stage all paths from an iterator.
    pub fn add_all(&mut self, paths: impl IntoIterator<Item = String>) {
        for path in paths {
            self.paths.insert(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_staged_changes_add_remove() {
        let mut staged = StagedChanges::new();
        staged.add("file1.md".to_string());
        staged.add("file2.md".to_string());

        assert!(staged.is_staged("file1.md"));
        assert!(staged.is_staged("file2.md"));
        assert!(!staged.is_staged("file3.md"));
        assert_eq!(staged.len(), 2);

        staged.remove("file1.md");
        assert!(!staged.is_staged("file1.md"));
        assert_eq!(staged.len(), 1);
    }

    #[test]
    fn test_staged_changes_clear() {
        let mut staged = StagedChanges::new();
        staged.add("file1.md".to_string());
        staged.add("file2.md".to_string());

        staged.clear();
        assert!(staged.is_empty());
    }
}
