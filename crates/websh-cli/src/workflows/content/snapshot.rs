use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::attestation::artifact::{ContentFile, sha256_hex, subject_id_for_route};
use websh_core::attestation::ledger::{
    CONTENT_LEDGER_CONTENT_PATH, ContentLedger, ContentLedgerCategory, ContentLedgerEntry,
    ContentLedgerInput, ContentLedgerSortKey,
};
use websh_core::domain::{
    ContentManifestDocument, ContentManifestEntry, GitHubMount, NodeKind, NodeMetadata,
    VirtualPath, validate_bundle_metadata_with_targets,
};
use websh_core::ports::{ManifestSnapshotError, parse_manifest_snapshot};
use websh_core::support::format::iso_date_prefix;

use super::metadata::{DirectoryDeclaration, SourceMetadata, directory_metadata, file_metadata};
use super::{DEFAULT_CONTENT_DIR, route_for_content_path};
use crate::CliResult;
use crate::infra::json::json_bytes;

/// A logical publication unit shared by ledger and attestation generation.
#[derive(Debug)]
pub(crate) struct ContentUnit {
    pub(crate) route: String,
    pub(crate) kind: NodeKind,
    pub(crate) files: Vec<ContentFile>,
}

/// All generated content projections computed from one immutable read of the
/// authored tree. Loading never writes source or generated files.
#[derive(Debug)]
pub(crate) struct ContentSnapshot {
    pub(crate) manifest: ContentManifestDocument,
    pub(crate) ledger: ContentLedger,
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
            if is_metadata(path) {
                continue;
            }
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
        let (ledger, units) = build_units(&sources, &nodes, &groups)?;

