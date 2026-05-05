use super::artifact::build_artifact_from_source;
use super::hash::{
    EMPTY_PRIVATE_DOMAIN, combined_root, decode_hash, hash_hex, normalize_ack_name,
    private_leaf_hash,
};
use super::model::{
    ACK_RECEIPT_SCHEME, AckArtifact, AckEntryMode, AckError, AckPrivateSource, AckProofStep,
    AckReceipt, AckReceiptProofStep, AckReceiptVerification,
};
use super::tree::{MerkleTree, verify_steps};

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
