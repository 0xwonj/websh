use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::Value;

use crate::config::BOOTSTRAP_SITE;
use crate::core::engine::{BackendRegistry, GlobalFs};
use crate::core::storage::{ScannedSubtree, StorageBackend, boot as storage_boot};
use crate::models::{
    DerivedIndex, DirectorySidecarMetadata, FileSidecarMetadata, MountDeclaration, RuntimeMount,
    VirtualPath,
};

#[derive(Clone)]
pub struct RuntimeLoad {
    pub global_fs: GlobalFs,
    pub backends: BackendRegistry,
    pub runtime_mounts: Vec<RuntimeMount>,
    pub remote_heads: BTreeMap<VirtualPath, String>,
    pub total_files: usize,
}

fn bootstrap_runtime_mounts() -> Vec<RuntimeMount> {
    vec![storage_boot::bootstrap_runtime_mount()]
}

fn bootstrap_backends() -> BackendRegistry {
    let mut backends = BTreeMap::new();
    let mount = storage_boot::bootstrap_runtime_mount();
    backends.insert(
        mount.root.clone(),
        storage_boot::build_backend_for_bootstrap_site(&BOOTSTRAP_SITE),
    );
    backends
}

pub fn bootstrap_runtime_load() -> RuntimeLoad {
    let global_fs = storage_boot::bootstrap_global_fs();
    let total_files = count_files(&global_fs, &VirtualPath::root());
    RuntimeLoad {
        global_fs,
        backends: bootstrap_backends(),
        runtime_mounts: bootstrap_runtime_mounts(),
        remote_heads: BTreeMap::new(),
        total_files,
    }
}

pub async fn load_runtime() -> Result<RuntimeLoad, String> {
    let mut backends = bootstrap_backends();
    let mut runtime_mounts = bootstrap_runtime_mounts();
    let roots: Vec<_> = backends.keys().cloned().collect();
    let mut scans = Vec::new();

    for root in roots {
        let Some(backend) = backends.get(&root).cloned() else {
            continue;
        };
        let scan = backend
            .scan()
            .await
            .map_err(|error| format!("mount {}: {error}", mount_label_for_root(&root)))?;
        scans.push((root, scan));
    }

    let mut global_fs = assemble_global_fs(&scans)
        .map_err(|error| format!("assemble global filesystem: {error:?}"))?;
    apply_runtime_conventions(&mut global_fs, &mut backends, &mut runtime_mounts).await?;
    let remote_heads = hydrate_remote_heads(&runtime_mounts).await;
    let total_files = count_files(&global_fs, &VirtualPath::root());

    Ok(RuntimeLoad {
        global_fs,
        backends,
        runtime_mounts,
        remote_heads,
        total_files,
    })
}

pub async fn reload_runtime() -> Result<RuntimeLoad, String> {
    load_runtime().await
}

async fn hydrate_remote_heads(runtime_mounts: &[RuntimeMount]) -> BTreeMap<VirtualPath, String> {
    let mut out = BTreeMap::new();

    for mount in runtime_mounts {
        if let Ok(Some(head)) = storage_boot::hydrate_remote_head(&mount.storage_id()).await {
            out.insert(mount.root.clone(), head);
        }
    }

    out
}

async fn apply_runtime_conventions(
    global: &mut GlobalFs,
    backends: &mut BackendRegistry,
    runtime_mounts: &mut Vec<RuntimeMount>,
) -> Result<(), String> {
    storage_boot::seed_bootstrap_routes(global);
    load_site_json_if_present(global, backends).await?;

    let bootstrap_roots = bootstrap_runtime_mounts()
        .into_iter()
        .map(|mount| mount.root)
        .collect::<Vec<_>>();
    let stale_roots = backends
        .keys()
        .filter(|root| {
            !bootstrap_roots
                .iter()
                .any(|bootstrap_root| bootstrap_root == *root)
        })
        .cloned()
        .collect::<Vec<_>>();
    for stale_root in stale_roots {
        backends.remove(&stale_root);
        global.remove_subtree(&stale_root);
    }

    runtime_mounts.retain(|mount| bootstrap_roots.iter().any(|root| root == &mount.root));

    for declaration in load_mount_declarations(global, backends).await? {
        let mount_root = VirtualPath::from_absolute(declaration.mount_at.clone())
            .map_err(|_| format!("invalid mount_at: {}", declaration.mount_at))?;
        if bootstrap_roots.iter().any(|root| root == &mount_root) {
            continue;
        }

        let Some((runtime_mount, backend)) =
            storage_boot::build_backend_for_declaration(&declaration)?
        else {
            continue;
        };
        let scan = backend
            .scan()
            .await
            .map_err(|error| format!("mount {}: {error}", runtime_mount.label))?;
        global
            .mount_scanned_subtree(runtime_mount.root.clone(), &scan)
            .map_err(|error| format!("mount {}: {error:?}", runtime_mount.label))?;
        backends.insert(runtime_mount.root.clone(), backend);
        runtime_mounts.push(runtime_mount);
    }

    runtime_mounts.sort_by(|left, right| left.root.cmp(&right.root));

    load_sidecar_metadata(global, backends).await?;
    load_route_index(global, backends).await?;
    storage_boot::seed_bootstrap_routes(global);
    Ok(())
}

