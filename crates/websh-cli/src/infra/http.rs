//! Public readback is a bounded observation after Git publication, not another upload.
use crate::CliResult;
use anyhow::{Context, bail};
use std::process::Command;

pub(crate) fn get(url: &str) -> CliResult<Vec<u8>> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--max-time",
            "15",
            "--max-filesize",
            "8388608",
            url,
        ])
        .output()
        .context("start public readback")?;
    if !output.status.success() {
        bail!(
            "public readback unavailable: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}
