//! Canonical public crypto artifact paths and bundled deployed artifacts.

use std::sync::LazyLock;

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

#[derive(Debug, thiserror::Error)]
pub enum SiteArtifactError {
    #[error("parse bundled attestation artifact: {source}")]
    Attestations {
        #[source]
        source: serde_json::Error,
    },
    #[error("parse bundled ACK artifact: {source}")]
    Ack {
        #[source]
        source: serde_json::Error,
    },
}

pub fn attestation_artifact() -> Result<&'static AttestationArtifact, &'static SiteArtifactError> {
    static ARTIFACT: LazyLock<Result<AttestationArtifact, SiteArtifactError>> =
        LazyLock::new(|| {
            AttestationArtifact::from_json_str(ATTESTATIONS_JSON)
                .map_err(|source| SiteArtifactError::Attestations { source })
        });
    ARTIFACT.as_ref()
}

pub fn ack_artifact() -> Result<&'static AckArtifact, &'static SiteArtifactError> {
    static ARTIFACT: LazyLock<Result<AckArtifact, SiteArtifactError>> = LazyLock::new(|| {
        AckArtifact::from_json_str(ACK_COMMITMENT_JSON)
            .map_err(|source| SiteArtifactError::Ack { source })
    });
    ARTIFACT.as_ref()
}
