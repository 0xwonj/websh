use std::path::{Path, PathBuf};

use websh_core::attestation::artifact::{Attestation, Subject, message_sha256};
use websh_core::attestation::ledger::ContentLedger;
use websh_core::crypto::ack::short_hash;
use websh_core::crypto::eth::verify_personal_sign;
use websh_core::crypto::pgp::normalize_fingerprint;

use crate::CliResult;
use crate::infra::json::read_json;
use crate::workflows::content::build_content_files;

use super::gpg::verify_pgp_signature;
use super::subject::{read_ack, read_artifact};

pub(crate) fn verify(root: &Path, route: Option<String>) -> CliResult {
    let artifact = read_artifact(root)?;
    artifact.validate_header()?;
    if artifact.subjects.is_empty() {
        return Err("no attestation subjects".into());
    }

    if let Some(route) = route {
        let subject = artifact
            .subject_for_route(&route)
            .ok_or_else(|| format!("attestation subject not found for route {route}"))?;
        verify_subject(root, subject)?;
        return Ok(());
    }

    for subject in &artifact.subjects {
        verify_subject(root, subject)?;
    }
    Ok(())
}

fn verify_subject(root: &Path, subject: &Subject) -> CliResult {
    subject.validate()?;

    let rebuilt = build_content_files(
        root,
        &subject
            .content_files()
            .iter()
            .map(|file| PathBuf::from(&file.path))
            .collect::<Vec<_>>(),
    )?;
    if rebuilt != subject.content_files() {
        return Err(format!("content file metadata mismatch for {}", subject.id()).into());
    }
    let content_sha256 = subject.content_sha256()?;

    match subject {
        Subject::Homepage(hp) => {
            let ack = read_ack(root)?;
            if ack.combined_root != hp.ack_combined_root {
                return Err(format!("ACK root mismatch for {}", subject.id()).into());
            }
        }
        Subject::Ledger(ls) => {
            let ledger_path = root.join(websh_core::attestation::ledger::CONTENT_LEDGER_PATH);
            let ledger: ContentLedger = read_json(&ledger_path)?;
            ledger.validate()?;
            if ledger.chain_head != ls.chain_head {
                return Err(format!("chain_head mismatch for {}", subject.id()).into());
            }
        }
        Subject::Document(_) | Subject::Page(_) => {}
    }

    let message = subject.canonical_message()?;
    let message_hash = message_sha256(&message);
    if subject.attestations().is_empty() {
        println!("{}: pending {}", subject.id(), short_hash(&content_sha256));
        return Ok(());
    }

    for attestation in subject.attestations() {
        if attestation.message_sha256() != message_hash {
            return Err(format!("attestation message hash mismatch for {}", subject.id()).into());
        }
        if !attestation.verified() {
            return Err(format!("stored attestation is not verified for {}", subject.id()).into());
        }

        match attestation {
            Attestation::Pgp {
                fingerprint,
                key_path,
                signature,
                ..
            } => {
                let verified_fingerprint =
                    verify_pgp_signature(root, Path::new(key_path), signature, &message)?;
                if normalize_fingerprint(fingerprint) != verified_fingerprint {
                    return Err(format!("PGP fingerprint mismatch for {}", subject.id()).into());
                }
                println!(
                    "{}: pgp ok {}",
                    subject.id(),
                    short_hash(&verified_fingerprint)
                );
            }
            Attestation::Ethereum {
                scheme,
                address,
                signature,
                recovered_address,
                ..
            } => {
                if scheme != "eip191-personal-sign" {
                    return Err(format!("unsupported Ethereum scheme {scheme}").into());
                }
                let verification = verify_personal_sign(address, &message, signature)?;
                if !verification
                    .recovered_address
                    .eq_ignore_ascii_case(recovered_address)
                {
                    return Err(format!(
                        "Ethereum recovered address mismatch for {}",
                        subject.id()
                    )
                    .into());
                }
                println!(
                    "{}: ethereum ok {}",
                    subject.id(),
                    short_hash(&verification.recovered_address)
                );
            }
        }
    }
    Ok(())
}
