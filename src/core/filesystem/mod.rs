//! Virtual filesystem module.
//!
//! Provides:
//! - [`VirtualFs`] - Immutable filesystem built from manifest
//! - [`MergedFs`] - Merged view with pending changes overlay
//! - [`FsState`] - Reactive state wrapper for Leptos
//! - [`DirEntry`] - Directory listing entry

mod entry;
mod merged;
mod state;
mod virtual_fs;

pub use entry::{DirEntry, sort_entries};
pub use merged::{MergedFs, create_directory_entry, create_file_entry};
pub use state::FsState;
pub use virtual_fs::VirtualFs;
