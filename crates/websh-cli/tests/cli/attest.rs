use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use websh_core::attestation::artifact::{
    Attestation, AttestationArtifact, Subject, compute_content_sha256,
};
use websh_core::attestation::ledger::{CONTENT_LEDGER_PATH, ContentLedger};
use websh_core::crypto::pgp::normalize_fingerprint;
use websh_site::{ACK_ARTIFACT_PATH, ACK_COMMITMENT_JSON, ATTESTATIONS_PATH};

use crate::support::{cli, cli_fails, cli_with_env, temp_dir};

fn write_ack_artifact(root: &Path) {
    let path = root.join(ACK_ARTIFACT_PATH);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, ACK_COMMITMENT_JSON).unwrap();
}

fn write_homepage_content(root: &Path) {
    write_ack_artifact(root);
    fs::create_dir_all(root.join("crates/websh-web/src/features/home")).unwrap();
    fs::create_dir_all(root.join("assets/themes")).unwrap();
    fs::create_dir_all(root.join("content/.site")).unwrap();
    fs::write(
        root.join("crates/websh-web/src/features/home/mod.rs"),
        "home",
    )
    .unwrap();
    fs::write(
        root.join("crates/websh-web/src/features/home/home.module.css"),
        "home-css",
    )
    .unwrap();
    fs::write(
        root.join("crates/websh-web/src/features/home/sections.rs"),
        "sections",
    )
    .unwrap();
    fs::write(root.join("assets/themes/dracula.css"), "theme").unwrap();
    fs::write(
        root.join("content/.site/_index.dir.json"),
        r#"{"kind":"directory","authored":{"title":"Site support"},"derived":{"kind":"directory"}}"#,
    )
    .unwrap();
    fs::write(
        root.join("content/.site/now.toml"),
        "[[items]]\ndate = \"2026-04-26\"\ntext = \"Testing now data.\"\n",
    )
    .unwrap();
}

fn fake_gpg(root: &Path, script: &str) -> String {
    let bin = root.join("fake-bin");
    fs::create_dir_all(&bin).unwrap();
    let executable = bin.join("gpg");
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

#[test]
fn prepare_refreshes_development_manifest_and_unsigned_release_attestations() {
    for profile in ["debug", "release"] {
        let root = temp_dir("prepare");
        write_homepage_content(&root);
        let path = fake_gpg(&root, "#!/bin/sh\necho 'unexpected signing' >&2\nexit 91\n");
        cli_with_env(
            &root,
            &["prepare"],
            &[
                ("TRUNK_PROFILE", profile),
                ("WEBSH_NO_SIGN", "1"),
                ("PATH", &path),
            ],
        );
        assert!(root.join("content/manifest.json").exists());
        assert_eq!(root.join(ATTESTATIONS_PATH).exists(), profile == "release");
        assert_eq!(
            root.join(CONTENT_LEDGER_PATH).exists(),
            profile == "release"
        );
        if profile == "release" {
            let artifact: AttestationArtifact =
                serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap())
                    .unwrap();
            assert!(artifact.subject_for_route("/").is_some());
            assert!(
                artifact
                    .subjects
                    .iter()
                    .all(|subject| subject.attestations().is_empty())
            );
        }
    }
}

#[test]
fn attest_preserves_invalid_existing_artifacts_before_generating_content() {
    for body in [
        "corrupt artifact",
        r#"{"version":2,"scheme":"websh.attestations.v1","subjects":[]}"#,
    ] {
        let root = temp_dir("attest-invalid-existing");
        write_homepage_content(&root);
        let artifact = root.join(ATTESTATIONS_PATH);
        fs::write(&artifact, body).unwrap();

        cli_fails(&root, &["attest", "--no-sign"]);
        cli_fails(
            &root,
            &[
                "attest", "subject", "set", "--route", "/", "--kind", "homepage",
            ],
        );

        assert_eq!(fs::read_to_string(artifact).unwrap(), body);
        assert!(!root.join("content/manifest.json").exists());
        assert!(!root.join(CONTENT_LEDGER_PATH).exists());
    }
}

#[test]
fn attest_does_not_treat_artifact_read_errors_as_a_new_project() {
    let root = temp_dir("attest-artifact-read-error");
    write_homepage_content(&root);
    let artifact = root.join(ATTESTATIONS_PATH);
    fs::create_dir(&artifact).unwrap();

    cli_fails(&root, &["attest", "--no-sign"]);
    cli_fails(
        &root,
        &[
            "attest", "subject", "set", "--route", "/", "--kind", "homepage",
        ],
    );

    assert!(artifact.is_dir());
    assert!(!root.join("content/manifest.json").exists());
    assert!(!root.join(CONTENT_LEDGER_PATH).exists());
}

