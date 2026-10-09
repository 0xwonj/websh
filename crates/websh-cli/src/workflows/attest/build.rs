use crate::CliResult;
use websh_core::attestation::artifact::AttestationArtifact;
use websh_core::publication::{Manifest, expected_subjects};

pub(crate) fn prepare(
    manifest: &Manifest,
    existing: &AttestationArtifact,
) -> CliResult<AttestationArtifact> {
    let mut subjects = expected_subjects(manifest)?;
    for subject in &mut subjects {
        if let Some(prior) = existing.subject_for_route(subject.route())
            && subject.same_payload(prior)
            && !prior.attestations().is_empty()
        {
            subject.set_issued_at(prior.issued_at().map(str::to_owned));
            *subject.attestations_mut() = prior.attestations().to_vec();
        }
        subject.validate()?;
    }
    Ok(AttestationArtifact {
        subjects,
        ..AttestationArtifact::default()
    })
}
