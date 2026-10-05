use std::path::Path;
use std::process::Command;

use anyhow::{Context, bail};

use crate::CliResult;

pub(crate) struct CapturedOutput {
    pub(crate) stdout: String,
}

pub(crate) fn run_output(
    root: &Path,
    program: &str,
    args: &[&str],
    envs: &[(String, String)],
) -> CliResult<CapturedOutput> {
    let output = Command::new(program)
        .args(args)
        .envs(envs.iter().map(|(key, value)| (key, value)))
        .current_dir(root)
        .output()
        .with_context(|| format!("run {program} in {}", root.display()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        bail!("{program} exited with status {}\n{stderr}", output.status);
    }

    Ok(CapturedOutput { stdout })
}