#[test]
fn attest_subject_set_builds_deterministic_content_hash() {
    let root = temp_dir("attest-set");
    write_ack_artifact(&root);
    fs::write(root.join("a.txt"), "alpha").unwrap();
    fs::write(root.join("b.txt"), "beta").unwrap();

    cli(
        &root,
        &[
            "attest",
            "subject",
            "set",
            "--route",
            "/",
            "--kind",
            "homepage",
            "--issued-at",
            "2026-04-26",
            "--content",
            "b.txt",
            "--content",
            "a.txt",
        ],
    );

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let subject = artifact.subject_for_route("/").unwrap();
    assert_eq!(
        subject
            .content_files()
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        vec!["a.txt", "b.txt"]
    );
    assert_eq!(
        compute_content_sha256(subject.content_files()).unwrap(),
        subject.content_sha256().unwrap()
    );

    let message = cli(&root, &["attest", "subject", "message", "--route", "/"]);
    assert_eq!(message.trim_end(), subject.canonical_message().unwrap());

    let first_hash = subject.content_sha256().unwrap();
    cli(
        &root,
        &[
            "attest",
            "subject",
            "set",
            "--route",
            "/",
            "--kind",
            "homepage",
            "--issued-at",
            "2026-04-26",
            "--content",
            "a.txt",
            "--content",
            "b.txt",
        ],
    );
    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    assert_eq!(
        artifact
            .subject_for_route("/")
            .unwrap()
            .content_sha256()
            .unwrap(),
        first_hash
    );
}

#[test]
fn attest_eth_import_rejects_invalid_signature() {
    let root = temp_dir("attest-eth");
    write_ack_artifact(&root);
    fs::write(root.join("page.txt"), "page").unwrap();
    cli(
        &root,
        &[
            "attest",
            "subject",
            "set",
            "--route",
            "/",
            "--kind",
            "homepage",
            "--issued-at",
            "2026-04-26",
            "--content",
            "page.txt",
        ],
    );

    cli_fails(
        &root,
        &[
            "attest",
            "subject",
            "eth-import",
            "--route",
            "/",
            "--address",
            "0x742d35Cc6634C0532925a3b844Bc454e44f3A8B4",
            "--signature",
            "0x1234",
        ],
    );
}

#[test]
fn attest_eth_import_accepts_valid_personal_sign_signature() {
    let root = temp_dir("attest-eth-valid");
    write_ack_artifact(&root);
    fs::write(root.join("page.txt"), "page").unwrap();
    cli(
        &root,
        &[
            "attest",
            "subject",
            "set",
            "--route",
            "/",
            "--kind",
            "homepage",
            "--issued-at",
            "2026-04-26",
            "--content",
            "page.txt",
        ],
    );

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let message = artifact
        .subject_for_route("/")
        .unwrap()
        .canonical_message()
        .unwrap();
    let (address, signature) = eth_personal_sign_fixture(&message);

    cli(
        &root,
        &[
            "attest",
            "subject",
            "eth-import",
            "--route",
            "/",
            "--address",
            &address,
            "--signature",
            &signature,
            "--signer",
            "test.eth",
        ],
    );
    cli(&root, &["attest", "verify", "--route", "/"]);

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let ethereum = artifact
        .subject_for_route("/")
        .unwrap()
        .attestations()
        .iter()
        .find_map(|attestation| match attestation {
            Attestation::Ethereum {
                signer,
                address,
                recovered_address,
                verified,
                ..
            } => Some((signer, address, recovered_address, verified)),
            _ => None,
        })
        .expect("Ethereum attestation is stored");
    assert_eq!(ethereum.0, "test.eth");
    assert_eq!(ethereum.1, &address);
    assert_eq!(ethereum.2, &address);
    assert!(*ethereum.3);
}

#[test]
fn attest_pgp_import_verifies_detached_signature() {
    let root = temp_dir("attest-pgp");
    write_ack_artifact(&root);
    fs::write(root.join("page.txt"), "page").unwrap();
    cli(
        &root,
        &[
            "attest",
            "subject",
            "set",
            "--route",
            "/",
            "--kind",
            "homepage",
            "--issued-at",
            "2026-04-26",
            "--content",
            "page.txt",
        ],
    );
    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let message = artifact
        .subject_for_route("/")
        .unwrap()
        .canonical_message()
        .unwrap();
    let (key_path, signature_path, fingerprint) = write_pgp_fixture(&root, &message);

    cli(
        &root,
        &[
            "attest",
            "subject",
            "pgp-import",
            "--route",
            "/",
            "--signature",
            signature_path.to_str().unwrap(),
            "--key",
            key_path.to_str().unwrap(),
        ],
    );
    cli(&root, &["attest", "verify", "--route", "/"]);
    cli(&root, &["attest", "verify"]);

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let subject = artifact.subject_for_route("/").unwrap();
    let pgp = subject
        .attestations()
        .iter()
        .find_map(|attestation| match attestation {
            Attestation::Pgp {
                signer,
                fingerprint,
                ..
            } => Some((signer, fingerprint)),
            _ => None,
        })
        .expect("PGP attestation is stored");
    assert_eq!(pgp.0.as_deref(), Some("Test User <test@example.com>"));
    assert_eq!(pgp.1, &fingerprint);
}

