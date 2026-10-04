mod fs;

pub use fs::temp_dir;

use std::path::Path;
use std::process::{Command, Output};

fn run(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_websh-cli"))
        .arg("--root")
        .arg(root)
        .args(args)
        .env_remove("TRUNK_PROFILE")
        .env_remove("WEBSH_NO_SIGN")
        .envs(envs.iter().copied())
        .output()
        .expect("run websh-cli")
}

fn checked_output(output: Output, args: &[&str], success: bool) -> String {
    assert_eq!(
        output.status.success(),
        success,
        "websh-cli {args:?}: unexpected status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is utf8")
}

pub fn cli(root: &Path, args: &[&str]) -> String {
    cli_with_env(root, args, &[])
}

pub fn cli_with_env(root: &Path, args: &[&str], envs: &[(&str, &str)]) -> String {
    checked_output(run(root, args, envs), args, true)
}

pub fn cli_fails(root: &Path, args: &[&str]) {
    checked_output(run(root, args, &[]), args, false);
}
