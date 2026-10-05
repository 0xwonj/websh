use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::attestation::artifact::sha256_hex;
use websh_core::domain::{
    ContentManifestDocument, ContentManifestEntry, DerivedMetadata, MempoolFields, NodeKind,
    NodeMetadata,
};
use websh_core::mempool::LEDGER_CATEGORIES;

use crate::CliResult;
use crate::infra::json::write_json;

use super::draft::Draft;
use super::path::EntryPath;

/// Rebuild from canonical source files; an existing manifest is never an input.
pub(crate) fn sync(repo_dir: &Path) -> CliResult<usize> {
    if !repo_dir.is_dir() {
        bail!(
            "mempool checkout is not a directory: {}",
            repo_dir.display()
        );
    }
    let mut entries = Vec::new();
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
                continue;
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
            entries.push(
                build_entry(&entry_path, &body)
                    .with_context(|| format!("validate {}", path.display()))?,
            );
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let count = entries.len();
    let manifest_path = repo_dir.join("manifest.json");
    match fs::symlink_metadata(&manifest_path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => bail!(
            "manifest must be a regular file: {}",
            manifest_path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("inspect mempool manifest"),
    }
    write_json(&manifest_path, &ContentManifestDocument { entries })?;
    Ok(count)
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
                content_sha256: Some(sha256_hex(body.as_bytes())),
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
