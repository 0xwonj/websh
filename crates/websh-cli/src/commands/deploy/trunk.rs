use std::path::Path;

use crate::CliResult;
use crate::infra::process::{CapturedOutput, run_output as run_process_output, run_status};

pub(super) fn run_trunk(root: &Path, args: &[&str], envs: &[(String, String)]) -> CliResult {
    run_status(root, "trunk", args, envs, true)
}

pub(super) fn run_output(
    root: &Path,
    program: &str,
    args: &[&str],
    envs: &[(String, String)],
) -> CliResult<CapturedOutput> {
    run_process_output(root, program, args, envs)
}
