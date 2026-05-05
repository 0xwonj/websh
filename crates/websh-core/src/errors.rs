//! Cross-context error facade.
//!
//! Public core APIs should expose typed errors at their boundary and leave
//! final wording to adapters such as the terminal UI or native CLI.

use thiserror::Error;

use crate::domain::VirtualPathParseError;
use crate::filesystem::{ContentReadError, FsMutationError, MountError};
use crate::mempool::ComposeError;
use crate::ports::StorageError;

pub type WebshResult<T> = Result<T, WebshError>;

#[derive(Debug, Error)]
pub enum WebshError {
    #[error(transparent)]
    Path(#[from] VirtualPathParseError),
    #[error("filesystem mount error: {0:?}")]
    Mount(MountError),
    #[error("filesystem mutation error: {0:?}")]
    FilesystemMutation(FsMutationError),
    #[error(transparent)]
    ContentRead(#[from] ContentReadError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("mempool compose error: {0:?}")]
    MempoolCompose(ComposeError),
}

impl From<MountError> for WebshError {
    fn from(error: MountError) -> Self {
        Self::Mount(error)
    }
}

impl From<FsMutationError> for WebshError {
    fn from(error: FsMutationError) -> Self {
        Self::FilesystemMutation(error)
    }
}

impl From<ComposeError> for WebshError {
    fn from(error: ComposeError) -> Self {
        Self::MempoolCompose(error)
    }
}
