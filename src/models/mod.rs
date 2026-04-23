//! Data models and types for the application.
//!
//! Contains domain types for:
//! - [`FsEntry`], [`FileMetadata`], [`FileType`] - canonical filesystem representation
//! - Filesystem-first sidecar and index metadata for pages/apps/routes
//! - [`OutputLine`] - Terminal output types
//! - [`WalletState`] - Web3 wallet connection state
//! - [`BootstrapSiteSource`], [`RuntimeMount`] - runtime mount metadata
//! - [`ViewMode`], [`ExplorerViewType`], [`SheetState`] - View management

mod explorer;
mod filesystem;
mod mount;
mod site;
mod terminal;
mod virtual_path;
mod wallet;

pub use explorer::{ExplorerViewType, Selection, ViewMode};
pub use filesystem::{
    AccessFilter, DirEntry, DirectoryMetadata, DisplayPermissions, FileMetadata, FileType, FsEntry,
    Recipient,
};
pub use mount::{BootstrapSiteSource, RuntimeBackendKind, RuntimeMount};
pub use site::{
    DerivedIndex, DirectorySidecarMetadata, FileSidecarMetadata, LoadedNodeMetadata,
    MountDeclaration, NodeKind, RendererKind, RouteIndexEntry, TrustLevel,
};
pub use terminal::{ListFormat, OutputLine, OutputLineData, OutputLineId, TextStyle};
pub use virtual_path::VirtualPath;
pub use wallet::WalletState;
