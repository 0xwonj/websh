//! Canonical homepage and page-level attestation data.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const ATTESTATIONS_PATH: &str = "assets/crypto/attestations.json";
pub const ATTESTATIONS_SCHEME: &str = "websh.attestations.v1";
pub const SUBJECT_MESSAGE_SCHEME: &str = "websh.subject.v1";
pub const CONTENT_HASH: &str = "sha256";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationArtifact {
    pub version: u32,
    pub scheme: String,
    pub subjects: Vec<AttestationSubject>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationSubject {
    pub id: String,
    pub route: String,
    pub kind: String,
    pub content: SubjectContent,
    pub content_sha256: String,
    pub ack_combined_root: String,
    pub issued_at: String,
    pub message: String,
    #[serde(default)]
    pub attestations: Vec<SubjectAttestation>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectContent {
    pub hash: String,
    pub files: Vec<SubjectContentFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubjectContentFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SubjectAttestation {
    Pgp {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signer: Option<String>,
        fingerprint: String,
        key_path: String,
        signature: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        signature_path: Option<String>,
        message_sha256: String,
        verified: bool,
    },
    Ethereum {
        scheme: String,
        signer: String,
        address: String,
        signature: String,
        recovered_address: String,
        message_sha256: String,
        verified: bool,
    },
}

impl Default for AttestationArtifact {
    fn default() -> Self {
        Self {
            version: 1,
            scheme: ATTESTATIONS_SCHEME.to_string(),
            subjects: Vec::new(),
        }
    }
}

impl AttestationArtifact {
    pub fn from_homepage_asset() -> Result<Self, serde_json::Error> {
        serde_json::from_str(include_str!("../../assets/crypto/attestations.json"))
    }

    pub fn subject_for_route(&self, route: &str) -> Option<&AttestationSubject> {
        self.subjects.iter().find(|subject| subject.route == route)
    }

    pub fn subject_for_route_mut(&mut self, route: &str) -> Option<&mut AttestationSubject> {
        self.subjects
            .iter_mut()
            .find(|subject| subject.route == route)
    }

    pub fn validate_header(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("unsupported attestations version {}", self.version));
        }
        if self.scheme != ATTESTATIONS_SCHEME {
            return Err(format!("unsupported attestations scheme {}", self.scheme));
        }
        Ok(())
    }
}

impl AttestationSubject {
    pub fn expected_message(&self) -> String {
        canonical_subject_message(
            &self.id,
            &self.route,
            &self.kind,
            &self.content_sha256,
            &self.ack_combined_root,
            &self.issued_at,
        )
    }
}

impl SubjectAttestation {
    pub fn message_sha256(&self) -> &str {
        match self {
            Self::Pgp { message_sha256, .. } | Self::Ethereum { message_sha256, .. } => {
                message_sha256
            }
        }
    }

    pub fn verified(&self) -> bool {
        match self {
            Self::Pgp { verified, .. } | Self::Ethereum { verified, .. } => *verified,
        }
    }
}

pub fn subject_id_for_route(route: &str) -> String {
    format!("route:{route}")
}

pub fn canonical_subject_message(
    id: &str,
    route: &str,
    kind: &str,
    content_sha256: &str,
    ack_combined_root: &str,
    issued_at: &str,
) -> String {
    format!(
        "{SUBJECT_MESSAGE_SCHEME}\nid={id}\nroute={route}\nkind={kind}\ncontent_sha256={content_sha256}\nack_combined_root={ack_combined_root}\nissued_at={issued_at}"
    )
}

pub fn compute_content_sha256(content: &SubjectContent) -> Result<String, serde_json::Error> {
    serde_json::to_vec(content).map(|bytes| sha256_hex(&bytes))
}

pub fn message_sha256(message: &str) -> String {
    sha256_hex(message.as_bytes())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("0x{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_subject_message_is_exact_and_deterministic() {
        let first = canonical_subject_message(
            "route:/",
            "/",
            "homepage",
            "0xcontent",
            "0xack",
            "2026-04-26",
        );
        let second = canonical_subject_message(
            "route:/",
            "/",
            "homepage",
            "0xcontent",
            "0xack",
            "2026-04-26",
        );
        assert_eq!(first, second);
        assert_eq!(
            first,
            "websh.subject.v1\nid=route:/\nroute=/\nkind=homepage\ncontent_sha256=0xcontent\nack_combined_root=0xack\nissued_at=2026-04-26"
        );
    }

    #[test]
    fn content_sha256_is_stable_for_same_file_set() {
        let content = SubjectContent {
            hash: CONTENT_HASH.to_string(),
            files: vec![
                SubjectContentFile {
                    path: "a.txt".to_string(),
                    sha256: "0xaaa".to_string(),
                    bytes: 3,
                },
                SubjectContentFile {
                    path: "b.txt".to_string(),
                    sha256: "0xbbb".to_string(),
                    bytes: 4,
                },
            ],
        };
        assert_eq!(
            compute_content_sha256(&content).unwrap(),
            compute_content_sha256(&content).unwrap()
        );
    }
}
