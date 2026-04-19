//! Pending filesystem changes tracking.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::models::{DirectoryMetadata, FileMetadata};
use crate::utils::current_timestamp;

/// Type of pending change.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChangeType {
    /// Create a new file
    CreateFile {
        content: String,
        description: String,
        meta: FileMetadata,
    },
    /// Create a new binary file (image, etc.)
    CreateBinaryFile {
        /// Base64-encoded content
        content_base64: String,
        /// MIME type (e.g., "image/png")
        mime_type: String,
        description: String,
        meta: FileMetadata,
    },
    /// Update existing file content
    UpdateFile {
        content: String,
        /// None = keep existing description
        description: Option<String>,
    },
    /// Delete a file
    DeleteFile,
    /// Create a new directory
    CreateDirectory { meta: DirectoryMetadata },
    /// Delete a directory
    DeleteDirectory,
}

/// A single pending change.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingChange {
    /// Path relative to mount root
    pub path: String,
    /// Type of change
    pub change_type: ChangeType,
    /// Timestamp when change was made
    pub timestamp: u64,
}

/// Collection of pending changes (overlay layer).
///
/// Changes are stored in a HashMap for O(1) lookup, with a separate
/// Vec maintaining insertion order for deterministic iteration.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PendingChanges {
    /// Map of path -> change (latest change per path)
    changes: HashMap<String, PendingChange>,
    /// Order of changes (for deterministic iteration)
    order: Vec<String>,
}

impl PendingChanges {
    /// Create empty pending changes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or update a pending change.
    pub fn add(&mut self, path: String, change_type: ChangeType) {
        let timestamp = current_timestamp();
        let change = PendingChange {
            path: path.clone(),
            change_type,
            timestamp,
        };

        if !self.changes.contains_key(&path) {
            self.order.push(path.clone());
        }
        self.changes.insert(path, change);
    }

    /// Remove a pending change (discard).
    pub fn remove(&mut self, path: &str) {
        self.changes.remove(path);
        self.order.retain(|p| p != path);
    }

    /// Get a pending change by path.
    pub fn get(&self, path: &str) -> Option<&PendingChange> {
        self.changes.get(path)
    }

    /// Check if a path has pending changes.
    pub fn has_change(&self, path: &str) -> bool {
        self.changes.contains_key(path)
    }

    /// Check if path is marked for deletion.
    pub fn is_deleted(&self, path: &str) -> bool {
        matches!(
            self.get(path).map(|c| &c.change_type),
            Some(ChangeType::DeleteFile | ChangeType::DeleteDirectory)
        )
    }

    /// Iterate changes in order.
    pub fn iter(&self) -> impl Iterator<Item = &PendingChange> {
        self.order.iter().filter_map(|p| self.changes.get(p))
    }

    /// Count of pending changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Clear all pending changes.
    pub fn clear(&mut self) {
        self.changes.clear();
        self.order.clear();
    }

    /// Get summary of changes for display.
    pub fn summary(&self) -> ChangesSummary {
        let mut creates = 0;
        let mut updates = 0;
        let mut deletes = 0;

        for change in self.changes.values() {
            match &change.change_type {
                ChangeType::CreateFile { .. }
                | ChangeType::CreateBinaryFile { .. }
                | ChangeType::CreateDirectory { .. } => creates += 1,
                ChangeType::UpdateFile { .. } => updates += 1,
                ChangeType::DeleteFile | ChangeType::DeleteDirectory => deletes += 1,
            }
        }

        ChangesSummary {
            creates,
            updates,
            deletes,
        }
    }

    /// Get all paths with pending changes.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.order.iter().map(|s| s.as_str())
    }

    /// Get binary content as data URL if path is a pending binary file.
    /// Returns None if path doesn't exist or isn't a binary file.
    pub fn get_binary_data_url(&self, path: &str) -> Option<String> {
        match self.get(path).map(|c| &c.change_type) {
            Some(ChangeType::CreateBinaryFile {
                content_base64,
                mime_type,
                ..
            }) => Some(format!("data:{};base64,{}", mime_type, content_base64)),
            _ => None,
        }
    }

    /// Get all pending binary files as a map of path -> data URL.
    pub fn get_all_binary_data_urls(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for change in self.changes.values() {
            if let ChangeType::CreateBinaryFile {
                content_base64,
                mime_type,
                ..
            } = &change.change_type
            {
                let data_url = format!("data:{};base64,{}", mime_type, content_base64);
                result.insert(change.path.clone(), data_url);
            }
        }
        result
    }
}

/// Summary of pending changes.
#[derive(Clone, Debug, Default)]
pub struct ChangesSummary {
    pub creates: usize,
    pub updates: usize,
    pub deletes: usize,
}

impl ChangesSummary {
    /// Total number of changes.
    pub fn total(&self) -> usize {
        self.creates + self.updates + self.deletes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pending_changes_add_and_get() {
        let mut changes = PendingChanges::new();
        changes.add(
            "test.md".to_string(),
            ChangeType::CreateFile {
                content: "Hello".to_string(),
                description: "Test file".to_string(),
                meta: FileMetadata::default(),
            },
        );

        assert!(changes.has_change("test.md"));
        assert!(!changes.has_change("other.md"));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_pending_changes_remove() {
        let mut changes = PendingChanges::new();
        changes.add("test.md".to_string(), ChangeType::DeleteFile);
        changes.remove("test.md");

        assert!(!changes.has_change("test.md"));
        assert!(changes.is_empty());
    }

    #[test]
    fn test_pending_changes_is_deleted() {
        let mut changes = PendingChanges::new();
        changes.add("deleted.md".to_string(), ChangeType::DeleteFile);
        changes.add(
            "created.md".to_string(),
            ChangeType::CreateFile {
                content: String::new(),
                description: String::new(),
                meta: FileMetadata::default(),
            },
        );

        assert!(changes.is_deleted("deleted.md"));
        assert!(!changes.is_deleted("created.md"));
    }

    #[test]
    fn test_pending_changes_summary() {
        let mut changes = PendingChanges::new();
        changes.add(
            "new.md".to_string(),
            ChangeType::CreateFile {
                content: String::new(),
                description: String::new(),
                meta: FileMetadata::default(),
            },
        );
        changes.add(
            "updated.md".to_string(),
            ChangeType::UpdateFile {
                content: "new content".to_string(),
                description: None,
            },
        );
        changes.add("deleted.md".to_string(), ChangeType::DeleteFile);

        let summary = changes.summary();
        assert_eq!(summary.creates, 1);
        assert_eq!(summary.updates, 1);
        assert_eq!(summary.deletes, 1);
        assert_eq!(summary.total(), 3);
    }

    #[test]
    fn test_pending_changes_order_preserved() {
        let mut changes = PendingChanges::new();
        changes.add("first.md".to_string(), ChangeType::DeleteFile);
        changes.add("second.md".to_string(), ChangeType::DeleteFile);
        changes.add("third.md".to_string(), ChangeType::DeleteFile);

        let paths: Vec<_> = changes.paths().collect();
        assert_eq!(paths, vec!["first.md", "second.md", "third.md"]);
    }
}
