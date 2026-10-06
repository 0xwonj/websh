use std::collections::BTreeMap;
use std::rc::Rc;

use websh_core::domain::{GitHubMount, VirtualPath};
use websh_core::filesystem::Snapshot;
use websh_core::ports::{StorageBackend, StorageBackendRef};
use websh_core::publication::{SourceSnapshot, VerifiedRelease};
use websh_core::runtime as core_runtime;
use websh_site::BOOTSTRAP_SITE;

use super::error::RuntimeLoadError;
use super::github_backend::{self, GitHubBackend};
use super::mounts::{MountLoadSet, MountScanJob, MountScanResult};

type BackendRegistry = BTreeMap<VirtualPath, StorageBackendRef>;

#[derive(Clone)]
pub struct RuntimeLoad {
    pub snapshot: Snapshot,
    pub backends: BackendRegistry,
    pub total_files: usize,
    pub mounts: MountLoadSet,
    pub release: Option<Rc<VerifiedRelease>>,
}

fn root_config() -> GitHubMount {
    GitHubMount::bootstrap(&BOOTSTRAP_SITE).expect("valid root source")
}

pub fn root_cache_descriptor() -> super::mount_cache::CacheDescriptor {
    GitHubBackend::new(root_config())
        .cache_descriptor()
        .expect("root cache descriptor")
}

pub fn root_error(error: impl std::fmt::Display) -> RuntimeLoadError {
    RuntimeLoadError::BootstrapMount {
        label: "~".into(),
        source: super::github_backend::rejected(error),
    }
}

pub fn bootstrap_runtime_load() -> RuntimeLoad {
    let mut mounts = MountLoadSet::empty();
    mounts.insert_declared_loading(core_runtime::bootstrap_runtime_mount(&BOOTSTRAP_SITE));
    RuntimeLoad {
        snapshot: Snapshot::new(core_runtime::bootstrap_global_fs()).expect("valid empty root"),
        backends: BTreeMap::new(),
        total_files: 0,
        mounts,
        release: None,
    }
}

pub async fn load_runtime(
    previous: Option<SourceSnapshot>,
) -> Result<RuntimeLoad, RuntimeLoadError> {
    let backend = Rc::new(GitHubBackend::refreshing(root_config(), previous));
    backend.scan().await.map_err(root_error)?;
    runtime_from_backend(backend)
}

pub fn restore_runtime(source: SourceSnapshot) -> Result<RuntimeLoad, RuntimeLoadError> {
    runtime_from_backend(Rc::new(
        GitHubBackend::from_snapshot(root_config(), source).map_err(root_error)?,
    ))
}

fn runtime_from_backend(backend: Rc<GitHubBackend>) -> Result<RuntimeLoad, RuntimeLoadError> {
    let loaded = backend
        .loaded()
        .ok_or_else(|| root_error("root unavailable"))?;
    let release = loaded
        .verified
        .clone()
        .ok_or_else(|| root_error("root verification missing"))?;
    let root = VirtualPath::root();
    let total_files = loaded.scan.files.len();
    let mut global = core_runtime::assemble_global_fs(&[(root.clone(), loaded.scan.clone())])
        .map_err(|source| RuntimeLoadError::AssembleGlobalFs { source })?;
    let mut backends: BackendRegistry = [(root, backend as StorageBackendRef)].into();
    let mut mounts = MountLoadSet::empty();
    mounts.insert_loaded(
        core_runtime::bootstrap_runtime_mount(&BOOTSTRAP_SITE),
        total_files,
    );
    for declaration in &release.release().mounts {
        let (mount, backend, descriptor) =
            github_backend::build_backend_for_declaration(declaration.clone());
        global
            .reserve_mount_point(mount.root.clone())
            .map_err(|source| RuntimeLoadError::AssembleGlobalFs { source })?;
        backends.insert(mount.root.clone(), backend.clone());
        let root = mount.root.clone();
        mounts.insert_loading(mount, backend);
        mounts.set_cache_descriptor(&root, descriptor);
    }
    Ok(RuntimeLoad {
        snapshot: Snapshot::new(global)
            .map_err(|source| RuntimeLoadError::InvalidRoutes { source })?,
        backends,
        total_files,
        mounts,
        release: Some(release),
    })
}

pub async fn scan_mount(mut job: MountScanJob) -> MountScanResult {
    if let Some(backend) = job.backend.fork_for_refresh() {
        job.backend = backend;
    }
    let scan = job.backend.scan().await;
    MountScanResult {
        mount: job.mount,
        backend: job.backend,
        epoch: job.epoch,
        scan,
    }
}

pub struct HistoricalSnapshot {
    pub commit: websh_core::publication::GitCommit,
    pub release: websh_core::publication::ReleaseId,
}

pub fn requested_snapshot() -> Result<Option<HistoricalSnapshot>, RuntimeLoadError> {
    let search = web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default();
    let query = web_sys::UrlSearchParams::new_with_str(&search)
        .map_err(|_| root_error("invalid snapshot query"))?;
    if !query.has("content") && !query.has("release") {
        return Ok(None);
    }
    if query.get_all("content").length() != 1 || query.get_all("release").length() != 1 {
        return Err(root_error(
            "historical viewing requires one content commit and one release digest",
        ));
    }
    Ok(Some(HistoricalSnapshot {
        commit: websh_core::publication::GitCommit::parse(query.get("content").unwrap_or_default())
            .map_err(root_error)?,
        release: websh_core::publication::ReleaseId::parse(
            query.get("release").unwrap_or_default(),
        )
        .map_err(root_error)?,
    }))
}

pub async fn load_historical(
    selection: HistoricalSnapshot,
) -> Result<RuntimeLoad, RuntimeLoadError> {
    let backend = Rc::new(
        GitHubBackend::at_commit(root_config(), selection.commit)
            .await
            .map_err(root_error)?,
    );
    let load = runtime_from_backend(backend)?;
    if load
        .release
        .as_ref()
        .is_none_or(|release| release.id() != &selection.release)
    {
        return Err(root_error("historical release digest mismatch"));
    }
    Ok(load)
}
