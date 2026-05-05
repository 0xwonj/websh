use std::path::Path;

use websh_core::attestation::artifact::{AttestationArtifact, Subject};
use websh_core::crypto::ack::AckArtifact;
use websh_site::{ACK_ARTIFACT_PATH, ATTESTATIONS_PATH};

use crate::CliResult;
use crate::infra::json::read_json;

pub(in crate::workflows::attest) fn read_artifact(root: &Path) -> CliResult<AttestationArtifact> {
    let path = root.join(ATTESTATIONS_PATH);
    if !path.exists() {
        return Ok(AttestationArtifact::default());
    }
    read_json(&path)
}

pub(in crate::workflows::attest) fn read_ack(root: &Path) -> CliResult<AckArtifact> {
    let ack = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    ack.validate()?;
    Ok(ack)
}

pub(super) fn upsert_subject(artifact: &mut AttestationArtifact, subject: Subject) {
    if let Some(existing) = artifact.subject_for_route_mut(subject.route()) {
        *existing = subject;
    } else {
        artifact.subjects.push(subject);
    }
    artifact.subjects.sort_by_key(|subject| subject.id());
}
