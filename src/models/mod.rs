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
mod wallet;

pub use explorer::{ExplorerViewType, Selection, ViewMode};
pub use filesystem::{
    DirectoryEntry, DirectoryMetadata, DisplayPermissions, FileMetadata, FileType, FsEntry,
    Manifest,
};
#[cfg(test)]
pub use filesystem::{EncryptionInfo, WrappedKey};
pub use mount::{Mount, MountRegistry};
pub use route::AppRoute;
pub use terminal::{ListFormat, OutputLine, OutputLineData, TextStyle};
pub use wallet::WalletState;