#[test]
fn attest_default_discovers_content_dir_and_manifest() {
    let root = temp_dir("attest-default");
    write_homepage_content(&root);
    fs::create_dir_all(root.join("content/writing")).unwrap();
    fs::write(
        root.join("content/writing/hello.md"),
        "---\ntitle: Hello Attested World\ntags: [crypto, writing]\n---\n# Ignored Heading\nbody",
    )
    .unwrap();

    cli(&root, &["attest", "--no-sign", "--issued-at", "2026-04-26"]);
    cli(&root, &["attest", "verify"]);

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("content/manifest.json")).unwrap())
            .unwrap();
    let entries = manifest["entries"].as_array().unwrap();
    let ledger = entries
        .iter()
        .find(|entry| entry["path"] == ".websh/ledger.json")
        .expect("ledger artifact is exposed through manifest");
    assert_eq!(
        ledger["metadata"]["derived"]["title"]
            .as_str()
            .or_else(|| ledger["metadata"]["authored"]["title"].as_str()),
        Some("ledger")
    );
    let hello = entries
        .iter()
        .find(|entry| entry["path"] == "writing/hello.md")
        .expect("content file remains in manifest");
    assert_eq!(
        hello["metadata"]["authored"]["title"].as_str(),
        Some("Hello Attested World")
    );

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let ledger_subject = artifact.subject_for_route("/ledger").unwrap();
    assert!(matches!(ledger_subject, Subject::Ledger(_)));
    let site_subject = artifact.subject_for_route("/.site").unwrap();
    assert!(matches!(site_subject, Subject::Directory(_)));
    assert!(
        site_subject
            .content_files()
            .iter()
            .any(|file| file.path == "content/.site/now.toml")
    );
    let ledger: ContentLedger =
        serde_json::from_str(&fs::read_to_string(root.join(CONTENT_LEDGER_PATH)).unwrap()).unwrap();
    ledger.validate().unwrap();
    assert!(
        ledger
            .blocks
            .iter()
            .any(|block| block.entry.path == ".site" && block.entry.route == "/.site")
    );
    if let Subject::Ledger(ledger_subject) = ledger_subject {
        assert_eq!(ledger_subject.chain_head, ledger.chain_head);
    }
    assert_eq!(
        ledger_subject
            .content_files()
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>(),
        vec!["content/.websh/ledger.json"]
    );
    let subject = artifact.subject_for_route("/writing/hello").unwrap();
    assert!(matches!(subject, Subject::Page(_)));
    let content_files: Vec<&str> = subject
        .content_files()
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert!(content_files.contains(&"content/writing/hello.md"));
    // sync also auto-generates the sidecar; it's part of the attested set.
    assert!(content_files.contains(&"content/writing/hello.meta.json"));
}

#[test]
fn attest_default_updates_existing_subject_issued_at_when_content_changes() {
    let root = temp_dir("attest-default-refresh-date");
    write_homepage_content(&root);
    fs::create_dir_all(root.join("content/writing")).unwrap();
    fs::write(
        root.join("content/writing/hello.md"),
        "---\ntitle: Hello\n---\nfirst body",
    )
    .unwrap();

    cli(&root, &["attest", "--no-sign", "--issued-at", "2000-01-01"]);

    fs::write(
        root.join("content/writing/hello.md"),
        "---\ntitle: Hello\n---\nrevised body",
    )
    .unwrap();
    cli(&root, &["attest", "--no-sign"]);

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    assert_eq!(
        artifact.subject_for_route("/").unwrap().issued_at(),
        "2000-01-01"
    );
    assert_ne!(
        artifact
            .subject_for_route("/writing/hello")
            .unwrap()
            .issued_at(),
        "2000-01-01"
    );
}

