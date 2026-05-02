//! Hybrid public/private acknowledgement commitment.
//!
//! Public acknowledgements are verifiable by name. Private acknowledgements are
//! verifiable only with a receipt that contains the private nonce and proof.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

pub type Hash = [u8; 32];

pub const ACK_ARTIFACT_PATH: &str = "assets/crypto/ack.commitment.json";
pub const ACK_LOCAL_SOURCE_PATH: &str = ".websh/local/crypto/ack.private.json";
pub const ACK_RECEIPTS_DIR: &str = ".websh/local/crypto/ack-receipts";

pub const ACK_SCHEME: &str = "websh.ack.hybrid.v1";
pub const ACK_RECEIPT_SCHEME: &str = "websh.ack.private.receipt.v1";
pub const ACK_NORMALIZATION: &str = "nfkc+trim+collapse-whitespace+lowercase";
pub const ACK_HASH: &str = "sha256";

const PUBLIC_LEAF_DOMAIN: &[u8] = b"websh.ack.public.leaf.v1";
const PRIVATE_LEAF_DOMAIN: &[u8] = b"websh.ack.private.leaf.v1";
const NODE_DOMAIN: &[u8] = b"websh.ack.node.v1";
const COMBINED_DOMAIN: &[u8] = b"websh.ack.combined.v1";
const EMPTY_PUBLIC_DOMAIN: &[u8] = b"websh.ack.public.empty.v1";
const EMPTY_PRIVATE_DOMAIN: &[u8] = b"websh.ack.private.empty.v1";

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AckError {
    InvalidArtifact(String),
    InvalidSource(String),
    InvalidReceipt(String),
    InvalidHex(String),
    MissingPrivateEntry(String),
}

impl fmt::Display for AckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArtifact(message) => write!(f, "invalid ACK artifact: {message}"),
            Self::InvalidSource(message) => write!(f, "invalid ACK source: {message}"),
            Self::InvalidReceipt(message) => write!(f, "invalid ACK receipt: {message}"),
            Self::InvalidHex(message) => write!(f, "invalid hex: {message}"),
            Self::MissingPrivateEntry(name) => write!(f, "private ACK entry not found: {name}"),
        }
    }
}

impl std::error::Error for AckError {}

impl Default for AckPrivateSource {
    fn default() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }
}

impl AckArtifact {
    pub fn from_homepage_asset() -> Result<Self, serde_json::Error> {
        serde_json::from_str(include_str!(
            "../../../../assets/crypto/ack.commitment.json"
        ))
    }

    pub fn count(&self) -> usize {
        self.public.count + self.private.count
    }

