use std::collections::BTreeSet;

use anyhow::bail;
use websh_core::attestation::artifact::{
    AttestationArtifact, BundleSubject, ContentFile, DirectorySubject, DocumentSubject, Envelope,
    PageSubject, Subject,
};
use websh_core::domain::NodeKind;

use crate::CliResult;
use crate::workflows::content::ContentSnapshot;

pub(crate) fn prepare(
    content: &ContentSnapshot,
    existing: &AttestationArtifact,
) -> CliResult<AttestationArtifact> {
    let env = |route: String, content_files: Vec<ContentFile>| Envelope {
        route,
        content_files,
        issued_at: None,
        attestations: Vec::new(),
    };
    let mut subjects = Vec::new();
    for unit in &content.units {
        let env = env(unit.route.clone(), unit.files.clone());
        subjects.push(match unit.kind {
            NodeKind::Bundle => Subject::Bundle(BundleSubject { env }),
            NodeKind::Directory => Subject::Directory(DirectorySubject { env }),
            NodeKind::Page => Subject::Page(PageSubject { env }),
            _ => Subject::Document(DocumentSubject { env }),
        });
    }
    let mut routes = BTreeSet::new();
    for subject in &mut subjects {
        if !routes.insert(subject.route().to_string()) {
            bail!("duplicate attestation route {}", subject.route());
        }
        if let Some(prior) = existing.subject_for_route(subject.route())
            && same_payload(subject, prior)
            && !prior.attestations().is_empty()
        {
            subject.envelope_mut().issued_at = prior.envelope().issued_at.clone();
            subject.envelope_mut().attestations = prior.attestations().to_vec();
        }
        subject.validate()?;
    }
    subjects.sort_by_key(Subject::id);
    Ok(AttestationArtifact {
        subjects,
        ..AttestationArtifact::default()
    })
}

pub(crate) fn same_payload(left: &Subject, right: &Subject) -> bool {
    let unsigned = |subject: &Subject| {
        let mut subject = subject.clone();
        subject.envelope_mut().issued_at = None;
        subject.attestations_mut().clear();
        subject
    };
    unsigned(left) == unsigned(right)
}
