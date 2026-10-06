use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::attestation::artifact::{ContentFile, sha256_hex};
use websh_core::domain::{
    ContentManifestEntry, GitHubMount, NodeKind, NodeMetadata, VirtualPath,
    validate_bundle_metadata_with_targets,
};
use websh_core::ports::{ManifestSnapshotError, parse_manifest_snapshot};
use websh_core::publication::Manifest;

use super::metadata::{DirectoryDeclaration, SourceMetadata, directory_metadata, file_metadata};
use super::{DEFAULT_CONTENT_DIR, route_for_content_path};
use crate::CliResult;

/// A portable publication unit derived from authored grouping.
#[derive(Debug)]
pub(crate) struct ContentUnit {
    pub(crate) route: String,
    pub(crate) path: String,
    pub(crate) kind: NodeKind,
    pub(crate) files: Vec<ContentFile>,
}

/// All generated content projections computed from one immutable read of the
/// authored tree. Loading never writes source or generated files.
#[derive(Debug)]
pub(crate) struct ContentSnapshot {
    pub(crate) manifest: Manifest,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    pub(crate) units: Vec<ContentUnit>,
}

impl ContentSnapshot {
    pub(crate) fn load(root: &Path) -> CliResult<Self> {
        Self::read(root, None)
    }

    /// Validate a proposed source addition before the import workflow writes it.
    /// `path` is canonical and relative to content/.
    pub(crate) fn load_with_file(root: &Path, path: &str, bytes: &[u8]) -> CliResult<Self> {
        Self::read(root, Some((path, bytes)))
    }

    fn read(root: &Path, addition: Option<(&str, &[u8])>) -> CliResult<Self> {
        let content_root = root.join(DEFAULT_CONTENT_DIR);
        let mut sources = BTreeMap::new();
        let mut directories = BTreeSet::new();
        read_tree(&content_root, "", &mut sources, &mut directories)?;
        if let Some((path, bytes)) = addition {
            let virtual_path = VirtualPath::from_absolute(format!("/{path}"))?;
            if path.is_empty() || path == "manifest.json" || is_metadata(path) || is_system(path) {
                bail!("invalid primary content path: {path}");
            }
            if sources.contains_key(path) || directories.contains(path) {
                bail!("content path already exists: {path}");
            }
            let mut parent = virtual_path.parent();
            while let Some(directory) = parent {
                let relative = directory.as_str().trim_start_matches('/');
                if sources.contains_key(relative) {
                    bail!("content parent is a file: {relative}");
                }
                directories.insert(relative.to_string());
                parent = directory.parent();
            }
            sources.insert(path.to_string(), bytes.to_vec());
        }
        // Empty directories are not authored inputs unless they contain a
        // declaration. This keeps a clean Git checkout equivalent to a local
        // tree with leftover empty directories after content removal.
        directories.retain(|directory| {
            directory.is_empty() || sources.keys().any(|path| inside(path, directory))
        });
        validate_mounts(&sources)?;
        validate_sidecars(&sources)?;

        let mut nodes = BTreeMap::new();
        let mut groups = BTreeSet::new();
        for directory in &directories {
            let declaration_path = join(directory, "_index.dir.json");
            let declaration = sources
                .get(&declaration_path)
                .map(|bytes| DirectoryDeclaration::parse(bytes, &declaration_path))
                .transpose()?;
            if declaration
                .as_ref()
                .is_some_and(|declaration| declaration.group)
            {
                if directory.is_empty() || is_system(directory) {
                    bail!("directory cannot be grouped for publication: {directory}");
                }
                groups.insert(directory.clone());
            }
            nodes.insert(
                directory.clone(),
                directory_metadata(directory, declaration),
            );
        }
        for (path, bytes) in &sources {
            super::validate_public_bytes(path, bytes)?;
            let authored = if path.ends_with(".md") {
                let body = std::str::from_utf8(bytes)
                    .with_context(|| format!("Markdown {path} must be UTF-8"))?;
                super::frontmatter::parse_yaml_frontmatter(body)
                    .with_context(|| format!("parse frontmatter {path}"))?
                    .unwrap_or_default()
            } else {
                sources
                    .get(&format!("{path}.meta.json"))
                    .map(|bytes| {
                        serde_json::from_slice::<SourceMetadata>(bytes)
                            .with_context(|| format!("parse metadata for {path}"))
                    })
                    .transpose()?
                    .unwrap_or_default()
            };
            nodes.insert(path.clone(), file_metadata(path, bytes, authored)?);
        }
        validate_bundles(&nodes)?;
        let units = build_units(&sources, &nodes, &groups)?;
        update_child_counts(&mut nodes);
        let manifest = Manifest {
            release: None,
            entries: nodes
                .into_iter()
                .map(|(path, metadata)| ContentManifestEntry {
                    path,
                    metadata,
                    mempool: None,
                })
                .collect(),
        };
        parse_manifest_snapshot(&serde_json::to_string(&manifest)?)?;
        Ok(Self {
            manifest,
            files: sources,
            units,
        })
    }
}