    pub fn validate(&self) -> Result<(), AckError> {
        if self.version != 1 {
            return Err(AckError::InvalidArtifact(format!(
                "unsupported version {}",
                self.version
            )));
        }
        if self.scheme != ACK_SCHEME {
            return Err(AckError::InvalidArtifact(format!(
                "unsupported scheme {}",
                self.scheme
            )));
        }
        if self.hash != ACK_HASH {
            return Err(AckError::InvalidArtifact(format!(
                "unsupported hash {}",
                self.hash
            )));
        }
        if self.normalization != ACK_NORMALIZATION {
            return Err(AckError::InvalidArtifact(format!(
                "unsupported normalization {}",
                self.normalization
            )));
        }
        if self.public.count != self.public.leaves.len() {
            return Err(AckError::InvalidArtifact(format!(
                "public count {} does not match {} leaves",
                self.public.count,
                self.public.leaves.len()
            )));
        }

        let public_leaves = decode_hashes(&self.public.leaves)?;
        let public_tree = MerkleTree::new(public_leaves, EMPTY_PUBLIC_DOMAIN);
        let public_root = hash_hex(&public_tree.root);
        if self.public.root != public_root {
            return Err(AckError::InvalidArtifact(format!(
                "public root mismatch: expected {}, got {}",
                self.public.root, public_root
            )));
        }

        let private_root = decode_hash(&self.private.root)?;
        let combined = combined_root(
            public_tree.root,
            self.public.count,
            private_root,
            self.private.count,
        );
        let combined_hex = hash_hex(&combined);
        if self.combined_root != combined_hex {
            return Err(AckError::InvalidArtifact(format!(
                "combined root mismatch: expected {}, got {}",
                self.combined_root, combined_hex
            )));
        }

        Ok(())
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

pub fn normalize_ack_name(raw: &str) -> String {
    let nfkc = raw.nfkc().collect::<String>();
    nfkc.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub fn public_leaf_hash(name: &str) -> Hash {
    let normalized = normalize_ack_name(name);
    hash_len_prefixed(PUBLIC_LEAF_DOMAIN, normalized.as_bytes())
}

pub fn private_leaf_hash(name: &str, nonce: &Hash) -> Hash {
    let normalized = normalize_ack_name(name);
    let mut hasher = Sha256::new();
    hasher.update(PRIVATE_LEAF_DOMAIN);
    hasher.update(nonce);
    hasher.update((normalized.len() as u32).to_be_bytes());
    hasher.update(normalized.as_bytes());
    hasher.finalize().into()
}

pub fn build_artifact_from_source(source: &AckPrivateSource) -> Result<AckArtifact, AckError> {
    if source.version != 1 {
        return Err(AckError::InvalidSource(format!(
            "unsupported version {}",
            source.version
        )));
    }

    let mut seen = HashSet::new();
    let mut public_leaves = Vec::new();
    let mut private_leaves = Vec::new();

    for entry in &source.entries {
        let normalized = normalize_ack_name(&entry.name);
        if normalized.is_empty() {
            return Err(AckError::InvalidSource("empty ACK name".to_string()));
        }
        if !seen.insert(normalized.clone()) {
            return Err(AckError::InvalidSource(format!(
                "duplicate ACK name after normalization: {normalized}"
            )));
        }

        match entry.mode {
            AckEntryMode::Public => public_leaves.push(public_leaf_hash(&entry.name)),
            AckEntryMode::Private => {
                let nonce_hex = entry.nonce.as_deref().ok_or_else(|| {
                    AckError::InvalidSource(format!("private entry missing nonce: {}", entry.name))
                })?;
                private_leaves.push(private_leaf_hash(&entry.name, &decode_hash(nonce_hex)?));
            }
        }
    }

    public_leaves.sort();
    private_leaves.sort();
    let public_tree = MerkleTree::new(public_leaves, EMPTY_PUBLIC_DOMAIN);
    let private_tree = MerkleTree::new(private_leaves, EMPTY_PRIVATE_DOMAIN);
    let combined = combined_root(
        public_tree.root,
        source
            .entries
            .iter()
            .filter(|entry| entry.mode == AckEntryMode::Public)
            .count(),
        private_tree.root,
        source
            .entries
            .iter()
            .filter(|entry| entry.mode == AckEntryMode::Private)
            .count(),
    );

    Ok(AckArtifact {
        version: 1,
        scheme: ACK_SCHEME.to_string(),
        hash: ACK_HASH.to_string(),
        normalization: ACK_NORMALIZATION.to_string(),
        public: PublicAckArtifact {
            count: public_tree.leaves.len(),
            root: hash_hex(&public_tree.root),
            leaves: public_tree.leaves.iter().map(hash_hex).collect(),
        },
        private: PrivateAckArtifact {
            count: private_tree.leaves.len(),
            root: hash_hex(&private_tree.root),
        },
        combined_root: hash_hex(&combined),
    })
}

pub fn public_proof_for_name(
    artifact: &AckArtifact,
    raw: &str,
) -> Result<Option<AckMembershipProof>, AckError> {
    artifact.validate()?;
    let target = normalize_ack_name(raw);
    if target.is_empty() {
        return Ok(None);
    }

    let leaf = public_leaf_hash(&target);
    let leaves = decode_hashes(&artifact.public.leaves)?;
    let tree = MerkleTree::new(leaves, EMPTY_PUBLIC_DOMAIN);
    let Some(idx) = tree.leaves.iter().position(|candidate| *candidate == leaf) else {
        return Ok(None);
    };
    let steps = tree.proof(idx);
    let tree_root = verify_steps(leaf, &steps)?;
    let combined = combined_root(
        tree_root,
        artifact.public.count,
        decode_hash(&artifact.private.root)?,
        artifact.private.count,
    );
    let combined_hex = hash_hex(&combined);

    Ok(Some(AckMembershipProof {
        idx,
        target: target.clone(),
        name: target,
        leaf_hex: hash_hex(&leaf),
        steps,
        recomputed_hex: combined_hex.clone(),
        committed_hex: artifact.combined_root.clone(),
        tree_root_hex: hash_hex(&tree_root),
        verified: combined_hex == artifact.combined_root,
    }))
}

pub fn private_receipt_from_source(
    source: &AckPrivateSource,
    raw: &str,
) -> Result<AckReceipt, AckError> {
    let target = normalize_ack_name(raw);
    let private_entries = source
        .entries
        .iter()
        .filter(|entry| entry.mode == AckEntryMode::Private)
        .collect::<Vec<_>>();
    let entry = private_entries
        .iter()
        .copied()
        .find(|entry| normalize_ack_name(&entry.name) == target)
        .ok_or_else(|| AckError::MissingPrivateEntry(raw.to_string()))?;

    let mut private_leaves = Vec::new();
    let mut target_leaf = None;
    for private in private_entries {
        let nonce = decode_hash(private.nonce.as_deref().ok_or_else(|| {
            AckError::InvalidSource(format!("private entry missing nonce: {}", private.name))
        })?)?;
        let leaf = private_leaf_hash(&private.name, &nonce);
        if normalize_ack_name(&private.name) == target {
            target_leaf = Some(leaf);
        }
        private_leaves.push(leaf);
    }

    private_leaves.sort();
    let private_tree = MerkleTree::new(private_leaves, EMPTY_PRIVATE_DOMAIN);
    let leaf = target_leaf.expect("target leaf was set from selected private entry");
    let idx = private_tree
        .leaves
        .iter()
        .position(|candidate| *candidate == leaf)
        .ok_or_else(|| AckError::MissingPrivateEntry(raw.to_string()))?;
    let proof = private_tree
        .proof(idx)
        .into_iter()
        .map(|step| AckReceiptProofStep {
            side: step.side,
            sibling: step.sibling_hex,
        })
        .collect::<Vec<_>>();
    let artifact = build_artifact_from_source(source)?;

    Ok(AckReceipt {
        version: 1,
        scheme: ACK_RECEIPT_SCHEME.to_string(),
        name: entry.name.clone(),
        nonce: entry
            .nonce
            .clone()
            .expect("private entry nonce was validated above"),
        leaf: hash_hex(&leaf),
        proof,
        private_root: hash_hex(&private_tree.root),
        combined_root: artifact.combined_root,
    })
}

pub fn verify_private_receipt(
    artifact: &AckArtifact,
    receipt: &AckReceipt,
) -> Result<AckReceiptVerification, AckError> {
    artifact.validate()?;
    if receipt.version != 1 {
        return Err(AckError::InvalidReceipt(format!(
            "unsupported version {}",
            receipt.version
        )));
    }
    if receipt.scheme != ACK_RECEIPT_SCHEME {
        return Err(AckError::InvalidReceipt(format!(
            "unsupported scheme {}",
            receipt.scheme
        )));
    }

    let nonce = decode_hash(&receipt.nonce)?;
    let leaf = private_leaf_hash(&receipt.name, &nonce);
    let receipt_leaf = decode_hash(&receipt.leaf)?;
    if leaf != receipt_leaf {
        return Err(AckError::InvalidReceipt("leaf mismatch".to_string()));
    }

    let steps = receipt
        .proof
        .iter()
        .enumerate()
        .map(|(idx, step)| {
            Ok(AckProofStep {
                number: idx + 1,
                side: step.side.clone(),
                sibling_hex: step.sibling.clone(),
                parent_hex: String::new(),
            })
        })
        .collect::<Result<Vec<_>, AckError>>()?;
    let private_root = verify_steps(leaf, &steps)?;
    let private_root_hex = hash_hex(&private_root);
    if private_root_hex != artifact.private.root || private_root_hex != receipt.private_root {
        return Err(AckError::InvalidReceipt(
            "private root mismatch".to_string(),
        ));
    }

    let public_root = decode_hash(&artifact.public.root)?;
    let combined = combined_root(
        public_root,
        artifact.public.count,
        private_root,
        artifact.private.count,
    );
    let combined_hex = hash_hex(&combined);
    if combined_hex != artifact.combined_root || combined_hex != receipt.combined_root {
        return Err(AckError::InvalidReceipt(
            "combined root mismatch".to_string(),
        ));
    }

    Ok(AckReceiptVerification {
        leaf_hex: hash_hex(&leaf),
        private_root: private_root_hex,
        combined_root: combined_hex,
    })
}

pub fn decode_hash(hex_value: &str) -> Result<Hash, AckError> {
    let trimmed = hex_value.strip_prefix("0x").unwrap_or(hex_value);
    let bytes = hex::decode(trimmed)
        .map_err(|error| AckError::InvalidHex(format!("{hex_value}: {error}")))?;
    let hash: Hash = bytes.try_into().map_err(|bytes: Vec<u8>| {
        AckError::InvalidHex(format!("expected 32 bytes, got {}", bytes.len()))
    })?;
    Ok(hash)
}

pub fn hash_hex(hash: &Hash) -> String {
    format!("0x{}", hex::encode(hash))
}

pub fn short_hash(hash: &str) -> String {
    if hash.len() <= 26 {
        return hash.to_string();
    }
    format!("{}…{}", &hash[..18], &hash[hash.len() - 8..])
}

pub fn slugify_name(name: &str) -> String {
    let mut out = String::new();
    let normalized = normalize_ack_name(name);
    for ch in normalized.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let slug = out.trim_matches('-');
    let digest = receipt_filename_digest(&normalized);
    let suffix = &digest[..16];
    if slug.is_empty() {
        format!("ack-{suffix}")
    } else {
        format!("{slug}-{suffix}")
    }
}

fn receipt_filename_digest(normalized: &str) -> String {
    hash_len_prefixed(b"websh.ack.receipt.filename.v1", normalized.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_hashes(values: &[String]) -> Result<Vec<Hash>, AckError> {
    values.iter().map(|value| decode_hash(value)).collect()
}

fn hash_len_prefixed(domain: &[u8], payload: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u32).to_be_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

fn node_hash(left: &Hash, right: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(NODE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn combined_root(
    public_root: Hash,
    public_count: usize,
    private_root: Hash,
    private_count: usize,
) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(COMBINED_DOMAIN);
    hasher.update((public_count as u32).to_be_bytes());
    hasher.update(public_root);
    hasher.update((private_count as u32).to_be_bytes());
    hasher.update(private_root);
    hasher.finalize().into()
}

fn empty_root(domain: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.finalize().into()
}

fn verify_steps(leaf: Hash, steps: &[AckProofStep]) -> Result<Hash, AckError> {
    let mut acc = leaf;
    for step in steps {
        let sibling = decode_hash(&step.sibling_hex)?;
        acc = match step.side.as_str() {
            "L" => node_hash(&sibling, &acc),
            "R" => node_hash(&acc, &sibling),
            side => {
                return Err(AckError::InvalidReceipt(format!(
                    "invalid proof side {side}"
                )));
            }
        };
    }
    Ok(acc)
}

struct MerkleTree {
    leaves: Vec<Hash>,
    levels: Vec<Vec<Hash>>,
    root: Hash,
}

impl MerkleTree {
    fn new(mut leaves: Vec<Hash>, empty_domain: &[u8]) -> Self {
        leaves.sort();
        let mut levels = vec![leaves.clone()];
        if leaves.is_empty() {
            return Self {
                leaves,
                levels,
                root: empty_root(empty_domain),
            };
        }

        while levels.last().is_some_and(|level| level.len() > 1) {
            let mut current = levels.last().cloned().unwrap_or_default();
            if current.len() % 2 == 1
                && let Some(last) = current.last().copied()
            {
                current.push(last);
            }
            let next = current
                .chunks_exact(2)
                .map(|pair| node_hash(&pair[0], &pair[1]))
                .collect::<Vec<_>>();
            levels.push(next);
        }

        let root = levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .unwrap_or_else(|| empty_root(empty_domain));

        Self {
            leaves,
            levels,
            root,
        }
    }

    fn proof(&self, idx: usize) -> Vec<AckProofStep> {
        let mut cursor = idx;
        let mut acc = self.leaves[idx];
        let mut steps = Vec::new();

        for (level_idx, level) in self.levels.iter().take(self.levels.len() - 1).enumerate() {
            let mut current = level.clone();
            if current.len() % 2 == 1
                && let Some(last) = current.last().copied()
            {
                current.push(last);
            }

            let is_right = cursor % 2 == 1;
            let sibling_idx = if is_right { cursor - 1 } else { cursor + 1 };
            let sibling = current[sibling_idx];
            let (left, right, side) = if is_right {
                (sibling, acc, "L")
            } else {
                (acc, sibling, "R")
            };
            acc = node_hash(&left, &right);
            steps.push(AckProofStep {
                number: level_idx + 1,
                side: side.to_string(),
                sibling_hex: hash_hex(&sibling),
                parent_hex: hash_hex(&acc),
            });
            cursor >>= 1;
        }

        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_with_private() -> AckPrivateSource {
        AckPrivateSource {
            version: 1,
            entries: vec![
                AckSourceEntry {
                    mode: AckEntryMode::Public,
                    name: "Coffee".to_string(),
                    nonce: None,
                },
                AckSourceEntry {
                    mode: AckEntryMode::Private,
                    name: "Anonymous Reviewer".to_string(),
                    nonce: Some(
                        "0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                            .to_string(),
                    ),
                },
            ],
        }
    }

    #[test]
    fn normalization_is_stable() {
        assert_eq!(normalize_ack_name("  COFFEE\t\nhouse  "), "coffee house");
        assert_eq!(normalize_ack_name("Ａｄｖｉｓｏｒ"), "advisor");
        assert_eq!(normalize_ack_name("  홍길동\t님  "), "홍길동 님");
    }

    #[test]
    fn receipt_filename_slug_is_ascii_and_hash_suffixed() {
        let ascii = slugify_name("Anonymous Reviewer");
        assert!(ascii.starts_with("anonymous-reviewer-"));
        assert!(
            ascii
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        );

        let korean = slugify_name("홍길동");
        assert!(korean.starts_with("ack-"));
        assert!(
            korean
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        );
        assert_ne!(korean, "ack-");
    }

    #[test]
    fn empty_artifact_is_stable_and_valid() {
        let artifact = build_artifact_from_source(&AckPrivateSource::default()).unwrap();
        assert_eq!(artifact.public.count, 0);
        assert_eq!(artifact.private.count, 0);
        artifact.validate().unwrap();
    }

    #[test]
    fn public_name_verifies_without_plaintext_artifact() {
        let artifact = build_artifact_from_source(&source_with_private()).unwrap();
        let proof = public_proof_for_name(&artifact, " coffee ")
            .unwrap()
            .expect("public proof");
        assert!(proof.verified);
        assert!(
            public_proof_for_name(&artifact, "anonymous reviewer")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn unicode_ack_names_verify() {
        let source = AckPrivateSource {
            version: 1,
            entries: vec![
                AckSourceEntry {
                    mode: AckEntryMode::Public,
                    name: "홍길동".to_string(),
                    nonce: None,
                },
                AckSourceEntry {
                    mode: AckEntryMode::Private,
                    name: "익명 리뷰어".to_string(),
                    nonce: Some(
                        "0x000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                            .to_string(),
                    ),
                },
            ],
        };
        let artifact = build_artifact_from_source(&source).unwrap();

        let proof = public_proof_for_name(&artifact, " 홍길동 ")
            .unwrap()
            .expect("public proof");
        assert!(proof.verified);

        let receipt = private_receipt_from_source(&source, "익명 리뷰어").unwrap();
        let verification = verify_private_receipt(&artifact, &receipt).unwrap();
        assert_eq!(verification.combined_root, artifact.combined_root);
    }

    #[test]
    fn private_receipt_verifies() {
        let source = source_with_private();
        let artifact = build_artifact_from_source(&source).unwrap();
        let receipt = private_receipt_from_source(&source, "anonymous reviewer").unwrap();
        let verification = verify_private_receipt(&artifact, &receipt).unwrap();
        assert_eq!(verification.combined_root, artifact.combined_root);
    }

    #[test]
    fn altered_private_receipt_fails() {
        let source = source_with_private();
        let artifact = build_artifact_from_source(&source).unwrap();
        let mut receipt = private_receipt_from_source(&source, "anonymous reviewer").unwrap();
        receipt.nonce =
            "0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe"
                .to_string();
        assert!(verify_private_receipt(&artifact, &receipt).is_err());
    }
}
