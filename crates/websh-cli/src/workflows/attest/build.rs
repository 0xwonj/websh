use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use websh_core::attestation::artifact::{
    AttestationArtifact, BundleSubject, ContentFile, DirectorySubject, DocumentSubject, Envelope,
    HomepageSubject, LedgerSubject, PageSubject, Subject, sha256_hex,
};
use websh_core::attestation::ledger::{CONTENT_LEDGER_PATH, CONTENT_LEDGER_ROUTE};
use websh_core::crypto::ack::AckArtifact;
use websh_core::domain::NodeKind;
use websh_site::{ACK_ARTIFACT_PATH, PUBLIC_KEY_PATH};

use crate::CliResult;
use crate::workflows::content::{ContentSnapshot, artifact_bytes, collect_files_recursive};

pub(crate) fn prepare(
    root: &Path,
    content: &ContentSnapshot,
    ack: &AckArtifact,
    existing: &AttestationArtifact,
) -> CliResult<AttestationArtifact> {
    let env = |route: String, content_files: Vec<ContentFile>| Envelope {
        route,
        content_files,
        issued_at: None,
        attestations: Vec::new(),
    };
    let mut subjects = vec![
        Subject::Homepage(HomepageSubject {
            env: env("/".to_string(), homepage_files(root, content, ack)?),
            ack_combined_root: ack.combined_root.clone(),
        }),
        Subject::Ledger(LedgerSubject {
            env: env(
                CONTENT_LEDGER_ROUTE.to_string(),
                vec![file_record(
                    CONTENT_LEDGER_PATH,
                    &artifact_bytes(&content.ledger)?,
                )],
            ),
            chain_head: content.ledger.chain_head.clone(),
        }),
    ];
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

pub(crate) fn file_record(path: &str, bytes: &[u8]) -> ContentFile {
    ContentFile {
        path: path.to_string(),
        sha256: sha256_hex(bytes),
        bytes: bytes.len() as u64,
    }
}

fn homepage_files(
    root: &Path,
    content: &ContentSnapshot,
    ack: &AckArtifact,
) -> CliResult<Vec<ContentFile>> {
    let known = content
        .units
        .iter()
        .flat_map(|unit| &unit.files)
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let mut paths = vec![
        PathBuf::from("content/.site/now.toml"),
        PathBuf::from(PUBLIC_KEY_PATH),
    ];
    for directory in ["crates/websh-web/src/features/home", "assets/themes"] {
        if !root.join(directory).is_dir() {
            bail!("homepage source directory missing: {directory}");
        }
        let mut files = Vec::new();
        collect_files_recursive(&root.join(directory), &mut files)?;
        for file in files {
            paths.push(
                file.strip_prefix(root)
                    .context("homepage path outside project")?
                    .to_path_buf(),
            );
        }
    }
    let mut records = BTreeMap::new();
    for path in paths {
        let relative = path.to_str().context("homepage path must be UTF-8")?;
        let file = if let Some(record) = known.get(relative) {
            (*record).clone()
        } else {
            file_record(
                relative,
                &fs::read(root.join(&path))
                    .with_context(|| format!("read homepage source {relative}"))?,
            )
        };
        records.insert(relative.to_string(), file);
    }
    records.insert(
        ACK_ARTIFACT_PATH.to_string(),
        file_record(ACK_ARTIFACT_PATH, &artifact_bytes(ack)?),
    );
    Ok(records.into_values().collect())
}
