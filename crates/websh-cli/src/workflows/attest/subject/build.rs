use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use websh_core::attestation::artifact::{
    AttestationArtifact, DocumentSubject, Envelope, HomepageSubject, LedgerSubject, PageSubject,
    Subject,
};
use websh_core::attestation::ledger::ContentLedger;
use websh_site::PUBLIC_KEY_PATH;

use crate::CliResult;
use crate::workflows::content::{build_content_files, collect_files_recursive};

use super::super::{DEFAULT_HOMEPAGE_CONTENT, today_utc};
use super::artifact::read_ack;
use super::types::SubjectKind;

pub(in crate::workflows::attest) fn build_subject(
    root: &Path,
    existing: &AttestationArtifact,
    route: String,
    kind: SubjectKind,
    content_paths: Vec<PathBuf>,
    issued_at: Option<String>,
    ledger: Option<&ContentLedger>,
) -> CliResult<Subject> {
    let content_files = build_content_files(root, &content_paths)?;
    let issued_at = issued_at
        .or_else(|| {
            existing
                .subject_for_route(&route)
                .map(|subject| subject.issued_at().to_string())
        })
        .unwrap_or_else(today_utc);

    let env = Envelope {
        route: route.clone(),
        issued_at,
        content_files,
        attestations: Vec::new(),
    };

    let mut subject = match kind {
        SubjectKind::Homepage => {
            let ack = read_ack(root)?;
            Subject::Homepage(HomepageSubject {
                env,
                ack_combined_root: ack.combined_root,
            })
        }
        SubjectKind::Ledger => {
            let ledger =
                ledger.ok_or("ledger subject requires a ContentLedger to bind chain_head")?;
            Subject::Ledger(LedgerSubject {
                env,
                chain_head: ledger.chain_head.clone(),
            })
        }
        SubjectKind::Document => Subject::Document(DocumentSubject { env }),
        SubjectKind::Page => Subject::Page(PageSubject { env }),
    };

    if let Some(prior) = existing.subject_for_route(&route)
        && let (Ok(prior_msg), Ok(new_msg)) =
            (prior.canonical_message(), subject.canonical_message())
        && prior_msg == new_msg
    {
        subject
            .attestations_mut()
            .extend(prior.attestations().iter().cloned());
    }

    Ok(subject)
}

pub(in crate::workflows::attest) fn content_paths_or_default(
    root: &Path,
    route: &str,
    kind: SubjectKind,
    paths: Vec<PathBuf>,
) -> CliResult<Vec<PathBuf>> {
    let raw = if paths.is_empty() {
        if !matches!(kind, SubjectKind::Homepage) || route != "/" {
            return Err("non-homepage subjects require at least one --content path".into());
        }
        let mut defaults = DEFAULT_HOMEPAGE_CONTENT
            .iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        if root.join(PUBLIC_KEY_PATH).exists() {
            defaults.push(PathBuf::from(PUBLIC_KEY_PATH));
        }
        defaults
    } else {
        paths
    };
    expand_content_paths(root, raw)
}

/// Expand `paths` so each directory entry is replaced by the recursive list
/// of files it contains. File entries pass through unchanged. Order is
/// preserved across the input list, with files inside an expanded directory
/// emitted in the canonical sort order produced by
/// `manifest::collect_files_recursive`. Duplicates (same canonical
/// filesystem location reached via multiple input paths) are dropped.
fn expand_content_paths(root: &Path, raw_paths: Vec<PathBuf>) -> CliResult<Vec<PathBuf>> {
    let mut seen = BTreeSet::new();
    let mut expanded = Vec::new();
    for path in raw_paths {
        let abs = if path.is_absolute() {
            path.clone()
        } else {
            root.join(&path)
        };
        if abs.is_dir() {
            let mut files = Vec::new();
            collect_files_recursive(&abs, &mut files)?;
            for file in files {
                let key = file.canonicalize().unwrap_or_else(|_| file.clone());
                if seen.insert(key) {
                    expanded.push(file);
                }
            }
        } else if abs.is_file() {
            let key = abs.canonicalize().unwrap_or_else(|_| abs.clone());
            if seen.insert(key) {
                expanded.push(path);
            }
        } else {
            return Err(format!("attestation content path not found: {}", path.display()).into());
        }
    }
    Ok(expanded)
}