fn read_tree(
    root: &Path,
    relative: &str,
    sources: &mut BTreeMap<String, Vec<u8>>,
    directories: &mut BTreeSet<String>,
) -> CliResult {
    let directory = root.join(relative);
    if fs::symlink_metadata(&directory)
        .with_context(|| format!("read content directory {}", directory.display()))?
        .file_type()
        .is_symlink()
    {
        bail!(
            "content directory must not be a symlink: {}",
            directory.display()
        );
    }
    directories.insert(relative.to_string());
    for entry in
        fs::read_dir(&directory).with_context(|| format!("read {}", directory.display()))?
    {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("content filename must be UTF-8"))?;
        if name == ".git" || name == ".env" || name.starts_with(".env.") {
            bail!("private or repository control file is not public content: {name}");
        }
        if matches!(name.as_str(), ".DS_Store" | ".gitkeep") {
            continue;
        }
        let path = join(relative, &name);
        let virtual_path = VirtualPath::from_absolute(format!("/{path}"))?;
        if websh_core::domain::is_runtime_overlay_path(&virtual_path) {
            bail!("content cannot occupy the runtime namespace: {path}");
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!("content symlinks are not supported: {path}");
        }
        if kind.is_dir() {
            read_tree(root, &path, sources, directories)?;
        } else if kind.is_file() {
            if matches!(
                path.as_str(),
                "manifest.json"
                    | "manifest.sig"
                    | ".websh/attestations.json"
                    | ".websh/ack.commitment.json"
            ) {
                continue;
            }
            sources.insert(
                path.clone(),
                fs::read(entry.path()).with_context(|| format!("read {path}"))?,
            );
        } else {
            bail!("unsupported content file type: {path}");
        }
    }
    Ok(())
}

fn validate_sidecars(sources: &BTreeMap<String, Vec<u8>>) -> CliResult {
    for (path, bytes) in sources {
        let Some(primary) = path.strip_suffix(".meta.json") else {
            continue;
        };
        if !sources.contains_key(primary) || is_metadata(primary) {
            bail!("metadata {path} requires its full-filename source {primary}");
        }
        if primary.ends_with(".md") {
            bail!("Markdown metadata belongs in frontmatter: {path}");
        }
        let _: SourceMetadata =
            serde_json::from_slice(bytes).with_context(|| format!("parse {path}"))?;
    }
    Ok(())
}

