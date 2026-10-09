//! Typed attestation subjects.
//!
//! A subject binds a publication file set or a computed page projection.
//! Snapshot authentication and subject selection are owned by `publication`.

use serde::{Deserialize, Serialize};

use crate::engine::attestation::artifact::{Attestation, SUBJECT_MESSAGE_SCHEME, sha256_hex};

/// One file contributing to a subject's content fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// Fields shared by file-based subject variants.
///
/// Flattened into each variant via `#[serde(flatten)]` so the JSON shape is
/// `{ "kind": "...", "route": "...", "issued_at": "...", "content_files": [...], "attestations": [...], <variant fields> }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Envelope {
    pub route: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_at: Option<String>,
    pub content_files: Vec<ContentFile>,
    pub attestations: Vec<Attestation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentSubject {
    #[serde(flatten)]
    pub env: Envelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSubject {
    #[serde(flatten)]
    pub env: Envelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleSubject {
    #[serde(flatten)]
    pub env: Envelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectorySubject {
    #[serde(flatten)]
    pub env: Envelope,
}

/// A computed view binds a projection, never a synthetic file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewSubject {
    pub route: String,
    pub site: String,
    pub content_sha256: String,
    pub issued_at: Option<String>,
    pub attestations: Vec<Attestation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Subject {
    Document(DocumentSubject),
    Page(PageSubject),
    Bundle(BundleSubject),
    Directory(DirectorySubject),
    Home(ViewSubject),
    Ledger(ViewSubject),
}

impl Subject {
    fn files_envelope(&self) -> Option<&Envelope> {
        match self {
            Self::Document(s) => Some(&s.env),
            Self::Page(s) => Some(&s.env),
            Self::Bundle(s) => Some(&s.env),
            Self::Directory(s) => Some(&s.env),
            Self::Home(_) | Self::Ledger(_) => None,
        }
    }

    fn files_envelope_mut(&mut self) -> Option<&mut Envelope> {
        match self {
            Self::Document(s) => Some(&mut s.env),
            Self::Page(s) => Some(&mut s.env),
            Self::Bundle(s) => Some(&mut s.env),
            Self::Directory(s) => Some(&mut s.env),
            Self::Home(_) | Self::Ledger(_) => None,
        }
    }

    pub fn route(&self) -> &str {
        match self {
            Self::Home(s) | Self::Ledger(s) => &s.route,
            _ => &self.files_envelope().expect("file subject").route,
        }
    }

    pub fn issued_at(&self) -> Option<&str> {
        match self {
            Self::Home(s) | Self::Ledger(s) => s.issued_at.as_deref(),
            _ => self
                .files_envelope()
                .expect("file subject")
                .issued_at
                .as_deref(),
        }
    }

    pub fn set_issued_at(&mut self, issued_at: Option<String>) {
        match self {
            Self::Home(s) | Self::Ledger(s) => s.issued_at = issued_at,
            _ => self.files_envelope_mut().expect("file subject").issued_at = issued_at,
        }
    }

    pub fn content_files(&self) -> &[ContentFile] {
        self.files_envelope().map_or(&[], |env| &env.content_files)
    }

    pub fn attestations(&self) -> &[Attestation] {
        match self {
            Self::Home(s) | Self::Ledger(s) => &s.attestations,
            _ => &self.files_envelope().expect("file subject").attestations,
        }
    }

    pub fn attestations_mut(&mut self) -> &mut Vec<Attestation> {
        match self {
            Self::Home(s) | Self::Ledger(s) => &mut s.attestations,
            _ => {
                &mut self
                    .files_envelope_mut()
                    .expect("file subject")
                    .attestations
            }
        }
    }

    pub fn same_payload(&self, other: &Self) -> bool {
        let payload = |subject: &Self| {
            let mut subject = subject.clone();
            subject.set_issued_at(None);
            subject.attestations_mut().clear();
            subject
        };
        payload(self) == payload(other)
    }

    pub fn kind_str(&self) -> &'static str {
        match self {
            Subject::Document(_) => "document",
            Subject::Page(_) => "page",
            Subject::Bundle(_) => "bundle",
            Subject::Directory(_) => "directory",
            Subject::Home(_) => "home",
            Subject::Ledger(_) => "ledger",
        }
    }

    pub fn id(&self) -> String {
        subject_id_for_route(self.route())
    }

    pub fn content_sha256(&self) -> Result<String, SubjectCanonicalError> {
        match self {
            Self::Home(s) | Self::Ledger(s) => Ok(s.content_sha256.clone()),
            _ => compute_content_sha256(self.content_files()),
        }
    }

    /// The canonical text bound by attestation signatures.
    ///
    /// Field order is part of the contract — changes invalidate every existing signature.
    pub fn canonical_message(&self) -> Result<String, SubjectCanonicalError> {
        let id = self.id();
        let content_sha256 = self.content_sha256()?;
        let issued_at = self
            .issued_at()
            .ok_or(SubjectCanonicalError::MissingIssuance)?;
        let route = self.route();
        let kind = self.kind_str();
        let site = match self {
            Self::Home(s) | Self::Ledger(s) => format!("site={}\n", s.site),
            _ => String::new(),
        };
        let body = format!(
            "id={id}\n{site}route={route}\nkind={kind}\ncontent_sha256={content_sha256}\nissued_at={issued_at}"
        );
        Ok(format!("{SUBJECT_MESSAGE_SCHEME}\n{body}"))
    }

    /// Static structural checks that don't require disk or network access.
    ///
    /// - `content_files` must be strictly sorted by path with no duplicates
    /// - `canonical_message` must serialize without error
    pub fn validate(&self) -> Result<(), SubjectValidationError> {
        if crate::domain::VirtualPath::from_absolute(self.route()).is_err() {
            return Err(SubjectValidationError::Invalid(
                "noncanonical subject route",
            ));
        }
        if let Some(date) = self.issued_at()
            && (date.len() != 10
                || !date.bytes().enumerate().all(|(i, b)| {
                    if i == 4 || i == 7 {
                        b == b'-'
                    } else {
                        b.is_ascii_digit()
                    }
                }))
        {
            return Err(SubjectValidationError::Invalid(
                "invalid subject issuance date",
            ));
        }
        if let Self::Home(view) | Self::Ledger(view) = self
            && (view.site.is_empty()
                || view.site.len() > 256
                || view.site.chars().any(char::is_control)
                || view
                    .content_sha256
                    .strip_prefix("0x")
                    .is_none_or(|hash| crate::publication::ReleaseId::parse(hash).is_err()))
        {
            return Err(SubjectValidationError::Invalid("invalid view commitment"));
        }
        let mut last: Option<&str> = None;
        for file in self.content_files() {
            if let Some(prev) = last {
                if file.path.as_str() == prev {
                    return Err(SubjectValidationError::DuplicateContentPath {
                        path: file.path.clone(),
                    });
                }
                if file.path.as_str() < prev {
                    return Err(SubjectValidationError::UnsortedContentFiles {
                        previous: prev.to_string(),
                        current: file.path.clone(),
                    });
                }
            }
            last = Some(file.path.as_str());
        }
        self.content_sha256()?;
        if self.issued_at().is_some() || !self.attestations().is_empty() {
            self.canonical_message()?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SubjectCanonicalError {
    #[error("unsigned subject has no issuance date; prepare an explicit signing request")]
    MissingIssuance,
    #[error("serialize subject content files: {source}")]
    ContentFiles {
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum SubjectValidationError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("duplicate content path: {path}")]
    DuplicateContentPath { path: String },
    #[error("content_files not strictly sorted: {previous} > {current}")]
    UnsortedContentFiles { previous: String, current: String },
    #[error("canonical_message failed: {source}")]
    Canonical {
        #[from]
        source: SubjectCanonicalError,
    },
}

pub fn subject_id_for_route(route: &str) -> String {
    format!("route:{route}")
}

pub fn compute_content_sha256(files: &[ContentFile]) -> Result<String, SubjectCanonicalError> {
    serde_json::to_vec(files)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(|source| SubjectCanonicalError::ContentFiles { source })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_files() -> Vec<ContentFile> {
        vec![
            ContentFile {
                path: "a.txt".to_string(),
                sha256: "0xaaa".to_string(),
                bytes: 3,
            },
            ContentFile {
                path: "b.txt".to_string(),
                sha256: "0xbbb".to_string(),
                bytes: 4,
            },
        ]
    }

    fn document() -> Subject {
        Subject::Document(DocumentSubject {
            env: Envelope {
                route: "/keys/wonjae.asc".to_string(),
                issued_at: Some("2026-04-30".to_string()),
                content_files: sample_files(),
                attestations: Vec::new(),
            },
        })
    }

    fn page() -> Subject {
        Subject::Page(PageSubject {
            env: Envelope {
                route: "/papers/tabula".to_string(),
                issued_at: Some("2026-04-30".to_string()),
                content_files: sample_files(),
                attestations: Vec::new(),
            },
        })
    }

    fn bundle() -> Subject {
        Subject::Bundle(BundleSubject {
            env: Envelope {
                route: "/writing/foo".to_string(),
                issued_at: Some("2026-04-30".to_string()),
                content_files: sample_files(),
                attestations: Vec::new(),
            },
        })
    }

    fn directory() -> Subject {
        Subject::Directory(DirectorySubject {
            env: Envelope {
                route: "/.site".to_string(),
                issued_at: Some("2026-04-30".to_string()),
                content_files: sample_files(),
                attestations: Vec::new(),
            },
        })
    }

    #[test]
    fn canonical_message_document_is_exact() {
        let subject = document();
        let content_sha = subject.content_sha256().unwrap();
        let expected = format!(
            "websh.subject.v1\nid=route:/keys/wonjae.asc\nroute=/keys/wonjae.asc\nkind=document\ncontent_sha256={content_sha}\nissued_at=2026-04-30"
        );
        assert_eq!(subject.canonical_message().unwrap(), expected);
    }

    #[test]
    fn canonical_message_page_is_exact() {
        let subject = page();
        let content_sha = subject.content_sha256().unwrap();
        let expected = format!(
            "websh.subject.v1\nid=route:/papers/tabula\nroute=/papers/tabula\nkind=page\ncontent_sha256={content_sha}\nissued_at=2026-04-30"
        );
        assert_eq!(subject.canonical_message().unwrap(), expected);
    }

    #[test]
    fn canonical_message_bundle_is_exact() {
        let subject = bundle();
        let content_sha = subject.content_sha256().unwrap();
        let expected = format!(
            "websh.subject.v1\nid=route:/writing/foo\nroute=/writing/foo\nkind=bundle\ncontent_sha256={content_sha}\nissued_at=2026-04-30"
        );
        assert_eq!(subject.canonical_message().unwrap(), expected);
    }

    #[test]
    fn canonical_message_directory_is_exact() {
        let subject = directory();
        let content_sha = subject.content_sha256().unwrap();
        let expected = format!(
            "websh.subject.v1\nid=route:/.site\nroute=/.site\nkind=directory\ncontent_sha256={content_sha}\nissued_at=2026-04-30"
        );
        assert_eq!(subject.canonical_message().unwrap(), expected);
    }

    #[test]
    fn content_sha256_differs_when_files_differ() {
        let mut files = sample_files();
        let baseline = compute_content_sha256(&files).unwrap();
        files[0].bytes += 1;
        assert_ne!(compute_content_sha256(&files).unwrap(), baseline);
    }

    #[test]
    fn validate_rejects_unsorted_content_files() {
        let mut subject = document();
        subject.files_envelope_mut().unwrap().content_files = vec![
            ContentFile {
                path: "b.txt".to_string(),
                sha256: "0xbbb".to_string(),
                bytes: 4,
            },
            ContentFile {
                path: "a.txt".to_string(),
                sha256: "0xaaa".to_string(),
                bytes: 3,
            },
        ];
        assert!(subject.validate().is_err());
    }

    #[test]
    fn validate_rejects_duplicate_content_paths() {
        let mut subject = document();
        subject.files_envelope_mut().unwrap().content_files = vec![
            ContentFile {
                path: "a.txt".to_string(),
                sha256: "0xaaa".to_string(),
                bytes: 3,
            },
            ContentFile {
                path: "a.txt".to_string(),
                sha256: "0xbbb".to_string(),
                bytes: 4,
            },
        ];
        assert!(subject.validate().is_err());
    }

    #[test]
    fn subject_variants_round_trip_without_derived_fields() {
        for subject in [document(), page(), bundle(), directory()] {
            subject.validate().unwrap();
            let json = serde_json::to_value(&subject).unwrap();
            assert_eq!(json["kind"], subject.kind_str());
            for field in ["id", "content_sha256", "message"] {
                assert!(json.get(field).is_none(), "{}: {field}", subject.kind_str());
            }
            assert_eq!(serde_json::from_value::<Subject>(json).unwrap(), subject);
        }
    }

    #[test]
    fn serde_requires_attestations_field() {
        let json = r#"{
            "kind": "page",
            "route": "/papers/tabula",
            "issued_at": "2026-04-30",
            "content_files": []
        }"#;

        let parsed = serde_json::from_str::<Subject>(json);
        assert!(parsed.is_err());
    }
}
