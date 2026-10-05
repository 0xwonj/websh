use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use websh_core::domain::{GitHubMount, RuntimeMount, VirtualPath, validate_mount_root};
use websh_core::filesystem::{GlobalFs, Snapshot};
use websh_core::ports::StorageBackendRef;
use websh_core::runtime as core_runtime;
use websh_site::BOOTSTRAP_SITE;

use super::error::RuntimeLoadError;
use super::github_backend;
use super::mounts::{MountLoadSet, MountScanJob, MountScanResult};

type BackendRegistry = BTreeMap<VirtualPath, StorageBackendRef>;

#[derive(Clone)]
pub struct RuntimeLoad {
    pub snapshot: Snapshot,
    pub backends: BackendRegistry,
    pub total_files: usize,
    pub mounts: MountLoadSet,
}

fn bootstrap_runtime_mounts() -> Vec<RuntimeMount> {
    vec![core_runtime::bootstrap_runtime_mount(&BOOTSTRAP_SITE)]
}

fn bootstrap_backends() -> BackendRegistry {
    let mut backends = BTreeMap::new();
    let mount = core_runtime::bootstrap_runtime_mount(&BOOTSTRAP_SITE);
    backends.insert(
        mount.root.clone(),
        github_backend::build_backend_for_bootstrap_site(&BOOTSTRAP_SITE),
    );
    backends
}

pub fn bootstrap_runtime_load() -> RuntimeLoad {
    let global_fs = core_runtime::bootstrap_global_fs();
    let total_files = count_files(&global_fs, &VirtualPath::root());
    let mut mounts = MountLoadSet::empty();
    for mount in bootstrap_runtime_mounts() {
        mounts.insert_declared_loading(mount);
    }
    RuntimeLoad {
        snapshot: Snapshot::new(global_fs).expect("bootstrap has valid routes"),
        backends: bootstrap_backends(),
        total_files,
        mounts,
    }
}

pub async fn load_runtime() -> Result<RuntimeLoad, RuntimeLoadError> {
    let mut backends = bootstrap_backends();
    let roots: Vec<_> = backends.keys().cloned().collect();
    let mut scans = Vec::new();

    for root in roots {
        let Some(backend) = backends.get(&root).cloned() else {
            continue;
        };
        // The bootstrap site backend is not best-effort: if it can't scan
        // the local manifest, the app has no usable filesystem at all.
        let scan = backend
            .scan()
            .await
            .map_err(|source| RuntimeLoadError::BootstrapMount {
                label: mount_label_for_root(&root),
                source,
            })?;
        scans.push((root, scan));
    }

    let mut global_fs = core_runtime::assemble_global_fs(&scans)
        .map_err(|source| RuntimeLoadError::AssembleGlobalFs { source })?;
    let root_total_files = count_files(&global_fs, &VirtualPath::root());
    let mut mounts = MountLoadSet::empty();
    for mount in bootstrap_runtime_mounts() {
        mounts.insert_loaded(mount, root_total_files);
    }
    load_external_mounts(&mut global_fs, &mut backends, &mut mounts).await?;
    let total_files = count_files(&global_fs, &VirtualPath::root());
    let snapshot =
        Snapshot::new(global_fs).map_err(|source| RuntimeLoadError::InvalidRoutes { source })?;

    Ok(RuntimeLoad {
        snapshot,
        backends,
        total_files,
        mounts,
    })
}

pub async fn scan_mount(job: MountScanJob) -> MountScanResult {
    let scan = job.backend.scan().await;
    MountScanResult {
        mount: job.mount,
        backend: job.backend,
        epoch: job.epoch,
        scan,
    }
}

async fn load_external_mounts(
    global: &mut GlobalFs,
    backends: &mut BackendRegistry,
    mounts: &mut MountLoadSet,
) -> Result<(), RuntimeLoadError> {
    let bootstrap_roots = bootstrap_runtime_mounts()
        .into_iter()
        .map(|mount| mount.root)
        .collect::<Vec<_>>();
    register_external_mounts(
        global,
        backends,
        mounts,
        load_mount_declarations(global, backends).await?,
        &bootstrap_roots,
    );

    Ok(())
}

struct ExternalMountCandidate {
    mount: RuntimeMount,
    backend: Option<StorageBackendRef>,
    build_error: Option<String>,
    descriptor: Option<super::mount_cache::CacheDescriptor>,
}

struct FailedMountDeclaration {
    mount: RuntimeMount,
    error: String,
}

enum LoadedMountDeclaration {
    Parsed(GitHubMount),
    Failed(FailedMountDeclaration),
}

