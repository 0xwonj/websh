use super::hash::{
    EMPTY_PUBLIC_DOMAIN, combined_root, decode_hash, decode_hashes, hash_hex, normalize_ack_name,
    public_leaf_hash,
};
use super::model::{AckArtifact, AckError, AckMembershipProof};
use super::tree::{MerkleTree, verify_steps};

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
