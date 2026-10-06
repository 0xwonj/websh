use super::{mempool, release};
use crate::{
    CliResult,
    infra::{
        git::Repository,
        http,
        json::{json_bytes, write_bytes},
    },
};
use anyhow::{Context, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use websh_core::publication::{GitCommit, SourcePointer};

pub(crate) struct Publication {
    pub(crate) snapshot: String,
    pub(crate) changed: bool,
    pub(crate) visibility_warning: Option<String>,
}

pub(crate) fn publish(root: &Path, drafts: bool) -> CliResult<Publication> {
    let allowed = |path: &str| {
        if drafts {
            mempool::manifest::is_public_path(path)
        } else {
            path.starts_with("content/")
        }
    };
    if root.join("current.json").is_symlink() {
        bail!("current.json must not be a symlink");
    }
    let mut repo = Repository::open(root, |path| path == "current.json" || allowed(path))?;
    let files = if drafts {
        let snapshot = mempool::manifest::prepare(root)?;
        snapshot.write(root)?;
        snapshot.files
    } else if repo.has_pending() {
        // Retry the prepared signed bytes without issuing another signature,
        // including when an editor changes inputs after the initial Git check.
        release::signed_files(root)?
    } else {
        let accepted = repo
            .remote_file("current.json")?
            .map(|bytes| -> CliResult<_> {
                let pointer: SourcePointer =
                    serde_json::from_slice(&bytes).context("parse remote content pointer")?;
                let manifest = repo.read_at(pointer.commit.as_str(), super::sync::MANIFEST_PATH)?;
                let signature =
                    repo.read_at(pointer.commit.as_str(), super::sync::SIGNATURE_PATH)?;
                Ok(release::verify_bytes(&manifest, &signature)?.accepted())
            })
            .transpose()?;
        release::sign_after(root, accepted.as_ref())?;
        release::signed_files(root)?
    };
    let existing = fs::read(root.join("current.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<SourcePointer>(&bytes).ok());
    let existing = existing.filter(|pointer| repo.contains_commit(pointer.commit.as_str()));
    let unchanged = existing.as_ref().is_some_and(|pointer| {
        files.iter().all(|(path, bytes)| {
            repo.read_at(pointer.commit.as_str(), path)
                .is_ok_and(|prior| prior == *bytes)
        }) && same_inventory(&repo, pointer.commit.as_str(), &files, drafts)
    });
    if repo.has_pending() && !unchanged {
        bail!(
            "prepared publication differs from local inputs; set newer edits aside before retrying"
        );
    }
    let snapshot = if unchanged {
        existing
            .expect("checked existing")
            .commit
            .as_str()
            .to_owned()
    } else {
        let snapshot = repo.snapshot(&files, allowed)?;
        for (path, expected) in &files {
            if repo.read_at(&snapshot, path)? != *expected {
                bail!("frozen Git snapshot differs at {path}");
            }
        }
        let pointer = SourcePointer {
            commit: GitCommit::parse(&snapshot)?,
        };
        let bytes = json_bytes(&pointer)?;
        let pointer_commit = repo.pointer_commit(&snapshot, &bytes)?;
        release::ensure_files(root, &files)?;
        // Re-scan to catch newly created or removed inputs as well as byte edits.
        let current = if drafts {
            mempool::manifest::prepare(root)?.files
        } else {
            release::signed_files(root)?
        };
        if current != files {
            bail!("public input set changed during publication");
        }
        write_bytes(&root.join("current.json"), &bytes)?;
        repo.install(&pointer_commit)?;
        snapshot
    };
    repo.push()?;
    let visibility_warning = visibility(&repo,&snapshot,&files).err().map(|error| format!("Git push succeeded; public visibility is pending: {error:#}. Rerun publish to check without re-signing."));
    Ok(Publication {
        snapshot,
        changed: !unchanged,
        visibility_warning,
    })
}

fn same_inventory(
    repo: &Repository,
    commit: &str,
    files: &BTreeMap<String, Vec<u8>>,
    drafts: bool,
) -> bool {
    // A removed source changes the generated manifest even if every retained body matches.
    let manifest = if drafts {
        "manifest.json"
    } else {
        "content/manifest.json"
    };
    files.get(manifest).is_some_and(|bytes| {
        repo.read_at(commit, manifest)
            .is_ok_and(|prior| prior == *bytes)
    })
}

fn visibility(repo: &Repository, snapshot: &str, files: &BTreeMap<String, Vec<u8>>) -> CliResult {
    let base = repo
        .raw_base()
        .context("origin has no public GitHub raw URL")?;
    let pointer: SourcePointer = serde_json::from_slice(&http::get(&format!(
        "{base}/{}/current.json",
        repo.branch()
    ))?)?;
    if pointer.commit.as_str() != snapshot {
        bail!("GitHub raw pointer still selects a different snapshot");
    }
    for path in [
        "manifest.json",
        "content/manifest.json",
        "content/manifest.sig",
    ] {
        if let Some(expected) = files.get(path)
            && http::get(&format!("{base}/{snapshot}/{path}"))? != *expected
        {
            bail!("public {path} does not match the pushed snapshot");
        }
    }
    Ok(())
}