fn validate_bundles(nodes: &BTreeMap<String, NodeMetadata>) -> CliResult {
    for (path, metadata) in nodes {
        if let Some(bundle) = &metadata.bundle {
            validate_bundle_metadata_with_targets(path, bundle, |variant| {
                if !nodes.contains_key(&join(path, &variant.path)) {
                    return Err(ManifestSnapshotError::MissingBundleVariantTarget {
                        bundle_path: path.clone(),
                        variant_id: variant.id.clone(),
                        path: variant.path.clone(),
                    });
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

fn build_units(
    sources: &BTreeMap<String, Vec<u8>>,
    nodes: &BTreeMap<String, NodeMetadata>,
    groups: &BTreeSet<String>,
) -> CliResult<Vec<ContentUnit>> {
    let bundles: Vec<&str> = nodes
        .iter()
        .filter_map(|(path, metadata)| {
            (metadata.kind == NodeKind::Bundle && !is_system(path)).then_some(path.as_str())
        })
        .collect();
    let mut grouped = Vec::new();
    for (path, metadata) in nodes {
        if path.is_empty()
            || is_system(path)
            || metadata.kind != NodeKind::Directory
            || !groups.contains(path)
            || bundles.iter().any(|parent| inside(path, parent))
            || grouped.iter().any(|parent: &&str| inside(path, parent))
        {
            continue;
        }
        grouped.push(path.as_str());
    }
    let hashed: BTreeMap<_, _> = sources
        .iter()
        .filter(|(path, _)| !is_system(path))
        .map(|(path, bytes)| {
            (
                path.as_str(),
                ContentFile {
                    path: format!("content/{path}"),
                    sha256: sha256_hex(bytes),
                    bytes: bytes.len() as u64,
                },
            )
        })
        .collect();
    let mut units = Vec::new();
    for (path, metadata) in nodes {
        if path.is_empty() || is_system(path) || is_metadata(path) {
            continue;
        }
        let files: Vec<ContentFile> = if bundles.contains(&path.as_str()) {
            hashed
                .iter()
                .filter(|(file, _)| inside(file, path))
                .map(|(_, file)| file.clone())
                .collect()
        } else if grouped.contains(&path.as_str()) {
            hashed
                .iter()
                .filter(|(file, _)| {
                    inside(file, path) && !bundles.iter().any(|bundle| inside(file, bundle))
                })
                .map(|(_, file)| file.clone())
                .collect()
        } else {
            if metadata.kind.is_directory_like()
                || bundles
                    .iter()
                    .chain(grouped.iter())
                    .any(|parent| inside(path, parent))
            {
                continue;
            }
            let mut files = vec![hashed[path.as_str()].clone()];
            if let Some(sidecar) = hashed.get(format!("{path}.meta.json").as_str()) {
                files.push((*sidecar).clone());
            }
            files.sort_by(|a, b| a.path.cmp(&b.path));
            files
        };
        let route = route_for_content_path(path);
        units.push(ContentUnit {
            route,
            path: path.clone(),
            kind: metadata.kind,
            files,
        });
    }
    units.sort_by(|a, b| a.route.cmp(&b.route));
    Ok(units)
}

fn update_child_counts(nodes: &mut BTreeMap<String, NodeMetadata>) {
    let mut counts = BTreeMap::<String, u32>::new();
    for path in nodes.keys().filter(|path| !path.is_empty()) {
        let parent = path
            .rsplit_once('/')
            .map(|(parent, _)| parent)
            .unwrap_or("");
        *counts.entry(parent.to_string()).or_default() += 1;
    }
    for (path, metadata) in nodes {
        if metadata.kind.is_directory_like() {
            metadata.derived.child_count = Some(counts.get(path).copied().unwrap_or(0));
        }
    }
}

fn validate_mounts(sources: &BTreeMap<String, Vec<u8>>) -> CliResult {
    let mut roots = BTreeSet::<VirtualPath>::new();
    for (path, bytes) in sources {
        if !path.starts_with(".websh/mounts/") || !path.ends_with(".mount.json") {
            continue;
        }
        let mount: GitHubMount =
            serde_json::from_slice(bytes).with_context(|| format!("parse mount {path}"))?;
        if !roots.insert(mount.mount_at().clone()) {
            bail!("duplicate mount target: {}", mount.mount_at());
        }
    }
    Ok(())
}

fn is_metadata(path: &str) -> bool {
    path.ends_with(".meta.json") || path.rsplit('/').next() == Some("_index.dir.json")
}

fn is_system(path: &str) -> bool {
    path == ".websh" || path.starts_with(".websh/") || path == ".site" || path.starts_with(".site/")
}
fn inside(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}
fn join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}
