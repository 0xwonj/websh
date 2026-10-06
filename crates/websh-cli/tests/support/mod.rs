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

/// Minimal current site inputs, with the real public identity and no private key.
pub fn write_site_fixture(root: &Path) {
    use std::fs;
    use websh_core::attestation::artifact::AttestationArtifact;
    use websh_core::crypto::ack::{AckPrivateSource, build_artifact_from_source};
    fs::create_dir_all(root.join("content/.site")).unwrap();
    fs::create_dir_all(root.join("content/.websh")).unwrap();
    fs::create_dir_all(root.join("content/writing")).unwrap();
    fs::write(
        root.join("content/writing/note.md"),
        "---\ntitle: Note\n---\nBody\n",
    )
    .unwrap();
    fs::write(
        root.join("content/.site/profile.toml"),
        r#"
title = "Fixture"
tagline = "Research"
name = "Fixture Author"
affiliation = "Independent"
email = "fixture@example.test"
abstract_text = "An archive"
introduction = "Introduction"
public_identity = "Public"
private_identity = "Private"
status = "Research"
research = ["PL"]
tools = ["Rust"]
habits = ["Reading"]
categories = ["writing"]
keywords = ["test"]
links = []
"#,
    )
    .unwrap();
    fs::write(
        root.join("content/.site/now.toml"),
        "[[items]]\ndate = \"2026-10-06\"\ntext = \"fixture\"\n",
    )
    .unwrap();
    let ack = build_artifact_from_source(&AckPrivateSource::default()).unwrap();
    fs::write(
        root.join(websh_site::ACK_ARTIFACT_PATH),
        format!("{}\n", serde_json::to_string_pretty(&ack).unwrap()),
    )
    .unwrap();
    fs::write(
        root.join(websh_site::ATTESTATIONS_PATH),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&AttestationArtifact::default()).unwrap()
        ),
    )
    .unwrap();
}
