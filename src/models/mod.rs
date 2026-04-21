//! Data models and types for the application.
//!
//! Contains domain types for:
//! - [`FsEntry`], [`FileMetadata`], [`FileType`] - Virtual filesystem representation
//! - [`OutputLine`] - Terminal output types
//! - [`WalletState`] - Web3 wallet connection state
//! - [`AppRoute`], [`Mount`], [`MountRegistry`] - Hash-based navigation for IPFS compatibility
//! - [`ViewMode`], [`ExplorerViewType`], [`SheetState`] - View management

mod explorer;
mod filesystem;
mod mount;
mod route;
mod terminal;
mod virtual_path;
mod wallet;

pub use explorer::{ExplorerViewType, Selection, ViewMode};
pub use filesystem::{
    AccessFilter, DirectoryEntry, DirectoryMetadata, DisplayPermissions, FileEntry, FileMetadata,
    FileType, FsEntry, Manifest, Recipient,
};
pub use mount::{Mount, MountRegistry};
pub use route::AppRoute;
pub use terminal::{ListFormat, OutputLine, OutputLineData, OutputLineId, TextStyle};
pub use virtual_path::VirtualPath;
pub use wallet::WalletState;
