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

/// View type for explorer (list or grid).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExplorerViewType {
    /// List view (default)
    #[default]
    List,
    /// Grid view
    Grid,
}

/// Bottom sheet state for file preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SheetState {
    /// Sheet is closed
    #[default]
    Closed,
    /// Preview mode (30-40% height)
    Preview,
    /// Full screen mode
    Expanded,
}
