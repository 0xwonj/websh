use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::domain::{
    ContentManifestEntry, DerivedMetadata, Manifest, MempoolFields, NodeKind, NodeMetadata,
};
use websh_core::mempool::LEDGER_CATEGORIES;

use crate::CliResult;
use crate::infra::json::{json_bytes, write_bytes};

use super::draft::Draft;
use super::path::EntryPath;

/// Rebuild from canonical source files; an existing manifest is never an input.
pub(crate) fn prepare(repo_dir: &Path) -> CliResult<DraftSnapshot> {
    if !repo_dir.is_dir() {
        bail!(
            "mempool checkout is not a directory: {}",
            repo_dir.display()
        );
    }
    let mut entries = Vec::new();
    let mut files = BTreeMap::new();
    for category in LEDGER_CATEGORIES {
        let dir = repo_dir.join(category);
        if !dir.exists() {
            continue;
        }
        if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
            bail!("mempool category must not be a symlink: {}", dir.display());
        }
        for item in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
            let item = item?;
            let path = item.path();
            if path.extension().is_none_or(|extension| extension != "md") {
                bail!("unsupported file in draft category: {}", path.display());
            }
            if !item.file_type()?.is_file() {
                bail!("mempool entry must be a regular file: {}", path.display());
            }
            let file_name = item.file_name();
            let file_name = file_name
                .to_str()
                .context("mempool filename is not UTF-8")?;
            let entry_path = EntryPath::parse(&format!("{category}/{file_name}"))?;
            let body =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            crate::workflows::content::validate_public_bytes(entry_path.as_str(), body.as_bytes())?;
            files.insert(entry_path.to_string(), body.as_bytes().to_vec());
            entries.push(
                build_entry(&entry_path, &body)
                    .with_context(|| format!("validate {}", path.display()))?,
            );
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let count = entries.len();
    let manifest = Manifest {
        entries,
        release: None,
    };
    manifest.validate()?;
    files.insert("manifest.json".into(), json_bytes(&manifest)?);
    Ok(DraftSnapshot { files, count })
}

pub(crate) struct DraftSnapshot {
    pub(crate) files: BTreeMap<String, Vec<u8>>,
    count: usize,
}
impl DraftSnapshot {
    pub(crate) fn write(&self, root: &Path) -> CliResult {
        write_bytes(&root.join("manifest.json"), &self.files["manifest.json"])
    }
}

pub(crate) fn sync(repo_dir: &Path) -> CliResult<usize> {
    let snapshot = prepare(repo_dir)?;
    snapshot.write(repo_dir)?;
    Ok(snapshot.count)
}

pub(crate) fn is_public_path(path: &str) -> bool {
    path == "manifest.json"
        || path.split_once('/').is_some_and(|(category, file)| {
            LEDGER_CATEGORIES.contains(&category) && !file.contains('/') && file.ends_with(".md")
        })
}

fn build_entry(path: &EntryPath, body: &str) -> CliResult<ContentManifestEntry> {
    let draft = Draft::parse(body)?;
    let category = path
        .as_str()
        .split('/')
        .next()
        .expect("validated entry path");
    if draft
        .metadata
        .category
        .as_deref()
        .is_some_and(|value| value != category)
    {
        bail!("draft category must match its directory `{category}`");
    }
    Ok(ContentManifestEntry {
        path: path.to_string(),
        metadata: NodeMetadata {
            kind: NodeKind::Page,
            bundle: None,
            authored: draft.metadata.fields(),
            derived: DerivedMetadata {
                size_bytes: Some(body.len() as u64),
                content_sha256: Some(
                    websh_core::publication::ReleaseId::of(body.as_bytes())
                        .as_str()
                        .to_owned(),
                ),
                word_count: Some(
                    u32::try_from(draft.body.split_whitespace().count()).unwrap_or(u32::MAX),
                ),
                ..DerivedMetadata::default()
            },
        },
        mempool: Some(MempoolFields {
            status: draft.metadata.status,
            priority: draft.metadata.priority,
            category: Some(category.to_string()),
        }),
    })
}
