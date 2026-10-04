use std::fs;
use std::path::Path;

use websh_core::attestation::artifact::{Attestation, AttestationArtifact};
use websh_core::crypto::ack::AckArtifact;
use websh_core::crypto::eth::verify_personal_sign;
use websh_core::crypto::pgp::normalize_fingerprint;
use websh_site::{ACK_ARTIFACT_PATH, ATTESTATIONS_PATH, EXPECTED_PGP_FINGERPRINT, PUBLIC_KEY_PATH};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
}

#[test]
fn homepage_hybrid_ack_artifact_verifies() {
    let body =
        fs::read_to_string(workspace_root().join(ACK_ARTIFACT_PATH)).expect("read ACK artifact");
    let artifact: AckArtifact = serde_json::from_str(&body).expect("parse ACK artifact");
    artifact.validate().expect("ACK artifact validates");
}

#[test]
fn homepage_attestation_artifact_verifies() {
    let path = workspace_root().join(ATTESTATIONS_PATH);
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
fn homepage_pgp_key_matches_deployed_identity() {
    let path = workspace_root().join(PUBLIC_KEY_PATH);
    use pgp::composed::{Deserializable, SignedPublicKey};
    use pgp::types::KeyDetails;

    let (key, _headers) = SignedPublicKey::from_armor_file(&path).expect("parse OpenPGP key");
    assert_eq!(
        normalize_fingerprint(&key.fingerprint().to_string()),
        EXPECTED_PGP_FINGERPRINT
    );
}
