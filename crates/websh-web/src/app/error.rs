use websh_core::domain::VirtualPath;

use crate::render::theme;
use crate::runtime::{EnvironmentError, RuntimeLoadError, WalletError};

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("unknown theme `{raw}`. available: {available}")]
    Unknown { raw: String, available: String },
    #[error("failed to persist {theme_id}: {source}")]
    Persist {
        theme_id: &'static str,
        #[source]
        source: EnvironmentError,
    },
}

impl ThemeError {
    pub fn unknown(raw: &str) -> Self {
        Self::Unknown {
            raw: raw.to_string(),
            available: theme::theme_ids().collect::<Vec<_>>().join(", "),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeServiceError {
    #[error(transparent)]
    Theme(#[from] ThemeError),
    #[error(transparent)]
    RuntimeLoad(#[from] RuntimeLoadError),
    #[error("refresh failed: {message}")]
    RefreshFailed { message: String },
    #[error("no backend registered at mount root {mount_root}")]
    NoBackend { mount_root: VirtualPath },
    #[error(transparent)]
    Environment(#[from] EnvironmentError),
    #[error(transparent)]
    Wallet(#[from] WalletError),
    #[error("no runtime mount declared at {root}")]
    MissingDeclaration { root: VirtualPath },
    #[error("invalid mounted routes: {source}")]
    InvalidRoutes {
        source: websh_core::filesystem::RouteCatalogError,
    },
    #[error("mount {label}: {source}")]
    ReplaceScannedSubtree {
        label: String,
        #[source]
        source: websh_core::filesystem::MountError,
    },
}

pub type RuntimeServiceResult<T = ()> = Result<T, RuntimeServiceError>;
