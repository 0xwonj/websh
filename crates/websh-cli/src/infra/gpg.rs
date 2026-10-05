use crate::CliResult;
use anyhow::{Context, bail};
use std::io::Write;
use std::process::{Command, Stdio};

/// Explicit signing uses the pinned site fingerprint and pipes the message;
/// no persistent message/signature cache is needed.
pub(crate) fn sign(message: &str, signer: &str) -> CliResult<String> {
    let mut child = Command::new("gpg")
        .args([
            "--batch",
            "--yes",
            "--armor",
            "--detach-sign",
            "--local-user",
            signer,
            "--output",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start gpg; explicit signing requires the site secret key")?;
    let input = child.stdin.take().context("open gpg input")?;
    if let Err(error) = {
        let mut input = input;
        input.write_all(message.as_bytes())
    } {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error).context("write gpg message");
    }
    let output = child.wait_with_output().context("wait for gpg")?;
    if !output.status.success() {
        bail!(
            "gpg signing failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("gpg signature must be armored UTF-8")
}