        // The ledger is a generated runtime file. Project its new bytes instead
        // of consulting yesterday's on-disk ledger.
        let ledger_bytes = json_bytes(&ledger)?;
        nodes.insert(
            CONTENT_LEDGER_CONTENT_PATH.to_string(),
            file_metadata(
                CONTENT_LEDGER_CONTENT_PATH,
                &ledger_bytes,
                SourceMetadata::default(),
            )?,
        );
        nodes
            .entry(".websh".to_string())
            .or_insert_with(|| directory_metadata(".websh", None));
        update_child_counts(&mut nodes);
        let manifest = ContentManifestDocument {
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
            ledger,
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
        if matches!(name.as_str(), ".git" | ".DS_Store" | ".gitkeep") {
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
            if path == "manifest.json" || path == CONTENT_LEDGER_CONTENT_PATH {
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
) -> CliResult<(ContentLedger, Vec<ContentUnit>)> {
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
    let mut inputs = Vec::new();
    let mut units = Vec::new();
    for (path, metadata) in nodes {
        if path.is_empty() || is_system(path) {
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
        let date = metadata
            .date()
            .map(|date| {
                iso_date_prefix(date)
                    .map(str::to_owned)
                    .with_context(|| format!("invalid publication date for {path}: {date}"))
            })
            .transpose()?;
        inputs.push(ContentLedgerInput::new(
            ContentLedgerSortKey::new(date, path.clone()),
            ContentLedgerEntry::new(
                subject_id_for_route(&route),
                route.clone(),
                path.clone(),
                ContentLedgerCategory::for_path(path),
                files.clone(),
            )?,
        ));
        units.push(ContentUnit {
            route,
            kind: metadata.kind,
            files,
        });
    }
    let ledger = ContentLedger::new(inputs)?;
    ledger.validate()?;
    units.sort_by(|a, b| a.route.cmp(&b.route));
    Ok((ledger, units))
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
    path.split('/').any(|part| part == ".websh")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;

    fn write(root: &Path, path: &str, body: &str) {
        let path = root.join("content").join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn metadata<'a>(snapshot: &'a ContentSnapshot, path: &str) -> &'a NodeMetadata {
        &snapshot
            .manifest
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .unwrap()
            .metadata
    }

    #[test]
    fn current_authored_inputs_define_metadata_without_generated_sidecars() {
        let root = temp_dir("content-snapshot");
        write(
            &root,
            "note.md",
            "---\ntitle: Old title\ntags: [old]\n---\nbody\n",
        );
        write(&root, "note.txt", "plain");
        write(
            &root,
            "note.txt.meta.json",
            r#"{"title":"Text","access":{"recipients":[{"address":"0xabc"}]}}"#,
        );
        let first = ContentSnapshot::load(&root).unwrap();
        assert_eq!(metadata(&first, "note.md").kind, NodeKind::Page);
        assert_eq!(metadata(&first, "note.txt").title(), Some("Text"));
        assert_eq!(metadata(&first, "note.txt").size_bytes(), Some(5));
        assert!(metadata(&first, "note.txt").access().is_some());
        assert!(!root.join("content/note.meta.json").exists());
        assert!(!root.join("content/manifest.json").exists());
        assert!(!root.join("content/.websh/ledger.json").exists());

        write(&root, "note.md", "body\n");
        let second = ContentSnapshot::load(&root).unwrap();
        assert_eq!(metadata(&second, "note.md").title(), Some("note"));
        assert_eq!(metadata(&second, "note.md").tags(), None);
        fs::create_dir(root.join("content/removed-directory")).unwrap();
        write(&root, "manifest.json", "stale generated output");
        write(&root, ".websh/ledger.json", "stale generated output");
        let rebuilt = ContentSnapshot::load(&root).unwrap();
        assert_eq!(
            json_bytes(&second.manifest).unwrap(),
            json_bytes(&rebuilt.manifest).unwrap()
        );
        assert_eq!(
            json_bytes(&second.ledger).unwrap(),
            json_bytes(&rebuilt.ledger).unwrap()
        );
        assert_eq!(
            metadata(&second, ".websh/ledger.json").content_sha256(),
            Some(sha256_hex(&json_bytes(&second.ledger).unwrap()).as_str())
        );
    }

    #[test]
    fn bundle_and_directory_units_share_the_ledger_source_inventory() {
        let root = temp_dir("content-unit-snapshot");
        write(
            &root,
            "writing/essay/_index.dir.json",
            r#"{"kind":"bundle","bundle":{"default_variant":{"strategy":"static","id":"en"},"variants":[{"id":"en","path":"en.md","label":"English"}]},"authored":{"title":"Essay","date":"2026-05-15"}}"#,
        );
        write(&root, "writing/essay/en.md", "english");
        write(&root, "writing/essay/cover.svg", "<svg></svg>");
        write(
            &root,
            ".site/_index.dir.json",
            r#"{"kind":"directory","group":true,"authored":{"title":"Site"}}"#,
        );
        write(&root, ".site/now.toml", "[[items]]\n");
        // Cosmetic directory metadata alone never changes publication boundaries.
        write(
            &root,
            "writing/_index.dir.json",
            r#"{"kind":"directory","authored":{"title":"Writing"}}"#,
        );
        let snapshot = ContentSnapshot::load(&root).unwrap();
        assert_eq!(snapshot.units.len(), 2);
        assert!(!snapshot.units.iter().any(|unit| unit.route == "/writing"));
        assert!(snapshot.units.iter().any(|unit| unit.route == "/.site"));
        let bundle = snapshot
            .units
            .iter()
            .find(|unit| unit.kind == NodeKind::Bundle)
            .unwrap();
        assert_eq!(bundle.route, "/writing/essay");
        assert_eq!(bundle.files.len(), 3);
        for unit in &snapshot.units {
            let entry = &snapshot
                .ledger
                .blocks
                .iter()
                .find(|block| block.entry.route == unit.route)
                .unwrap()
                .entry;
            assert_eq!(entry.content_files, unit.files);
        }
        assert!(
            ContentSnapshot::load_with_file(&root, "writing/essay.md", b"conflicting route")
                .is_err()
        );
        assert!(!root.join("content/writing/essay.md").exists());
        assert!(!root.join("content/manifest.json").exists());
    }

    #[test]
    fn invalid_authored_input_and_duplicate_mounts_do_not_write_artifacts() {
        let root = temp_dir("content-validation");
        write(&root, "a.txt", "source");
        write(&root, "a.txt.meta.json", r#"{"size_bytes":999}"#);
        assert!(
            format!("{:#}", ContentSnapshot::load(&root).unwrap_err())
                .contains("unknown field `size_bytes`")
        );
        assert_eq!(
            fs::read_to_string(root.join("content/a.txt.meta.json")).unwrap(),
            r#"{"size_bytes":999}"#
        );
        fs::remove_file(root.join("content/a.txt.meta.json")).unwrap();
        write(&root, ".websh/state/injected.md", "not content");
        assert!(
            format!("{:#}", ContentSnapshot::load(&root).unwrap_err())
                .contains("runtime namespace")
        );
        fs::remove_dir_all(root.join("content/.websh/state")).unwrap();
        let mount = r#"{"backend":"github","mount_at":"/mempool","repo":"owner/repo"}"#;
        write(&root, ".websh/mounts/a.mount.json", mount);
        write(&root, ".websh/mounts/b.mount.json", mount);
        assert!(
            ContentSnapshot::load(&root)
                .unwrap_err()
                .to_string()
                .contains("duplicate mount")
        );
        assert!(!root.join("content/manifest.json").exists());
        assert!(!root.join("content/.websh/ledger.json").exists());
    }
}
