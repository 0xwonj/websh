//! Sign required page subjects before authenticating their containing root snapshot.
use super::sync::{MANIFEST_PATH, Prepared, SIGNATURE_PATH};
use crate::{
    CliResult,
    infra::{
        gpg,
        json::{json_bytes, write_bytes},
        time::unix_seconds,
    },
};
use anyhow::{Context, bail};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use websh_core::publication::{AcceptedRelease, Manifest, VerifiedRelease, verify_release};
use websh_site::EXPECTED_PGP_FINGERPRINT;

pub(crate) fn verify(root: &Path) -> CliResult<VerifiedRelease> {
    let manifest = fs::read(root.join(MANIFEST_PATH)).context("read manifest")?;
    let signature =
        fs::read(root.join(SIGNATURE_PATH)).context("read manifest.sig; run websh-cli sign")?;
    verify_bytes(&manifest, &signature)
}

pub(crate) fn verify_bytes(manifest: &[u8], signature: &[u8]) -> CliResult<VerifiedRelease> {
    Ok(verify_release(
        manifest,
        signature,
        &websh_site::content_trust(),
        unix_seconds(),
    )?)
}

/// Reuse an exact, already verified signed snapshot; issuance occurs only here.
pub(crate) fn sign(root: &Path) -> CliResult<Manifest> {
    sign_after(root, None)
}

pub(crate) fn sign_after(root: &Path, accepted: Option<&AcceptedRelease>) -> CliResult<Manifest> {
    let mut prepared = Prepared::load(root)?;
    super::attest::sign_subjects(&mut prepared, None)?;
    let mut manifest = prepared.manifest()?;
    let bytes = json_bytes(&manifest)?;
    if let Ok(signature) = fs::read(root.join(SIGNATURE_PATH))
        && verify_bytes(&bytes, &signature)
            .is_ok_and(|verified| accepted.is_none_or(|accepted| accepted.check(&verified).is_ok()))
    {
        prepared.ensure_current(root)?;
        prepared.publish(root)?;
        return Ok(manifest);
    }
    let metadata = manifest
        .release
        .as_mut()
        .context("root manifest must contain release metadata")?;
    metadata.sequence = metadata
        .sequence
        .max(accepted.map_or(0, |accepted| accepted.sequence))
        .checked_add(1)
        .context("publication sequence exhausted")?;
    metadata.issued_at = unix_seconds();
    let bytes = json_bytes(&manifest)?;
    let signature = gpg::sign(std::str::from_utf8(&bytes)?, EXPECTED_PGP_FINGERPRINT)?;
    verify_bytes(&bytes, signature.as_bytes()).context("verify new manifest signature")?;
    prepared.ensure_current(root)?;
    // All source checks and crypto validation complete before generated writes.
    prepared.publish(root)?;
    write_bytes(&root.join(SIGNATURE_PATH), signature.as_bytes())?;
    write_bytes(&root.join(MANIFEST_PATH), &bytes)?;
    Ok(manifest)
}

pub(crate) fn signed_files(root: &Path) -> CliResult<BTreeMap<String, Vec<u8>>> {
    let prepared = Prepared::load(root)?;
    prepared.check_outputs(root)?;
    super::attest::verify::verify_artifact(&prepared.artifact, true)?;
    verify(root)?;
    let mut files: BTreeMap<_, _> = prepared
        .files()?
        .into_iter()
        .map(|(path, bytes)| (format!("content/{path}"), bytes))
        .collect();
    files.insert(
        MANIFEST_PATH.to_owned(),
        fs::read(root.join(MANIFEST_PATH))?,
    );
    files.insert(
        SIGNATURE_PATH.to_owned(),
        fs::read(root.join(SIGNATURE_PATH))?,
    );
    Ok(files)
}

pub(crate) fn ensure_files(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> CliResult {
    for (path, expected) in files {
        let path_on_disk = root.join(path);
        if path_on_disk.is_symlink()
            || fs::read(&path_on_disk).with_context(|| format!("reread {path}"))? != *expected
        {
            bail!("{path} changed during publication; retry against the current sources");
        }
    }
    Ok(())
}
