use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Hash = [u8; 32];

pub const ACK_LOCAL_SOURCE_PATH: &str = ".websh/local/crypto/ack.private.json";
pub const ACK_RECEIPTS_DIR: &str = ".websh/local/crypto/ack-receipts";

pub const ACK_SCHEME: &str = "websh.ack.hybrid.v1";
pub const ACK_RECEIPT_SCHEME: &str = "websh.ack.private.receipt.v1";
pub const ACK_NORMALIZATION: &str = "nfkc+trim+collapse-whitespace+lowercase";
pub const ACK_HASH: &str = "sha256";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckArtifact {
    pub version: u32,
    pub scheme: String,
    pub hash: String,
    pub normalization: String,
    pub public: PublicAckArtifact,
    pub private: PrivateAckArtifact,
    pub combined_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicAckArtifact {
    pub count: usize,
    pub root: String,
    pub leaves: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateAckArtifact {
    pub count: usize,
    pub root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckPrivateSource {
    pub version: u32,
    pub entries: Vec<AckSourceEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckSourceEntry {
    pub mode: AckEntryMode,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AckEntryMode {
    Public,
    Private,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckReceipt {
    pub version: u32,
    pub scheme: String,
    pub name: String,
    pub nonce: String,
    pub leaf: String,
    pub proof: Vec<AckReceiptProofStep>,
    pub private_root: String,
    pub combined_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AckReceiptProofStep {
    pub side: String,
    pub sibling: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AckMembershipProof {
    pub idx: usize,
    pub target: String,
    pub name: String,
    pub leaf_hex: String,
    pub steps: Vec<AckProofStep>,
    pub recomputed_hex: String,
    pub committed_hex: String,
    pub tree_root_hex: String,
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AckProofStep {
    pub number: usize,
    pub side: String,
    pub sibling_hex: String,
    pub parent_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AckReceiptVerification {
    pub leaf_hex: String,
    pub private_root: String,
    pub combined_root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum AckError {
    #[error("invalid ACK artifact: {0}")]
    InvalidArtifact(String),
    #[error("invalid ACK source: {0}")]
    InvalidSource(String),
    #[error("invalid ACK receipt: {0}")]
    InvalidReceipt(String),
    #[error("invalid hex: {0}")]
    InvalidHex(String),
    #[error("private ACK entry not found: {0}")]
    MissingPrivateEntry(String),
}

impl Default for AckPrivateSource {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl AckMembershipProof {
    pub fn side_path(&self) -> String {
        if self.steps.is_empty() {
            return "(singleton)".to_string();
        }
        self.steps
            .iter()
            .map(|step| step.side.as_str())
            .collect::<Vec<_>>()
            .join(" → ")
    }
}
