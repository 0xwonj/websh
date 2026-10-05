use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::attestation::artifact::sha256_hex;
use websh_core::attestation::ledger::CONTENT_LEDGER_PATH;
use websh_site::{ACK_ARTIFACT_PATH, ATTESTATIONS_PATH, PUBLIC_KEY_PATH};

use super::{attest, sync::Prepared};
use crate::CliResult;

pub(crate) struct CheckOutcome {
    pub(crate) subjects: usize,
    pub(crate) signed: usize,
}

pub(crate) fn check(root: &Path, require_signatures: bool) -> CliResult<CheckOutcome> {
    let prepared = Prepared::load(root)?;
    prepared.check_outputs(root)?;
    let signed = attest::verify::verify_artifact(root, &prepared.artifact, require_signatures)?;
    Ok(CheckOutcome {
        subjects: prepared.artifact.subjects.len(),
        signed,
    })
}

/// Deploy only the bundle of the current verified project. Source-bound home
/// attestations are checked against the project, and copied content against dist.
pub(crate) fn check_bundle(root: &Path) -> CliResult {
    let dist = root.join("dist");
    if dist.is_symlink() || !dist.is_dir() {
        bail!("dist must be a prebuilt directory, not a symlink");
    }
    reject_symlinks(&dist)?;
    let prepared = Prepared::load(root)?;
    prepared.check_outputs(root)?;
    attest::verify::verify_artifact(root, &prepared.artifact, true)?;
    for path in [
        "content/manifest.json",
        CONTENT_LEDGER_PATH,
        ACK_ARTIFACT_PATH,
        ATTESTATIONS_PATH,
        PUBLIC_KEY_PATH,
    ] {
        let built = fs::read(dist.join(path)).with_context(|| format!("read bundled {path}"))?;
        let current = fs::read(root.join(path))?;
        if built != current {
            bail!("bundled {path} differs from the current project; rebuild dist");
        }
    }
    for entry in &prepared.content.manifest.entries {
        let Some(hash) = entry.metadata.derived.content_sha256.as_ref() else {
            continue;
        };
        let path = dist.join("content").join(&entry.path);
        let bytes =
            fs::read(&path).with_context(|| format!("read bundled content {}", path.display()))?;
        if sha256_hex(&bytes) != *hash {
            bail!("bundled content hash mismatch: {}", entry.path);
        }
    }
    // Authored metadata participates in signatures but is intentionally absent
    // from the runtime manifest; verify those copied files as well.
    for file in prepared
        .artifact
        .subjects
        .iter()
        .flat_map(|subject| subject.content_files())
    {
        if file.path.starts_with("content/") || file.path.starts_with("assets/") {
            let bytes = fs::read(dist.join(&file.path))
                .with_context(|| format!("read bundled signed file {}", file.path))?;
            if sha256_hex(&bytes) != file.sha256 || bytes.len() as u64 != file.bytes {
                bail!("bundled signed file mismatch: {}", file.path);
            }
        }
    }
    let index = fs::read_to_string(dist.join("index.html")).context("read bundled index.html")?;
    if !index.contains("<html") || !index.contains(".js") {
        bail!("dist has no built application entry point");
    }
    let mut wasm = false;
    let mut javascript = false;
    for entry in fs::read_dir(&dist)? {
        let path = entry?.path();
        match path.extension().and_then(|value| value.to_str()) {
            Some("wasm") => {
                let bytes = fs::read(&path)?;
                wasm |= bytes.starts_with(b"\0asm\x01\0\0\0") && bytes.len() > 8;
            }
            Some("js") => {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .context("bundle filename must be UTF-8")?;
                javascript |= index.contains(name) && fs::metadata(path)?.len() > 0;
            }
            _ => {}
        }
    }
    if !wasm || !javascript {
        bail!("dist must contain the built JavaScript and WebAssembly application");
    }
    Ok(())
}

fn reject_symlinks(root: &Path) -> CliResult {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!("bundle contains a symlink: {}", entry.path().display());
        }
        if kind.is_dir() {
            reject_symlinks(&entry.path())?;
        }
    }
    Ok(())
}
