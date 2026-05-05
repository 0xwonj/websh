//! Hybrid public/private acknowledgement commitment.

mod artifact;
mod hash;
mod model;
mod proof;
mod receipt;
mod slug;
#[cfg(test)]
mod tests;
mod tree;

pub use artifact::build_artifact_from_source;
pub use hash::{
    decode_hash, hash_hex, normalize_ack_name, private_leaf_hash, public_leaf_hash, short_hash,
};
pub use model::{
    ACK_HASH, ACK_LOCAL_SOURCE_PATH, ACK_NORMALIZATION, ACK_RECEIPT_SCHEME, ACK_RECEIPTS_DIR,
    ACK_SCHEME, AckArtifact, AckEntryMode, AckError, AckMembershipProof, AckPrivateSource,
    AckProofStep, AckReceipt, AckReceiptProofStep, AckReceiptVerification, AckSourceEntry, Hash,
    PrivateAckArtifact, PublicAckArtifact,
};
pub use proof::public_proof_for_name;
pub use receipt::{private_receipt_from_source, verify_private_receipt};
pub use slug::slugify_name;
