//! Canonical public crypto artifact paths and bundled deployed artifacts.

use websh_core::attestation::artifact::AttestationArtifact;
use websh_core::crypto::ack::AckArtifact;

pub const ATTESTATIONS_PATH: &str = "assets/crypto/attestations.json";
pub const ACK_ARTIFACT_PATH: &str = "assets/crypto/ack.commitment.json";

pub const ATTESTATIONS_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/crypto/attestations.json"
));

pub const ACK_COMMITMENT_JSON: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/crypto/ack.commitment.json"
));

pub fn attestation_artifact() -> Result<AttestationArtifact, serde_json::Error> {
    AttestationArtifact::from_json_str(ATTESTATIONS_JSON)
}

pub fn ack_artifact() -> Result<AckArtifact, serde_json::Error> {
    AckArtifact::from_json_str(ACK_COMMITMENT_JSON)
}
