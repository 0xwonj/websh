//! Explorer-related data types for the file browser UI.

use crate::domain::VirtualPath;

/// Main view mode (Terminal or Explorer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// Terminal view (default)
    #[default]
    Terminal,
    /// File explorer view
    Explorer,
}

/// View type for explorer (list or grid).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExplorerViewType {
    /// List view (default)
    #[default]
    List,
    /// Grid view
    Grid,
}

/// Selected item in the explorer (file or directory).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Absolute canonical path for the selected node.
    pub path: VirtualPath,
    /// Whether this is a directory.
    pub is_dir: bool,
}
