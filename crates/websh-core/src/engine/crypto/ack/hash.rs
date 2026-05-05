use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use super::model::{AckError, Hash};

const PUBLIC_LEAF_DOMAIN: &[u8] = b"websh.ack.public.leaf.v1";
const PRIVATE_LEAF_DOMAIN: &[u8] = b"websh.ack.private.leaf.v1";
const NODE_DOMAIN: &[u8] = b"websh.ack.node.v1";
const COMBINED_DOMAIN: &[u8] = b"websh.ack.combined.v1";
pub(super) const EMPTY_PUBLIC_DOMAIN: &[u8] = b"websh.ack.public.empty.v1";
pub(super) const EMPTY_PRIVATE_DOMAIN: &[u8] = b"websh.ack.private.empty.v1";

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

pub(super) fn decode_hashes(values: &[String]) -> Result<Vec<Hash>, AckError> {
    values.iter().map(|value| decode_hash(value)).collect()
}

pub(super) fn hash_len_prefixed(domain: &[u8], payload: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u32).to_be_bytes());
    hasher.update(payload);
    hasher.finalize().into()
}

pub(super) fn node_hash(left: &Hash, right: &Hash) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(NODE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

pub(super) fn combined_root(
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

pub(super) fn empty_root(domain: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.finalize().into()
}
