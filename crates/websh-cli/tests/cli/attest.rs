use crate::support::{cli, cli_fails, cli_with_env, temp_dir, write_site_fixture};
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::Path};
use websh_core::attestation::artifact::AttestationArtifact;
use websh_site::ATTESTATIONS_PATH;

fn artifacts(root: &Path) -> Vec<Vec<u8>> {
    [
        "content/manifest.json",
        websh_site::ACK_ARTIFACT_PATH,
        ATTESTATIONS_PATH,
    ]
    .iter()
    .map(|path| fs::read(root.join(path)).unwrap())
    .collect()
}

#[test]
fn unsigned_sync_is_profile_independent_and_failed_signing_preserves_artifacts() {
    let root = temp_dir("unsigned-sync");
    write_site_fixture(&root);
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let gpg = bin.join("gpg");
    fs::write(&gpg, "#!/bin/sh\nexit 97\n").unwrap();
    fs::set_permissions(&gpg, fs::Permissions::from_mode(0o755)).unwrap();
    cli_with_env(
        &root,
        &["sync"],
        &[
            ("PATH", bin.to_str().unwrap()),
            ("TRUNK_PROFILE", "release"),
        ],
    );
    let before = artifacts(&root);
    let artifact: AttestationArtifact = serde_json::from_slice(&before[2]).unwrap();
    assert!(
        artifact
            .subjects
            .iter()
            .all(|subject| subject.issued_at().is_none() && subject.attestations().is_empty())
    );
    cli(&root, &["check"]);
    cli_fails(&root, &["check", "--require-signatures"]);
    cli(&root, &["sync"]);
    assert_eq!(before, artifacts(&root));
    let message = cli(&root, &["attest", "message", "/writing/note"]);
    assert!(message.starts_with("websh.subject.v1\n"));
    assert!(!message.ends_with('\n'));
    assert_eq!(before, artifacts(&root));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_websh-cli"))
        .args([
            "--root",
            root.to_str().unwrap(),
            "attest",
            "sign",
            "/writing/note",
        ])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("gpg signing failed"));
    let root_sign = std::process::Command::new(env!("CARGO_BIN_EXE_websh-cli"))
        .args(["--root", root.to_str().unwrap(), "sign"])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(!root_sign.status.success());
    assert!(!root.join("content/manifest.sig").exists());
    assert_eq!(before, artifacts(&root));
}

#[test]
fn external_signature_round_trip_preserves_evidence_and_rejects_stale_requests() {
    let root = temp_dir("external-signature");
    write_site_fixture(&root);
    cli(&root, &["sync"]);
    let message = cli(&root, &["attest", "message", "/writing/note"]);
    let request = root.join("request.txt");
    fs::write(&request, &message).unwrap();
    let (address, signature) = eth_personal_sign_fixture(&message);
    let args = [
        "attest",
        "import",
        "ethereum",
        "/writing/note",
        "--message",
        request.to_str().unwrap(),
        "--address",
        &address,
        "--signature",
        &signature,
    ];
    cli(&root, &args);
    cli(&root, &["check"]);
    cli_fails(&root, &["check", "--require-signatures"]);
    let signed = artifacts(&root);
    cli(&root, &["sync"]);
    assert_eq!(signed, artifacts(&root));
    fs::write(root.join("content/writing/note.md"), "{\"changed\":true}\n").unwrap();
    cli_fails(&root, &args);
    assert_eq!(signed, artifacts(&root));
    cli_fails(&root, &["check"]);
    cli(&root, &["sync"]);
    let artifact: AttestationArtifact =
        serde_json::from_slice(&fs::read(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let subject = artifact.subject_for_route("/writing/note").unwrap();
    assert!(subject.issued_at().is_none());
    assert!(subject.attestations().is_empty());
}

#[test]
fn missing_or_corrupt_retained_evidence_is_not_silently_recreated() {
    let root = temp_dir("retained-evidence");
    write_site_fixture(&root);
    cli(&root, &["sync"]);
    let manifest = fs::read(root.join("content/manifest.json")).unwrap();
    let path = root.join(ATTESTATIONS_PATH);
    fs::write(&path, "broken").unwrap();
    cli_fails(&root, &["sync"]);
    assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    fs::remove_file(&path).unwrap();
    cli_fails(&root, &["sync"]);
    assert!(!path.exists());
    assert_eq!(
        manifest,
        fs::read(root.join("content/manifest.json")).unwrap()
    );
}
fn eth_personal_sign_fixture(message: &str) -> (String, String) {
    use alloy_primitives::{Address, eip191_hash_message};
    use k256::ecdsa::SigningKey;

    let private_key =
        hex::decode("4c0883a69102937d6231471b5dbb6204fe5129617082792ae468d01a3f362318").unwrap();
    let signing_key = SigningKey::from_slice(&private_key).unwrap();
    let address = Address::from_private_key(&signing_key).to_checksum(None);
    let prehash = eip191_hash_message(message);
    let (signature, recovery_id) = signing_key
        .sign_prehash_recoverable(prehash.as_slice())
        .unwrap();
    let mut bytes = [0u8; 65];
    bytes[..64].copy_from_slice(signature.to_bytes().as_slice());
    bytes[64] = 27 + recovery_id.to_byte();

    (address, format!("0x{}", hex::encode(bytes)))
}
