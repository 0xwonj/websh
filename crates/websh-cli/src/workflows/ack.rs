use std::path::Path;

use anyhow::{Context, anyhow, bail};
use websh_core::crypto::ack::{
    ACK_LOCAL_SOURCE_PATH, AckArtifact, AckEntryMode, AckPrivateSource, AckReceipt,
    AckReceiptVerification, AckSourceEntry, build_artifact_from_source, hash_hex,
    normalize_ack_name, private_receipt_from_source, verify_private_receipt,
};
use websh_site::ACK_ARTIFACT_PATH;

use crate::CliResult;
use crate::infra::json::{read_json, write_json};

/// A public checkout does not need the author's private acknowledgement source.
pub(crate) fn prepare(root: &Path) -> CliResult<Option<AckArtifact>> {
    let path = root.join(ACK_LOCAL_SOURCE_PATH);
    if !path
        .try_exists()
        .with_context(|| format!("inspect {}", path.display()))?
    {
        return Ok(None);
    }
    let source = read_json::<AckPrivateSource>(&path)?;
    Ok(Some(build_artifact_from_source(&source)?))
}

pub(crate) fn check(root: &Path) -> CliResult<AckArtifact> {
    let artifact = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    artifact.validate()?;
    if let Some(expected) = prepare(root)?
        && expected != artifact
    {
        bail!("ACK commitment differs from the private source; run websh-cli sync");
    }
    Ok(artifact)
}

pub(crate) fn add(root: &Path, name: String, mode: AckEntryMode) -> CliResult<AckArtifact> {
    let path = root.join(ACK_LOCAL_SOURCE_PATH);
    let mut source = if path
        .try_exists()
        .with_context(|| format!("inspect {}", path.display()))?
    {
        read_json::<AckPrivateSource>(&path)?
    } else {
        if root.join(ACK_ARTIFACT_PATH).try_exists()? {
            bail!(
                "private ACK source is missing; restore {} before editing the published commitment",
                path.display()
            );
        }
        AckPrivateSource::default()
    };
    source.entries.push(AckSourceEntry {
        mode,
        name,
        nonce: if mode == AckEntryMode::Private {
            let mut bytes = [0u8; 32];
            getrandom::fill(&mut bytes)?;
            Some(hash_hex(&bytes))
        } else {
            None
        },
    });
    save(root, &source)
}

pub(crate) fn remove(root: &Path, name: &str) -> CliResult<AckArtifact> {
    let mut source = read_json::<AckPrivateSource>(&root.join(ACK_LOCAL_SOURCE_PATH))?;
    let normalized = normalize_ack_name(name);
    let index = source
        .entries
        .iter()
        .position(|entry| normalize_ack_name(&entry.name) == normalized)
        .ok_or_else(|| anyhow!("ACK entry not found: {name}"))?;
    source.entries.remove(index);
    save(root, &source)
}

pub(crate) fn list(root: &Path) -> CliResult<Vec<AckSourceEntry>> {
    let source = read_json::<AckPrivateSource>(&root.join(ACK_LOCAL_SOURCE_PATH))?;
    build_artifact_from_source(&source)?;
    Ok(source.entries)
}

/// Export a proof of the current published commitment, never a cached receipt.
pub(crate) fn receipt(root: &Path, name: &str, out: &Path) -> CliResult<AckReceipt> {
    let artifact = check(root)?;
    let source = read_json::<AckPrivateSource>(&root.join(ACK_LOCAL_SOURCE_PATH))?;
    let receipt = private_receipt_from_source(&source, name)?;
    verify_private_receipt(&artifact, &receipt)?;
    write_json(out, &receipt)?;
    Ok(receipt)
}

pub(crate) fn verify(root: &Path, path: &Path) -> CliResult<AckReceiptVerification> {
    let artifact = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    let receipt = read_json::<AckReceipt>(path)?;
    Ok(verify_private_receipt(&artifact, &receipt)?)
}

fn save(root: &Path, source: &AckPrivateSource) -> CliResult<AckArtifact> {
    // Validate everything before publishing either file. The private source is
    // authoritative; sync can rebuild the commitment if its write later fails.
    let artifact = build_artifact_from_source(source)?;
    artifact.validate()?;
    write_json(&root.join(ACK_LOCAL_SOURCE_PATH), source)?;
    write_json(&root.join(ACK_ARTIFACT_PATH), &artifact)?;
    Ok(artifact)
}
