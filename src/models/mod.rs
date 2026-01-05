//! Data models and types for the application.
//!
//! Contains domain types for:
//! - [`VirtualPath`], [`FsEntry`] - Virtual filesystem representation
//! - [`OutputLine`], [`ScreenMode`] - Terminal output and display modes
//! - [`WalletState`] - Web3 wallet connection state
//! - [`Route`] - Hash-based navigation for IPFS compatibility

mod filesystem;
mod route;
mod terminal;
mod wallet;

pub use filesystem::{
    DisplayPermissions, FileMetadata, FileType, FsEntry, ManifestEntry, VirtualPath,
};
#[cfg(test)]
pub use filesystem::{EncryptionInfo, WrappedKey};
pub use route::Route;
pub use terminal::{ListFormat, OutputLine, OutputLineData, ScreenMode, TextStyle};
pub use wallet::WalletState;
