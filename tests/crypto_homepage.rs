use std::fs;
use std::path::Path;

use websh::crypto::ack::{
    ACK_ARTIFACT_PATH, ACK_LOCAL_SOURCE_PATH, AckArtifact, AckPrivateSource,
    build_artifact_from_source,
};
use websh::crypto::attestation::{ATTESTATIONS_PATH, Attestation, AttestationArtifact};
use websh::crypto::eth::verify_personal_sign;
use websh::crypto::pgp::{EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_PATH, normalize_fingerprint};

#[test]
fn homepage_hybrid_ack_artifact_verifies() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let body = fs::read_to_string(root.join(ACK_ARTIFACT_PATH)).expect("read ACK artifact");
    let artifact: AckArtifact = serde_json::from_str(&body).expect("parse ACK artifact");
    artifact.validate().expect("ACK artifact validates");
}

#[test]
fn local_ack_plaintext_source_matches_public_artifact_when_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_path = root.join(ACK_LOCAL_SOURCE_PATH);
    if !source_path.exists() {
        return;
    }

    let source_body = fs::read_to_string(&source_path).expect("read local ACK plaintext source");
    let source: AckPrivateSource =
        serde_json::from_str(&source_body).expect("parse local ACK plaintext source");
    let rebuilt = build_artifact_from_source(&source).expect("rebuild ACK artifact from plaintext");

    let artifact_body =
        fs::read_to_string(root.join(ACK_ARTIFACT_PATH)).expect("read public ACK artifact");
    let artifact: AckArtifact =
        serde_json::from_str(&artifact_body).expect("parse public ACK artifact");

    assert_eq!(rebuilt, artifact);
}

#[test]
fn homepage_attestation_artifact_verifies_when_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(ATTESTATIONS_PATH);
    if !path.exists() {
        return;
    }

    let body = fs::read_to_string(&path).expect("read attestations artifact");
    let artifact: AttestationArtifact =
        serde_json::from_str(&body).expect("parse attestations artifact");
    artifact
        .validate_header()
        .expect("artifact header validates");
    let subject = artifact
        .subject_for_route("/")
        .expect("homepage subject is present");
    assert_eq!(subject.id(), "route:/");
    subject.validate().expect("homepage subject validates");
    let message = subject
        .canonical_message()
        .expect("canonical message renders");

    for attestation in subject.attestations() {
        if let Attestation::Ethereum {
            address,
            signature,
            recovered_address,
            ..
        } = attestation
        {
            let verification = verify_personal_sign(address, &message, signature)
                .expect("homepage Ethereum attestation should verify");
            assert_eq!(&verification.recovered_address, recovered_address);
        }
    }
}

#[test]
fn homepage_pgp_key_verifies_when_present() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join(PUBLIC_KEY_PATH);
    if !path.exists() {
        return;
    }

    use pgp::composed::{Deserializable, SignedPublicKey};
    use pgp::types::KeyDetails;

    let (key, _headers) = SignedPublicKey::from_armor_file(&path).expect("parse OpenPGP key");
    assert_eq!(
        normalize_fingerprint(&key.fingerprint().to_string()),
        EXPECTED_PGP_FINGERPRINT
    );
}