fn assemble_global_fs(
    scans: &[(VirtualPath, ScannedSubtree)],
) -> Result<GlobalFs, crate::core::engine::MountError> {
    let mut global = GlobalFs::empty();
    for (mount_root, scan) in scans {
        global.mount_scanned_subtree(mount_root.clone(), scan)?;
    }
    Ok(global)
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

async fn load_site_json_if_present(
    global: &GlobalFs,
    backends: &BackendRegistry,
) -> Result<(), String> {
    let path = VirtualPath::from_absolute("/.websh/site.json").expect("constant path");
    if !global.exists(&path) {
        return Ok(());
    }

    let site_root = BOOTSTRAP_SITE.mount_root();
    let Some(site_backend) = backends.get(&site_root) else {
        return Ok(());
    };
    let body = read_backend_text(site_backend, &site_root, &path).await?;
    let _: Value =
        serde_json::from_str(&body).map_err(|error| format!("parse {}: {error}", path.as_str()))?;
    Ok(())
}

async fn load_mount_declarations(
    global: &GlobalFs,
    backends: &BackendRegistry,
) -> Result<Vec<MountDeclaration>, String> {
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
        let declaration: MountDeclaration = serde_json::from_str(&body)
            .map_err(|error| format!("parse {}: {error}", entry.path.as_str()))?;
        declarations.push(declaration);
    }

    Ok(declarations)
}

async fn load_sidecar_metadata(
    global: &mut GlobalFs,
    backends: &BackendRegistry,
) -> Result<(), String> {
    let mounts: Vec<_> = backends
        .iter()
        .map(|(root, backend)| (root.clone(), backend.clone()))
        .collect();
    let mount_roots = backends.keys().cloned().collect::<Vec<_>>();

    for (mount_root, backend) in mounts {
        let files = collect_file_paths_for_mount(global, &mount_root, &mount_roots);
        for file_path in files {
            let Some(file_name) = file_path.file_name() else {
                continue;
            };

            if file_name.ends_with(".meta.json") {
                let Some(target_path) = resolve_file_sidecar_target(global, &file_path) else {
                    continue;
                };
                let body = read_backend_text(&backend, &mount_root, &file_path).await?;
                let metadata: FileSidecarMetadata = serde_json::from_str(&body)
                    .map_err(|error| format!("parse {}: {error}", file_path.as_str()))?;
                global.set_node_metadata(target_path, metadata.into());
                continue;
            }

            if file_name == "_index.dir.json" {
                let Some(target_path) = file_path.parent() else {
                    continue;
                };
                let body = read_backend_text(&backend, &mount_root, &file_path).await?;
                let metadata: DirectorySidecarMetadata = serde_json::from_str(&body)
                    .map_err(|error| format!("parse {}: {error}", file_path.as_str()))?;
                global.set_node_metadata(target_path, metadata.into());
            }
        }
    }

    Ok(())
}

async fn load_route_index(global: &mut GlobalFs, backends: &BackendRegistry) -> Result<(), String> {
    let site_root = BOOTSTRAP_SITE.mount_root();
    let index_path = VirtualPath::from_absolute("/.websh/index.json").expect("constant path");
    let Some(site_backend) = backends.get(&site_root) else {
        global.replace_route_index(Vec::new());
        return Ok(());
    };
    if !global.exists(&index_path) {
        global.replace_route_index(Vec::new());
        return Ok(());
    }

    let body = read_backend_text(site_backend, &site_root, &index_path).await?;
    let index: DerivedIndex = serde_json::from_str(&body)
        .map_err(|error| format!("parse {}: {error}", index_path.as_str()))?;
    global.replace_route_index(index.routes);
    Ok(())
}

