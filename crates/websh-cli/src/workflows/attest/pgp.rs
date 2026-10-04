use std::fs;
use std::path::Path;

use anyhow::Context;
use websh_core::attestation::artifact::{Attestation, Subject, message_sha256};

use crate::CliResult;
use crate::infra::{gpg, pgp};
use crate::workflows::content::{artifact_path, resolve_path};

pub(super) fn sign_subject_with_gpg(
    root: &Path,
    subject: &Subject,
    key: &Path,
    gpg_key: Option<&str>,
    signature_dir: &Path,
) -> CliResult<Attestation> {
    let message = subject.canonical_message()?;
    let signature_dir = resolve_path(root, signature_dir);
    fs::create_dir_all(&signature_dir)
        .with_context(|| format!("create directory {}", signature_dir.display()))?;
    let slug = slugify_route(subject.route());
    let message_path = signature_dir.join(format!("{slug}.message.txt"));
    let signature_path = signature_dir.join(format!("{slug}.sig.asc"));
    fs::write(&message_path, &message)
        .with_context(|| format!("write {}", message_path.display()))?;

    gpg::sign(root, &message_path, &signature_path, gpg_key).with_context(|| {
        format!(
            "sign {} with gpg; use --no-sign to leave it pending",
            subject.route()
        )
    })?;

    let signature_body = fs::read_to_string(&signature_path)
        .with_context(|| format!("read {}", signature_path.display()))?;
    let fingerprint = pgp::verify_signature(&resolve_path(root, key), &signature_body, &message)?;
    let signer = pgp::read_key(&resolve_path(root, key))
        .ok()
        .and_then(|key| key.user_ids.into_iter().find(|id| !id.is_empty()))
        .or_else(|| gpg_key.map(ToOwned::to_owned));
    Ok(Attestation::Pgp {
        signer,
        fingerprint,
        key_path: artifact_path(root, key)?,
        signature: signature_body,
        signature_path: artifact_path(root, &signature_path).ok(),
        message_sha256: message_sha256(&message),
        verified: true,
    })
}

fn slugify_route(route: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in route.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let slug = out.trim_matches('-');
    if slug.is_empty() {
        "root".to_string()
    } else {
        slug.to_string()
    }
}
