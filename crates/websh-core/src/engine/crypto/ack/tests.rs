use super::*;

fn source_with_private() -> AckPrivateSource {
    AckPrivateSource {
        version: 1,
        entries: vec![
            AckSourceEntry {
                mode: AckEntryMode::Public,
                name: "Coffee".to_string(),
                nonce: None,
            },
            AckSourceEntry {
                mode: AckEntryMode::Private,
                name: "Anonymous Reviewer".to_string(),
                nonce: Some(
                    "0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                        .to_string(),
                ),
            },
        ],
    }
}

#[test]
fn normalization_is_stable() {
    assert_eq!(normalize_ack_name("  COFFEE\t\nhouse  "), "coffee house");
    assert_eq!(normalize_ack_name("Ａｄｖｉｓｏｒ"), "advisor");
    assert_eq!(normalize_ack_name("  홍길동\t님  "), "홍길동 님");
}

#[test]
fn receipt_filename_slug_is_ascii_and_hash_suffixed() {
    let ascii = slugify_name("Anonymous Reviewer");
    assert!(ascii.starts_with("anonymous-reviewer-"));
    assert!(
        ascii
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    );

    let korean = slugify_name("홍길동");
    assert!(korean.starts_with("ack-"));
    assert!(
        korean
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    );
    assert_ne!(korean, "ack-");
}

#[test]
fn empty_artifact_is_stable_and_valid() {
    let artifact = build_artifact_from_source(&AckPrivateSource::default()).unwrap();
    assert_eq!(artifact.public.count, 0);
    assert_eq!(artifact.private.count, 0);
    artifact.validate().unwrap();
}

#[test]
fn public_name_verifies_without_plaintext_artifact() {
    let artifact = build_artifact_from_source(&source_with_private()).unwrap();
    let proof = public_proof_for_name(&artifact, " coffee ")
        .unwrap()
        .expect("public proof");
    assert!(proof.verified);
    assert!(
        public_proof_for_name(&artifact, "anonymous reviewer")
            .unwrap()
            .is_none()
    );
}

#[test]
fn unicode_ack_names_verify() {
    let source = AckPrivateSource {
        version: 1,
        entries: vec![
            AckSourceEntry {
                mode: AckEntryMode::Public,
                name: "홍길동".to_string(),
                nonce: None,
            },
            AckSourceEntry {
                mode: AckEntryMode::Private,
                name: "익명 리뷰어".to_string(),
                nonce: Some(
                    "0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                        .to_string(),
                ),
            },
        ],
    };
    let artifact = build_artifact_from_source(&source).unwrap();

    let proof = public_proof_for_name(&artifact, " 홍길동 ")
        .unwrap()
        .expect("public proof");
    assert!(proof.verified);

    let receipt = private_receipt_from_source(&source, "익명 리뷰어").unwrap();
    let verification = verify_private_receipt(&artifact, &receipt).unwrap();
    assert_eq!(verification.combined_root, artifact.combined_root);
}

#[test]
fn private_receipt_verifies() {
    let source = source_with_private();
    let artifact = build_artifact_from_source(&source).unwrap();
    let receipt = private_receipt_from_source(&source, "anonymous reviewer").unwrap();
    let verification = verify_private_receipt(&artifact, &receipt).unwrap();
    assert_eq!(verification.combined_root, artifact.combined_root);
}

#[test]
fn altered_private_receipt_fails() {
    let source = source_with_private();
    let artifact = build_artifact_from_source(&source).unwrap();
    let mut receipt = private_receipt_from_source(&source, "anonymous reviewer").unwrap();
    receipt.nonce =
        "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe".to_string();
    assert!(verify_private_receipt(&artifact, &receipt).is_err());
}
