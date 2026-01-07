//! File explorer UI components.
//!
//! Provides a graphical file browser interface as an alternative to the terminal.
//!
//! Components:
//! - [`Explorer`] - Main explorer view
//! - [`FileList`] - List view of files and directories
//! - [`PreviewPanel`] - Desktop side panel for file preview
//! - [`BottomSheet`] - Mobile bottom sheet for file preview

#[allow(clippy::module_inception)]
mod explorer;
mod file_list;
mod preview;
mod sheet;

pub use explorer::Explorer;
pub use file_list::FileList;
pub use preview::PreviewPanel;
pub use sheet::BottomSheet;
