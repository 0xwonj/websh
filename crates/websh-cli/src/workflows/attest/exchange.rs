use super::verify::verify_artifact;
use crate::workflows::sync::Prepared;
use crate::{
    CliResult,
    infra::{pgp, time::today_utc},
};
use anyhow::{Context, bail};
use std::fs;
use std::path::Path;
use websh_core::attestation::artifact::{Attestation, message_sha256};
use websh_core::crypto::eth::verify_personal_sign;
use websh_site::{EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_PATH};

/// Exact canonical bytes for an external signer. Exporting never changes state.
pub(crate) fn message(root: &Path, route: &str) -> CliResult<String> {
    let prepared = Prepared::load(root)?;
    let mut subject = prepared
        .artifact
        .subject_for_route(route)
        .with_context(|| format!("unknown attestation route {route}"))?
        .clone();
    if subject.issued_at().is_none() {
        subject.set_issued_at(Some(today_utc()));
    }
    Ok(subject.canonical_message()?)
}

fn request(root: &Path, route: &str, path: &Path) -> CliResult<(Prepared, String)> {
    let message = fs::read_to_string(path)
        .with_context(|| format!("read signing request {}", path.display()))?;
    let issued_at = message
        .lines()
        .last()
        .and_then(|line| line.strip_prefix("issued_at="))
        .context("signing request has no issuance date")?;
    if issued_at.len() != 10
        || !issued_at.bytes().enumerate().all(|(i, byte)| {
            if i == 4 || i == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        bail!("signing request issuance date must be YYYY-MM-DD");
    }
    let mut prepared = Prepared::load(root)?;
    let subject = prepared
        .artifact
        .subject_for_route_mut(route)
        .with_context(|| format!("unknown attestation route {route}"))?;
    if let Some(existing) = subject.issued_at()
        && existing != issued_at
    {
        bail!("request date differs from retained signatures for {route}");
    }
    subject.set_issued_at(Some(issued_at.to_string()));
    if subject.canonical_message()? != message {
        bail!("signing request does not match current subject {route}; export a new message");
    }
    Ok((prepared, message))
}

pub(crate) fn import_pgp(
    root: &Path,
    route: &str,
    message_path: &Path,
    signature_path: &Path,
) -> CliResult {
    let (mut prepared, message) = request(root, route, message_path)?;
    let signature = fs::read_to_string(signature_path)
        .with_context(|| format!("read signature {}", signature_path.display()))?;
    let fingerprint = pgp::verify_signature(&signature, &message)?;
    if fingerprint != EXPECTED_PGP_FINGERPRINT {
        bail!("PGP signature does not match site identity");
    }
    let signer = Some(websh_site::APP_NAME.to_owned());
    let subject = prepared
        .artifact
        .subject_for_route_mut(route)
        .expect("validated route");
    subject
        .attestations_mut()
        .retain(|attestation| !matches!(attestation, Attestation::Pgp { .. }));
    subject.attestations_mut().push(Attestation::Pgp {
        signer,
        fingerprint,
        key_path: PUBLIC_KEY_PATH.to_string(),
        signature,
        signature_path: None,
        message_sha256: message_sha256(&message),
    });
    verify_artifact(&prepared.artifact, false)?;
    prepared.ensure_current(root)?;
    prepared.publish(root)
}

pub(crate) fn import_ethereum(
    root: &Path,
    route: &str,
    message_path: &Path,
    address: &str,
    signature: &str,
) -> CliResult {
    let (mut prepared, message) = request(root, route, message_path)?;
    let verification = verify_personal_sign(address, &message, signature)?;
    let subject = prepared
        .artifact
        .subject_for_route_mut(route)
        .expect("validated route");
    subject.attestations_mut().retain(|attestation| !matches!(attestation, Attestation::Ethereum { address: existing, .. } if existing.eq_ignore_ascii_case(address)));
    subject.attestations_mut().push(Attestation::Ethereum {
        scheme: "eip191-personal-sign".to_string(),
        signer: verification.expected_address.clone(),
        address: verification.expected_address,
        signature: signature.to_string(),
        recovered_address: verification.recovered_address,
        message_sha256: message_sha256(&message),
    });
    verify_artifact(&prepared.artifact, false)?;
    prepared.ensure_current(root)?;
    prepared.publish(root)
}
