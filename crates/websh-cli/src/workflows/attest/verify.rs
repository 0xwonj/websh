use std::collections::BTreeSet;

use anyhow::{Context, bail};
use websh_core::attestation::artifact::{Attestation, AttestationArtifact, message_sha256};
use websh_core::crypto::eth::verify_personal_sign;
use websh_core::crypto::pgp::normalize_fingerprint;
use websh_site::{EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_PATH};

use crate::{CliResult, infra::time::unix_seconds};
use websh_core::publication::verify_subject_signature;

/// Verify retained evidence independently of whether its original source is
/// still current. Freshness is checked against the prepared snapshot.
pub(crate) fn verify_artifact(
    artifact: &AttestationArtifact,
    require_signatures: bool,
) -> CliResult<usize> {
    artifact.validate_header()?;
    let mut routes = BTreeSet::new();
    let mut signed = 0;
    for subject in &artifact.subjects {
        if !routes.insert(subject.route()) {
            bail!("duplicate attestation route {}", subject.route());
        }
        subject
            .validate()
            .with_context(|| format!("validate {}", subject.route()))?;
        let mut site_signed = false;
        if !subject.attestations().is_empty() {
            let message = subject.canonical_message()?;
            let hash = message_sha256(&message);
            for attestation in subject.attestations() {
                if attestation.message_sha256() != hash {
                    bail!(
                        "invalid attestation message binding for {}",
                        subject.route()
                    );
                }
                match attestation {
                    Attestation::Pgp {
                        fingerprint,
                        key_path,
                        ..
                    } => {
                        if key_path != PUBLIC_KEY_PATH
                            || normalize_fingerprint(fingerprint) != EXPECTED_PGP_FINGERPRINT
                        {
                            bail!(
                                "PGP attestation is not bound to the site identity for {}",
                                subject.route()
                            );
                        }
                        verify_subject_signature(
                            subject,
                            attestation,
                            &websh_site::pgp_policy(),
                            unix_seconds(),
                        )?;
                        site_signed = true;
                    }
                    Attestation::Ethereum {
                        signature,
                        scheme,
                        address,
                        recovered_address,
                        ..
                    } => {
                        if scheme != "eip191-personal-sign" {
                            bail!("unsupported Ethereum signature scheme {scheme}");
                        }
                        let verified = verify_personal_sign(address, &message, signature)?;
                        if !verified
                            .recovered_address
                            .eq_ignore_ascii_case(recovered_address)
                        {
                            bail!(
                                "Ethereum recovered address mismatch for {}",
                                subject.route()
                            );
                        }
                    }
                }
            }
        }
        if require_signatures && !site_signed {
            bail!(
                "site signature required for {}; run websh-cli sign",
                subject.route()
            );
        }
        signed += usize::from(site_signed);
    }
    Ok(signed)
}