fn register_external_mounts(
    global: &mut GlobalFs,
    backends: &mut BackendRegistry,
    mounts: &mut MountLoadSet,
    declarations: Vec<LoadedMountDeclaration>,
    bootstrap_roots: &[VirtualPath],
) {
    let candidates = external_mount_candidates(declarations, bootstrap_roots);
    let mut seen_roots = BTreeSet::new();

    for candidate in candidates {
        if !seen_roots.insert(candidate.mount.root.clone()) {
            let error = format!(
                "duplicate mount root {}; first declaration kept",
                candidate.mount.root
            );
            mounts.reject(candidate.mount, error);
            continue;
        }

        if let Some(error) = candidate.build_error {
            mounts.insert_failed(candidate.mount, error);
            continue;
        }

        let Some(backend) = candidate.backend else {
            continue;
        };
        if let Err(error) = global.reserve_mount_point(candidate.mount.root.clone()) {
            mounts.insert_failed(candidate.mount, error.to_string());
            continue;
        }

        backends.insert(candidate.mount.root.clone(), backend.clone());
        let root = candidate.mount.root.clone();
        mounts.insert_loading(candidate.mount, backend);
        mounts.set_cache_descriptor(&root, candidate.descriptor);
    }

    reserve_failed_mount_points(global, mounts);
}

fn external_mount_candidates(
    declarations: Vec<LoadedMountDeclaration>,
    bootstrap_roots: &[VirtualPath],
) -> Vec<ExternalMountCandidate> {
    let mut out = Vec::new();
    for declaration in declarations {
        match declaration {
            LoadedMountDeclaration::Parsed(declaration) => {
                if bootstrap_roots
                    .iter()
                    .any(|root| root == declaration.mount_at())
                {
                    continue;
                }
                let (mount, backend, descriptor) =
                    github_backend::build_backend_for_declaration(declaration);
                out.push(ExternalMountCandidate {
                    mount,
                    backend: Some(backend),
                    build_error: None,
                    descriptor,
                });
            }
            LoadedMountDeclaration::Failed(failed) => {
                if bootstrap_roots
                    .iter()
                    .any(|root| root == &failed.mount.root)
                {
                    continue;
                }
                out.push(ExternalMountCandidate {
                    mount: failed.mount,
                    backend: None,
                    build_error: Some(failed.error),
                    descriptor: None,
                });
            }
        }
    }
    out
}

fn reserve_failed_mount_points(global: &mut GlobalFs, mounts: &MountLoadSet) {
    let mut roots = mounts.failed_roots_under(&VirtualPath::root());
    roots.sort_by_key(|root| root.as_str().len());
    for root in roots {
        let _ = global.reserve_mount_point(root);
    }
}

fn mount_label_for_root(root: &VirtualPath) -> String {
    if root.is_root() {
        "~".to_string()
    } else {
        root.file_name()
            .map(str::to_string)
            .unwrap_or_else(|| root.as_str().to_string())
    }
}

async fn load_mount_declarations(
    global: &GlobalFs,
    backends: &BackendRegistry,
) -> Result<Vec<LoadedMountDeclaration>, RuntimeLoadError> {
    let site_root = BOOTSTRAP_SITE.mount_root();
    let mounts_root = VirtualPath::from_absolute("/.websh/mounts").expect("constant path");
    let Some(site_backend) = backends.get(&site_root) else {
        return Ok(Vec::new());
    };
    if !global.is_directory(&mounts_root) {
        return Ok(Vec::new());
    }

    let mut declarations = Vec::new();
    for entry in global.list_dir(&mounts_root).unwrap_or_default() {
        if entry.is_dir || !entry.name.ends_with(".mount.json") {
            continue;
        }

        let body = read_backend_text(site_backend, &site_root, &entry.path).await?;
        match serde_json::from_str::<GitHubMount>(&body) {
            Ok(declaration) => declarations.push(LoadedMountDeclaration::Parsed(declaration)),
            Err(source) => {
                if let Some(failed) = recover_failed_mount_declaration(&entry.path, &body, &source)
                {
                    declarations.push(LoadedMountDeclaration::Failed(failed));
                } else {
                    leptos::logging::warn!(
                        "runtime: ignoring mount declaration {}: {source}",
                        entry.path.as_str()
                    );
                }
            }
        }
    }

    Ok(declarations)
}

fn recover_failed_mount_declaration(
    path: &VirtualPath,
    body: &str,
    source: &serde_json::Error,
) -> Option<FailedMountDeclaration> {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(error) => {
            leptos::logging::warn!(
                "runtime: ignoring malformed mount declaration {}: {error}",
                path.as_str()
            );
            return None;
        }
    };

    let mount_at = value.get("mount_at").and_then(Value::as_str)?;
    let mount_root = match VirtualPath::from_absolute(mount_at.to_string()) {
        Ok(root) => root,
        Err(error) => {
            leptos::logging::warn!(
                "runtime: ignoring mount declaration {} with invalid mount_at `{mount_at}`: {error}",
                path.as_str()
            );
            return None;
        }
    };
    validate_mount_root(&mount_root).ok()?;
    let label = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| mount_label_for_root(&mount_root));
    Some(FailedMountDeclaration {
        mount: RuntimeMount::new(mount_root, label),
        error: format!("parse {}: {source}", path.as_str()),
    })
}

