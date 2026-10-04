use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::domain::{ContentManifestDocument, ContentManifestEntry};
use websh_core::mempool::{
    LEDGER_CATEGORIES, MempoolManifestState, build_mempool_manifest_state, mempool_root,
};

use crate::CliResult;
use crate::infra::json::write_json;

use super::path::MempoolEntryPath;

/// Rebuild from canonical source files; an existing manifest is never an input.
pub(crate) fn rebuild(repo_dir: &Path) -> CliResult<usize> {
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
            let entry_path = MempoolEntryPath::parse(&format!("{category}/{file_name}"))?;
            let body =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            entries.push(build_entry(&entry_path, &body));
        }
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    let count = entries.len();
    write_json(
        &repo_dir.join("manifest.json"),
        &ContentManifestDocument { entries },
    )?;
    Ok(count)
}

pub(super) fn build_entry(path: &MempoolEntryPath, body: &str) -> ContentManifestEntry {
    let canonical = mempool_root().join(path.as_str());
    let MempoolManifestState { meta, extensions } = build_mempool_manifest_state(body, &canonical);
    ContentManifestEntry {
        path: path.to_string(),
        metadata: meta,
        mempool: extensions.mempool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_dir;
    use websh_core::domain::MempoolStatus;

    #[test]
    fn rebuilds_current_manifest_from_sources_deterministically() {
        let temp = temp_dir("rebuild");
        fs::create_dir(temp.join("writing")).unwrap();
        fs::create_dir(temp.join("papers")).unwrap();
        fs::create_dir(temp.join("unrelated")).unwrap();
        let body = "---\ntitle: Test\nstatus: review\npriority: high\n---\n\nHello world.\n";
        fs::write(temp.join("writing/test.md"), body).unwrap();
        fs::write(temp.join("papers/first.md"), "# First\n").unwrap();
        fs::write(temp.join("README.md"), "# Repository\n").unwrap();
        fs::write(temp.join("unrelated/ignored.md"), "# Ignore\n").unwrap();
        fs::write(temp.join("manifest.json"), "not an input").unwrap();

        assert_eq!(rebuild(&temp).unwrap(), 2);
        let encoded = fs::read_to_string(temp.join("manifest.json")).unwrap();
        let manifest: ContentManifestDocument = serde_json::from_str(&encoded).unwrap();
        assert_eq!(manifest.entries[0].path, "papers/first.md");
        assert_eq!(
            manifest.entries[1].mempool.as_ref().unwrap().status,
            MempoolStatus::Review
        );
        assert_eq!(
            fs::read_to_string(temp.join("writing/test.md")).unwrap(),
            body
        );
        rebuild(&temp).unwrap();
        assert_eq!(
            fs::read_to_string(temp.join("manifest.json")).unwrap(),
            encoded
        );
    }

    #[test]
    fn invalid_source_does_not_replace_the_manifest() {
        let temp = temp_dir("invalid");
        fs::create_dir(temp.join("writing")).unwrap();
        fs::write(temp.join("writing/not a slug.md"), "# Invalid\n").unwrap();
        fs::write(temp.join("manifest.json"), "untouched").unwrap();
        assert!(rebuild(&temp).is_err());
        assert_eq!(
            fs::read_to_string(temp.join("manifest.json")).unwrap(),
            "untouched"
        );
    }
}
