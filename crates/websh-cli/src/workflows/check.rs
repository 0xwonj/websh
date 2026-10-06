use std::fs;
use std::path::Path;

use anyhow::{Context, bail};

use super::{attest, sync::Prepared};
use crate::CliResult;

pub(crate) struct CheckOutcome {
    pub(crate) subjects: usize,
    pub(crate) signed: usize,
    pub(crate) root_signed: bool,
}

pub(crate) fn check(root: &Path, require_signatures: bool) -> CliResult<CheckOutcome> {
    let prepared = Prepared::load(root)?;
    prepared.check_outputs(root)?;
    let signed = attest::verify::verify_artifact(root, &prepared.artifact, false)?;
    let root_signed = root.join(super::sync::SIGNATURE_PATH).exists();
    if require_signatures || root_signed {
        super::release::verify(root)?;
    }
    Ok(CheckOutcome {
        subjects: prepared.artifact.subjects.len(),
        signed,
        root_signed,
    })
}

/// App deployment is independent of today's separately published content.
pub(crate) fn check_bundle(root: &Path) -> CliResult {
    let dist = root.join("dist");
    if dist.is_symlink() || !dist.is_dir() {
        bail!("dist must be a prebuilt directory, not a symlink");
    }
    reject_symlinks(&dist)?;
    if dist.join("content").exists() {
        bail!("dist contains bundled content; rebuild the independent app");
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
    let certificate = fs::read(dist.join(websh_site::PUBLIC_KEY_PATH))
        .context("read bundled owner certificate")?;
    if certificate != websh_site::PUBLIC_KEY_BLOCK.as_bytes() {
        bail!(
            "dist owner certificate differs from the pinned production identity; rebuild the production app"
        );
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
