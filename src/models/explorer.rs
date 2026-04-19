//! Explorer-related data types for the file browser UI.

/// Main view mode (Terminal or Explorer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    /// Terminal view (default)
    #[default]
    Terminal,
    /// File explorer view
    Explorer,
}

/// Reader view mode (Read, Write, or Split).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ReaderViewMode {
    /// Read-only rendered view (default)
    #[default]
    Read,
    /// Write/edit mode with textarea
    Write,
    /// Split view: editor on left, preview on right
    Split,
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
    /// Path relative to mount root.
    pub path: String,
    /// Whether this is a directory.
    pub is_dir: bool,
}
