//! Shared helpers for invoking the `gh` CLI as a subprocess.
//!
//! Both `mount init` (Phase 4) and the `mempool` subcommand (Phase 5) shell
//! out to `gh` for GitHub API access. Centralizing the boilerplate keeps
//! their auth model identical (gh's stored credentials, set via
//! `gh auth login`) and avoids re-inventing the require/check/capture
//! patterns.

use std::ffi::OsStr;
use std::process::Command as Process;

use super::CliResult;

/// Verify that the `gh` CLI is installed and on `PATH`. Does not check
/// authentication — that's enforced by the actual `gh api` calls.
pub(crate) fn require_gh() -> CliResult {
    let probe = Process::new("gh").arg("--version").output();
    match probe {
        Ok(out) if out.status.success() => Ok(()),
        _ => Err("the `gh` CLI is required (https://cli.github.com); \
             ensure `gh auth status` reports an authenticated account before re-running"
            .into()),
    }
}

/// Run `gh` with the given args, return whether the process exited
/// successfully. Stdout / stderr are inherited from this process for
/// commands the user might want to see.
pub(crate) fn gh_succeeds<I, S>(args: I) -> CliResult<bool>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Process::new("gh").args(args).output()?;
    Ok(out.status.success())
}

/// Run `gh` with the given args, capture stdout as a `String`. Errors when
/// the process exits non-zero — stderr is included in the error message
/// so the caller can surface it without re-running.
pub(crate) fn gh_capture<I, S>(args: I) -> CliResult<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let out = Process::new("gh").args(args).output()?;
    if !out.status.success() {
        return Err(format!(
            "gh failed (exit {}): {}",
            out.status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "?".to_string()),
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
