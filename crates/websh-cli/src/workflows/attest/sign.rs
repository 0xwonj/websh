use super::verify::verify_artifact;
use crate::workflows::sync::Prepared;
use crate::{
    CliResult,
    infra::{gpg, pgp, time::today_utc},
};
use anyhow::{Context, bail};
use std::path::Path;
use websh_core::attestation::artifact::{Attestation, message_sha256};
use websh_site::{EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_PATH};

pub(crate) fn sign(root: &Path, route: Option<&str>) -> CliResult<usize> {
    let mut prepared = Prepared::load(root)?;
    if let Some(route) = route
        && prepared.artifact.subject_for_route(route).is_none()
    {
        bail!("unknown attestation route {route}");
    }
    let signer = Some(websh_site::APP_NAME.to_owned());
    let date = today_utc();
    let mut signed = 0;
    for subject in &mut prepared.artifact.subjects {
        if route.is_some_and(|route| route != subject.route()) {
            continue;
        }
        if subject
            .attestations()
            .iter()
            .any(|attestation| matches!(attestation, Attestation::Pgp { .. }))
        {
            continue;
        }
        // Existing additional signatures bind the same date, so retain it.
        if subject.issued_at().is_none() {
            subject.envelope_mut().issued_at = Some(date.clone());
        }
        let message = subject.canonical_message()?;
        let signature = gpg::sign(&message, EXPECTED_PGP_FINGERPRINT)
            .with_context(|| format!("sign {}", subject.route()))?;
        let fingerprint = pgp::verify_signature(&signature, &message)?;
        if fingerprint != EXPECTED_PGP_FINGERPRINT {
            bail!("GPG signer does not match the site identity");
        }
        subject.attestations_mut().push(Attestation::Pgp {
            signer: signer.clone(),
            fingerprint,
            key_path: PUBLIC_KEY_PATH.to_string(),
            signature,
            signature_path: None,
            message_sha256: message_sha256(&message),
        });
        signed += 1;
    }
    verify_artifact(root, &prepared.artifact, route.is_none())?;
    prepared.ensure_current(root)?;
    prepared.publish(root)?;
    Ok(signed)
}
