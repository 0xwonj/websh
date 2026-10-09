//! Page commitments and signature evidence shared by native publication and the reader.
use serde::Serialize;
use std::collections::BTreeSet;

use crate::attestation::artifact::{
    Attestation, BundleSubject, ContentFile, DirectorySubject, DocumentSubject, Envelope,
    PageSubject, Subject, ViewSubject, message_sha256, sha256_hex,
};
use crate::crypto::pgp::{PgpPolicy, normalize_fingerprint, verify_detached};
use crate::domain::{NodeKind, VirtualPath};
use crate::filesystem::{GlobalFs, content_route_for_path};
use crate::mempool::LEDGER_CATEGORIES;
use crate::ports::manifest_snapshot;

use super::{Manifest, PublicationChain, ReleaseError, VerifiedRelease};

#[derive(Serialize)]
struct HomePageData<'a> {
    home: &'a super::HomeProjection,
    counts: Vec<(String, usize)>,
    recent: Vec<super::RecentItem>,
}

fn invalid(error: impl std::fmt::Display) -> ReleaseError {
    ReleaseError::Invalid(error.to_string())
}

fn indexed_fs(manifest: &Manifest) -> Result<GlobalFs, ReleaseError> {
    let scan = manifest_snapshot(manifest)?;
    let mut fs = GlobalFs::empty();
    fs.mount_scanned_subtree(VirtualPath::root(), &scan)
        .map_err(invalid)?;
    Ok(fs)
}

/// Required routes are explicit; arbitrary ancestors and external mounts are not signers.
pub fn expected_subjects(manifest: &Manifest) -> Result<Vec<Subject>, ReleaseError> {
    let release = manifest
        .release
        .as_ref()
        .ok_or_else(|| invalid("missing root release"))?;
    let mut routes = vec!["/".to_string(), "/ledger".to_string()];
    routes.extend(
        LEDGER_CATEGORIES
            .iter()
            .map(|category| format!("/{category}")),
    );
    routes.extend(
        release
            .publications
            .iter()
            .map(|path| content_route_for_path(path)),
    );
    routes.sort();
    if routes.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid("ambiguous page signature route"));
    }
    let fs = indexed_fs(manifest)?;
    let chain = PublicationChain::from_manifest(manifest)?;
    routes
        .iter()
        .map(|route| project_subject(manifest, route, &fs, &chain))
        .collect()
}

fn expected_subject(manifest: &Manifest, route: &str) -> Result<Subject, ReleaseError> {
    project_subject(
        manifest,
        route,
        &indexed_fs(manifest)?,
        &PublicationChain::from_manifest(manifest)?,
    )
}

fn project_subject(
    manifest: &Manifest,
    route: &str,
    fs: &GlobalFs,
    chain: &PublicationChain,
) -> Result<Subject, ReleaseError> {
    let release = manifest
        .release
        .as_ref()
        .ok_or_else(|| invalid("missing root release"))?;
    if route == "/"
        || route == "/ledger"
        || LEDGER_CATEGORIES.iter().any(|c| route == format!("/{c}"))
    {
        let bytes = if route == "/" {
            serde_json::to_vec(&HomePageData {
                home: &release.home,
                counts: LEDGER_CATEGORIES
                    .iter()
                    .map(|category| {
                        let path =
                            VirtualPath::from_absolute(format!("/{category}")).expect("category");
                        ((*category).to_string(), super::count_toc_entries(fs, &path))
                    })
                    .collect(),
                recent: super::recent_items_from_fs(fs),
            })?
        } else {
            let path = VirtualPath::from_absolute(route).map_err(invalid)?;
            let filter = super::ledger_filter_for_route(route, &path);
            super::ledger::commitment_bytes(&super::build_ledger_model(fs, chain, &filter))?
        };
        let view = ViewSubject {
            route: route.into(),
            site: release.site.clone(),
            content_sha256: sha256_hex(&bytes),
            issued_at: None,
            attestations: Vec::new(),
        };
        return Ok(if route == "/" {
            Subject::Home(view)
        } else {
            Subject::Ledger(view)
        });
    }
    let path = release
        .publications
        .iter()
        .find(|path| content_route_for_path(path) == route)
        .ok_or_else(|| invalid(format!("unknown page subject: {route}")))?;
    let entry = manifest
        .entries
        .iter()
        .find(|entry| entry.path == *path)
        .ok_or_else(|| invalid("subject publication is not indexed"))?;
    let mut files = super::membership::publication_entries(manifest, path)?
        .into_iter()
        .filter(|file| !file.metadata.kind.is_directory_like())
        .map(|file| {
            let integrity = manifest.integrity(&file.path)?;
            Ok(ContentFile {
                path: format!("content/{}", file.path),
                sha256: format!("0x{}", integrity.sha256),
                bytes: integrity.size,
            })
        })
        .collect::<Result<Vec<_>, ReleaseError>>()?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    let env = Envelope {
        route: route.into(),
        issued_at: None,
        content_files: files,
        attestations: vec![],
    };
    Ok(match entry.metadata.kind {
        NodeKind::Bundle => Subject::Bundle(BundleSubject { env }),
        NodeKind::Directory => Subject::Directory(DirectorySubject { env }),
        NodeKind::Page => Subject::Page(PageSubject { env }),
        _ => Subject::Document(DocumentSubject { env }),
    })
}

