//! File explorer UI components.
//!
//! Provides a graphical file browser interface as an alternative to the terminal.
//!
//! ## Structure
//!
//! - [`Explorer`] - Main explorer layout
//! - [`Header`] - Navigation and action buttons
//! - [`FileList`] - Directory listing
//! - [`PathBar`] - Bottom path bar (macOS Finder style)
//! - [`preview`] - File/directory preview (panel and sheet)

#[allow(clippy::module_inception)]
mod explorer;
mod file_list;
mod header;
mod pathbar;
mod preview;

pub use explorer::Explorer;
pub use file_list::FileList;
pub use header::Header;
