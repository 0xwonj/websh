use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};
use websh_core::crypto::pgp::normalize_fingerprint;

use crate::CliResult;

/// A missing executable or key leaves release attestations pending.
pub(crate) fn secret_key_fingerprint(key: Option<&str>) -> Option<String> {
    let mut command = Command::new("gpg");
    command.args(["--with-colons", "--list-secret-keys"]);
    if let Some(key) = key {
        command.arg(key);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    // GnuPG's machine-readable format places a fingerprint in field 10.
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            let mut fields = line.split(':');
            (fields.next() == Some("fpr"))
                .then(|| fields.nth(8))
                .flatten()
                .filter(|value| !value.is_empty())
                .map(normalize_fingerprint)
        })
}

pub(crate) fn sign(
    root: &Path,
    message: &Path,
    signature: &Path,
    signer: Option<&str>,
) -> CliResult {
    let mut command = Command::new("gpg");
    command
        .current_dir(root)
        .args(["--yes", "--armor", "--detach-sign", "--output"])
        .arg(signature);
    if let Some(signer) = signer {
        command.arg("--local-user").arg(signer);
    }
    let output = command.arg(message).output().context("run gpg")?;
    if !output.status.success() {
        bail!(
            "gpg signing failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}