pub fn subject_for_path<'a>(manifest: &'a Manifest, path: &VirtualPath) -> Option<&'a Subject> {
    let release = manifest.release.as_ref()?;
    if release
        .mounts
        .iter()
        .any(|mount| path.starts_with(mount.mount_at()))
    {
        return None;
    }
    let route = content_route_for_path(path.as_str());
    if let Some(subject) = release.attestations.subject_for_route(&route) {
        return Some(subject);
    }
    // Only explicitly committed membership permits a file to use its publication proof.
    let file = format!("content{}", path.as_str());
    release.attestations.subjects.iter().find(|subject| {
        subject
            .content_files()
            .iter()
            .any(|entry| entry.path == file)
    })
}

/// Constructor-free evidence: the subject matches this release and its owner signature is valid.
#[derive(Clone, Debug)]
pub struct VerifiedSubject {
    subject: Subject,
    owner: Attestation,
    message_hash: String,
}
impl VerifiedSubject {
    pub fn subject(&self) -> &Subject {
        &self.subject
    }
    pub fn owner(&self) -> &Attestation {
        &self.owner
    }
    pub fn message_hash(&self) -> &str {
        &self.message_hash
    }
}

pub fn verify_subject_signature(
    subject: &Subject,
    attestation: &Attestation,
    policy: &PgpPolicy<'_>,
    now: u64,
) -> Result<(), ReleaseError> {
    subject.validate().map_err(invalid)?;
    let message = subject.canonical_message().map_err(invalid)?;
    if message_sha256(&message) != attestation.message_sha256() {
        return Err(invalid("page message hash mismatch"));
    }
    let Attestation::Pgp {
        fingerprint,
        signature,
        ..
    } = attestation
    else {
        return Err(invalid("owner PGP signature required"));
    };
    if normalize_fingerprint(fingerprint) != normalize_fingerprint(policy.primary_fingerprint) {
        return Err(invalid("page signer does not match owner"));
    }
    verify_detached(message.as_bytes(), signature.as_bytes(), policy, now)?;
    Ok(())
}

pub fn verify_subject(
    release: &VerifiedRelease,
    path: &VirtualPath,
    policy: &PgpPolicy<'_>,
    now: u64,
) -> Result<VerifiedSubject, ReleaseError> {
    let subject =
        subject_for_path(release.manifest(), path).ok_or_else(|| invalid("page is unsigned"))?;
    let expected = expected_subject(release.manifest(), subject.route())?;
    if !subject.same_payload(&expected) {
        return Err(invalid("page subject differs from current content"));
    }
    let owner = subject
        .attestations()
        .iter()
        .find(|attestation| matches!(attestation, Attestation::Pgp { .. }))
        .ok_or_else(|| invalid("page is unsigned"))?;
    verify_subject_signature(subject, owner, policy, now)?;
    Ok(VerifiedSubject {
        subject: subject.clone(),
        owner: owner.clone(),
        message_hash: owner.message_sha256().to_string(),
    })
}

