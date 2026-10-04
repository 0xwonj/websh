use websh_core::domain::VirtualPath;
use websh_core::filesystem::MountError;
use websh_core::ports::StorageError;

#[derive(Debug, thiserror::Error)]
pub enum RuntimeLoadError {
    #[error("invalid root routes: {source}")]
    InvalidRoutes {
        source: websh_core::filesystem::RouteCatalogError,
    },
    #[error("mount {label}: {source}")]
    BootstrapMount {
        label: String,
        #[source]
        source: StorageError,
    },
    #[error("assemble global filesystem: {source}")]
    AssembleGlobalFs {
        #[source]
        source: MountError,
    },
    #[error("read {path}: {source}")]
    Read {
        path: VirtualPath,
        #[source]
        source: StorageError,
    },
    #[error("{path} outside {mount_root}")]
    PathOutsideMount {
        path: VirtualPath,
        mount_root: VirtualPath,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error(transparent)]
    RuntimeLoad(#[from] RuntimeLoadError),
    #[error("refresh failed: {message}")]
    RefreshFailed { message: String },
    #[error("no backend registered at mount root {mount_root}")]
    NoBackend { mount_root: VirtualPath },
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

pub type RuntimeResult<T = ()> = Result<T, RuntimeError>;