async fn read_backend_text(
    backend: &StorageBackendRef,
    mount_root: &VirtualPath,
    path: &VirtualPath,
) -> Result<String, RuntimeLoadError> {
    let rel_path =
        path.strip_prefix(mount_root)
            .ok_or_else(|| RuntimeLoadError::PathOutsideMount {
                path: path.clone(),
                mount_root: mount_root.clone(),
            })?;
    backend
        .read_text(rel_path)
        .await
        .map_err(|source| RuntimeLoadError::Read {
            path: path.clone(),
            source,
        })
}

fn collect_file_paths(global: &GlobalFs, root: &VirtualPath) -> Vec<VirtualPath> {
    let mut out = Vec::new();
    collect_file_paths_recursive(global, root, &mut out);
    out
}

fn collect_file_paths_recursive(global: &GlobalFs, path: &VirtualPath, out: &mut Vec<VirtualPath>) {
    let Some(entry) = global.get_entry(path) else {
        return;
    };
    if !entry.is_directory() {
        out.push(path.clone());
        return;
    }

    for child in global.list_dir(path).unwrap_or_default() {
        collect_file_paths_recursive(global, &child.path, out);
    }
}

fn count_files(global: &GlobalFs, root: &VirtualPath) -> usize {
    collect_file_paths(global, root).len()
}

#[cfg(test)]
mod tests {
    use super::super::MountLoadStatus;
    use super::*;
    use wasm_bindgen_test::*;

    fn declaration(mount_at: &str, name: &str) -> GitHubMount {
        serde_json::from_value(serde_json::json!({
            "backend": "github", "mount_at": mount_at, "repo": "0xwonj/websh-test",
            "root": "content", "name": name
        }))
        .unwrap()
    }

    #[wasm_bindgen_test]
    fn duplicate_mount_is_reported_without_replacing_the_first() {
        let mut global = GlobalFs::empty();
        let mut backends = BTreeMap::new();
        let mut mounts = MountLoadSet::empty();
        let declarations = vec![
            LoadedMountDeclaration::Parsed(declaration("/db", "db")),
            LoadedMountDeclaration::Parsed(declaration("/db", "db-duplicate")),
        ];

        register_external_mounts(
            &mut global,
            &mut backends,
            &mut mounts,
            declarations,
            &[VirtualPath::root()],
        );

        let db = VirtualPath::from_absolute("/db").expect("db");
        assert!(matches!(mounts.status(&db), Some(MountLoadStatus::Loading)));
        assert_eq!(mounts.scan_jobs.len(), 1);
        assert!(global.is_directory(&db));

        let failures = mounts.failed_entries();
        assert_eq!(failures.len(), 1);
        assert!(failures.iter().any(|entry| entry.declared.root == db));
    }

    #[wasm_bindgen_test]
    fn invalid_declarations_remain_visible_without_creating_backends() {
        let path = VirtualPath::from_absolute("/.websh/mounts/db.mount.json").unwrap();
        let valid = serde_json::json!({
            "backend": "github", "mount_at": "/db", "repo": "0xwonj/db"
        });
        for (field, value, message) in [
            ("branch", serde_json::json!(123), "invalid type"),
            ("backend", serde_json::json!("unknown"), "unknown variant"),
        ] {
            let mut input = valid.clone();
            input[field] = value;
            let body = input.to_string();
            let source = serde_json::from_str::<GitHubMount>(&body).unwrap_err();
            let failed = recover_failed_mount_declaration(&path, &body, &source)
                .expect("invalid declaration with a public root remains visible");
            assert_eq!(failed.mount.label, "db");

            let mut global = GlobalFs::empty();
            let mut backends = BTreeMap::new();
            let mut mounts = MountLoadSet::empty();
            register_external_mounts(
                &mut global,
                &mut backends,
                &mut mounts,
                vec![LoadedMountDeclaration::Failed(failed)],
                &[VirtualPath::root()],
            );
            let db = VirtualPath::from_absolute("/db").unwrap();
            assert!(matches!(
                mounts.status(&db),
                Some(MountLoadStatus::Failed { ref error, .. }) if error.contains(message)
            ));
            assert!(global.is_directory(&db));
            assert!(backends.is_empty());
        }
    }
}
