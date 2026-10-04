use std::path::Path;

use super::process::run_status;
use crate::CliResult;

pub(crate) fn clean(root: &Path, dist: &Path, envs: &[(String, String)]) -> CliResult {
    run_status(
        root,
        "trunk",
        &["clean", "--dist", &dist.to_string_lossy()],
        envs,
        true,
    )
}

pub(crate) fn release(root: &Path, dist: &Path, envs: &[(String, String)]) -> CliResult {
    run_status(
        root,
        "trunk",
        &[
            "build",
            "--release",
            "--locked",
            "--dist",
            &dist.to_string_lossy(),
        ],
        envs,
        true,
    )
}