pub(super) fn validate_catalog(manifest: &Manifest) -> Result<(), ReleaseError> {
    let Some(release) = &manifest.release else {
        return Ok(());
    };
    let catalog = &release.attestations;
    catalog.validate_header().map_err(invalid)?;
    if catalog.subjects.len() > manifest.entries.len() + LEDGER_CATEGORIES.len() + 2 {
        return Err(invalid("too many page subjects"));
    }
    let mut routes = BTreeSet::new();
    for subject in &catalog.subjects {
        if !routes.insert(subject.route()) || subject.attestations().len() > 16 {
            return Err(invalid("duplicate route or too many attestations"));
        }
        VirtualPath::from_absolute(subject.route()).map_err(invalid)?;
        subject.validate().map_err(invalid)?;
        if subject
            .attestations()
            .iter()
            .filter(|a| matches!(a, Attestation::Pgp { .. }))
            .count()
            > 1
        {
            return Err(invalid("ambiguous owner signature"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ContentManifestEntry, NodeMetadata, ReleaseMetadata};

    fn manifest() -> Manifest {
        let mut metadata = NodeMetadata {
            kind: NodeKind::Page,
            ..Default::default()
        };
        metadata.authored.title = Some("Note".into());
        metadata.authored.date = Some("2026-01-01".into());
        metadata.derived.size_bytes = Some(4);
        metadata.derived.content_sha256 = Some(super::super::ReleaseId::of(b"note").to_string());
        Manifest {
            entries: vec![ContentManifestEntry {
                path: "writing/note.md".into(),
                metadata,
                mempool: None,
            }],
            release: Some(ReleaseMetadata {
                purpose: super::super::CONTENT_PURPOSE.into(),
                site: "fixture".into(),
                sequence: 1,
                issued_at: 1,
                home: super::super::tests::home(),
                mounts: vec![],
                publications: vec!["writing/note.md".into()],
                attestations: Default::default(),
            }),
        }
    }

    #[test]
    fn page_dependencies_are_independent_of_release_issuance_and_proofs() {
        let mut manifest = manifest();
        let before = expected_subjects(&manifest).unwrap();
        let release = manifest.release.as_mut().unwrap();
        release.sequence += 1;
        release.issued_at += 1;
        release.attestations.subjects = before.clone();
        assert_eq!(before, expected_subjects(&manifest).unwrap());
        manifest.release.as_mut().unwrap().home.now.items[0].text = "Changed Now".into();
        let now = expected_subjects(&manifest).unwrap();
        for (old, new) in before.iter().zip(&now) {
            assert_eq!(old != new, old.route() == "/");
        }
        manifest.entries[0].metadata.derived.content_sha256 =
            Some(super::super::ReleaseId::of(b"edit").to_string());
        let body = expected_subjects(&manifest).unwrap();
        for (old, new) in now.iter().zip(&body) {
            assert_eq!(old != new, old.route() != "/");
        }
        manifest.entries[0].metadata.authored.title = Some("Revised title".into());
        let renamed = expected_subjects(&manifest).unwrap();
        assert_ne!(body[0], renamed[0]);
        // A new sidecar invalidates a file-set proof even if the main file is unchanged.
        let old = expected_subject(&manifest, "/writing/note").unwrap();
        let chain = PublicationChain::from_manifest(&manifest).unwrap();
        let mut sidecar = manifest.entries[0].clone();
        sidecar.path = "writing/note.md.meta.json".into();
        manifest.entries.push(sidecar);
        let new = expected_subject(&manifest, "/writing/note").unwrap();
        assert!(!old.same_payload(&new));
        assert_eq!(new.content_files().len(), 2);
        assert_ne!(
            PublicationChain::from_manifest(&manifest).unwrap().head,
            chain.head
        );
    }

    #[test]
    fn view_messages_bind_site_kind_route_digest_and_issuance_exactly() {
        let subject = Subject::Home(ViewSubject {
            route: "/".into(),
            site: "fixture".into(),
            content_sha256: format!("0x{}", "1".repeat(64)),
            issued_at: Some("2026-01-01".into()),
            attestations: vec![],
        });
        assert_eq!(
            subject.canonical_message().unwrap(),
            format!(
                "websh.subject.v1\nid=route:/\nsite=fixture\nroute=/\nkind=home\ncontent_sha256=0x{}\nissued_at=2026-01-01",
                "1".repeat(64)
            )
        );
        let mut bad = subject.clone();
        if let Subject::Home(view) = &mut bad {
            view.site.push_str("\nroute=/other");
        }
        assert!(bad.validate().is_err());
        let mut manifest = manifest();
        manifest.release.as_mut().unwrap().attestations.subjects =
            expected_subjects(&manifest).unwrap();
        assert!(
            subject_for_path(
                &manifest,
                &VirtualPath::from_absolute("/writing/note/unknown.md").unwrap()
            )
            .is_none()
        );
        let mut duplicate = manifest.clone();
        duplicate
            .release
            .as_mut()
            .unwrap()
            .attestations
            .subjects
            .push(subject);
        assert!(duplicate.validate().is_err());
    }
}
