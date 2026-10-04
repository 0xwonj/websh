//! Browser runtime services and adapter facades.
//!
//! UI features should call this module for browser-side runtime work instead
//! of reaching directly into core storage or runtime adapter internals.

pub(crate) mod content;
pub(crate) mod content_cache;
mod error;
pub(crate) mod github_backend;
pub(crate) mod loader;
pub(crate) mod mount_cache;
pub(crate) mod mount_refresh;
pub(crate) mod mounts;
pub(crate) mod state;
mod system;
pub(crate) mod wallet;

pub use error::{RuntimeError, RuntimeLoadError, RuntimeResult};
pub use loader::RuntimeLoad;
pub use mounts::{MountLoadSet, MountLoadStatus, MountScanResult};
pub use state::EnvironmentError;
pub use system::shell_execution_context;