async fn read_backend_text(
    backend: &Arc<dyn StorageBackend>,
    mount_root: &VirtualPath,
    path: &VirtualPath,
) -> Result<String, String> {
    let rel_path = path
        .strip_prefix(mount_root)
        .ok_or_else(|| format!("{} outside {}", path.as_str(), mount_root.as_str()))?;
    backend
        .read_text(rel_path)
        .await
        .map_err(|error| format!("read {}: {error}", path.as_str()))
}

fn collect_file_paths(global: &GlobalFs, root: &VirtualPath) -> Vec<VirtualPath> {
    let mut out = Vec::new();
    collect_file_paths_recursive(global, root, &mut out);
    out
}

fn collect_file_paths_for_mount(
    global: &GlobalFs,
    root: &VirtualPath,
    mount_roots: &[VirtualPath],
) -> Vec<VirtualPath> {
    let mut out = Vec::new();
    collect_file_paths_for_mount_recursive(global, root, root, mount_roots, &mut out);
    out
}

fn collect_file_paths_for_mount_recursive(
    global: &GlobalFs,
    mount_root: &VirtualPath,
    path: &VirtualPath,
    mount_roots: &[VirtualPath],
    out: &mut Vec<VirtualPath>,
) {
    if path != mount_root
        && mount_roots
            .iter()
            .any(|root| root != mount_root && path.starts_with(root))
    {
        return;
    }

    let Some(entry) = global.get_entry(path) else {
        return;
    };
    if !entry.is_directory() {
        out.push(path.clone());
        return;
    }

    for child in global.list_dir(path).unwrap_or_default() {
        collect_file_paths_for_mount_recursive(global, mount_root, &child.path, mount_roots, out);
    }
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

fn resolve_file_sidecar_target(
    global: &GlobalFs,
    sidecar_path: &VirtualPath,
) -> Option<VirtualPath> {
    let file_name = sidecar_path.file_name()?;
    let sidecar_stem = file_name.strip_suffix(".meta.json")?;
    let parent = sidecar_path.parent()?;
    let mut candidates = global
        .list_dir(&parent)?
        .into_iter()
        .filter(|entry| {
            !entry.is_dir
                && entry.path != *sidecar_path
                && !entry.name.ends_with(".meta.json")
                && entry.name != "_index.dir.json"
                && (entry.name == sidecar_stem
                    || entry.name.starts_with(&format!("{sidecar_stem}.")))
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        left.name
            .len()
            .cmp(&right.name.len())
            .then_with(|| left.name.cmp(&right.name))
    });

    candidates.into_iter().next().map(|entry| entry.path)
}

fn count_files(global: &GlobalFs, root: &VirtualPath) -> usize {
    collect_file_paths(global, root).len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::storage::{ScannedDirectory, ScannedFile};
    use crate::models::{DirectoryMetadata, FileMetadata};

    #[test]
    fn assembles_global_fs_under_canonical_mount_roots() {
        let scan = ScannedSubtree {
            files: vec![ScannedFile {
                path: "index.md".to_string(),
                description: "index".to_string(),
                meta: FileMetadata::default(),
            }],
            directories: vec![ScannedDirectory {
                path: "".to_string(),
                meta: DirectoryMetadata {
                    title: "home".to_string(),
                    ..Default::default()
                },
            }],
        };

        let fs =
            assemble_global_fs(&[(VirtualPath::root(), scan)]).expect("global fs should assemble");

        assert!(
            fs.get_entry(&crate::models::VirtualPath::from_absolute("/index.md").unwrap())
                .is_some()
        );
    }

    #[test]
    fn resolve_file_sidecar_targets_matching_file() {
        let scan = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "about.md".to_string(),
                    description: "About".to_string(),
                    meta: FileMetadata::default(),
                },
                ScannedFile {
                    path: "about.meta.json".to_string(),
                    description: "meta".to_string(),
                    meta: FileMetadata::default(),
                },
            ],
            directories: vec![],
        };

        let fs =
            assemble_global_fs(&[(VirtualPath::root(), scan)]).expect("global fs should assemble");
        let target = resolve_file_sidecar_target(
            &fs,
            &VirtualPath::from_absolute("/about.meta.json").unwrap(),
        )
        .expect("target");

        assert_eq!(target.as_str(), "/about.md");
    }
}