#[test]
fn attest_default_can_sign_with_local_gpg() {
    let root = temp_dir("attest-default-pgp");
    write_homepage_content(&root);

    cli(&root, &["attest", "--no-sign", "--issued-at", "2026-04-26"]);
    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let message = artifact
        .subject_for_route("/")
        .unwrap()
        .canonical_message()
        .unwrap();
    let ledger_message = artifact
        .subject_for_route("/ledger")
        .unwrap()
        .canonical_message()
        .unwrap();
    let directory_message = artifact
        .subject_for_route("/.site")
        .unwrap()
        .canonical_message()
        .unwrap();
    let (key_path, signature_dir, fingerprint) = write_pgp_fixture_set(
        &root,
        &[
            ("root", &message),
            ("site", &directory_message),
            ("ledger", &ledger_message),
        ],
    );

    let path = fake_gpg(
        &root,
        "#!/bin/sh\nif [ \"$1\" = \"--with-colons\" ] && [ \"$2\" = \"--list-secret-keys\" ]; then\n  printf 'sec:::::::::\\n'\n  printf 'fpr:::::::::%s:\\n' \"$WEBSH_FAKE_GPG_FINGERPRINT\"\n  exit 0\nfi\nout=\"\"\nin=\"\"\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then\n    shift\n    out=\"$1\"\n  else\n    in=\"$1\"\n  fi\n  shift\ndone\nslug=$(basename \"$in\" .message.txt)\ncp \"$WEBSH_FAKE_GPG_SIGNATURE_DIR/$slug.sig.asc\" \"$out\"\n",
    );

    cli_with_env(
        &root,
        &[
            "attest",
            "--issued-at",
            "2026-04-26",
            "--key",
            key_path.to_str().unwrap(),
        ],
        &[
            ("PATH", &path),
            (
                "WEBSH_FAKE_GPG_SIGNATURE_DIR",
                signature_dir.to_str().unwrap(),
            ),
            ("WEBSH_FAKE_GPG_FINGERPRINT", &fingerprint),
        ],
    );
    cli(&root, &["attest", "verify"]);

    let artifact: AttestationArtifact =
        serde_json::from_str(&fs::read_to_string(root.join(ATTESTATIONS_PATH)).unwrap()).unwrap();
    let pgp = artifact
        .subject_for_route("/")
        .unwrap()
        .attestations()
        .iter()
        .find_map(|attestation| match attestation {
            Attestation::Pgp {
                signer,
                fingerprint,
                ..
            } => Some((signer, fingerprint)),
            _ => None,
        })
        .expect("PGP attestation is stored");
    assert_eq!(pgp.0.as_deref(), Some("Test User <test@example.com>"));
    assert_eq!(pgp.1, &fingerprint);
    let ledger_pgp = artifact
        .subject_for_route("/ledger")
        .unwrap()
        .attestations()
        .iter()
        .find_map(|attestation| match attestation {
            Attestation::Pgp { fingerprint, .. } => Some(fingerprint),
            _ => None,
        })
        .expect("ledger PGP attestation is stored");
    assert_eq!(ledger_pgp, &fingerprint);
}

fn write_pgp_fixture(root: &Path, message: &str) -> (PathBuf, PathBuf, String) {
    let (key_path, signature_dir, fingerprint) =
        write_pgp_fixture_set(root, &[("subject", message)]);
    (key_path, signature_dir.join("subject.sig.asc"), fingerprint)
}

fn write_pgp_fixture_set(root: &Path, messages: &[(&str, &str)]) -> (PathBuf, PathBuf, String) {
    use pgp::composed::{ArmorOptions, DetachedSignature, KeyType, SecretKeyParamsBuilder};
    use pgp::crypto::hash::HashAlgorithm;
    use pgp::types::{KeyDetails, Password};

    let mut rng = rand08::thread_rng();
    let key_params = SecretKeyParamsBuilder::default()
        .key_type(KeyType::Ed25519Legacy)
        .can_certify(true)
        .can_sign(true)
        .primary_user_id("Test User <test@example.com>".into())
        .passphrase(None)
        .build()
        .unwrap();
    let secret = key_params.generate(&mut rng).unwrap();
    let public = secret.to_public_key();
    let fingerprint = normalize_fingerprint(&public.fingerprint().to_string());

    let key_path = root.join(".test-keys/test.asc");
    fs::create_dir_all(key_path.parent().unwrap()).unwrap();
    fs::write(
        &key_path,
        public.to_armored_string(ArmorOptions::default()).unwrap(),
    )
    .unwrap();

    let signature_dir = root.join(".test-signatures");
    fs::create_dir_all(&signature_dir).unwrap();
    for (slug, message) in messages {
        let signature = DetachedSignature::sign_binary_data(
            &mut rng,
            &secret.primary_key,
            &Password::empty(),
            HashAlgorithm::Sha256,
            message.as_bytes(),
        )
        .unwrap();
        fs::write(
            signature_dir.join(format!("{slug}.sig.asc")),
            signature
                .to_armored_string(ArmorOptions::default())
                .unwrap(),
        )
        .unwrap();
    }

    (key_path, signature_dir, fingerprint)
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
