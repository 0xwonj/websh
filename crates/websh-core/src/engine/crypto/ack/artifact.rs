use std::collections::HashSet;

use super::hash::{
    EMPTY_PRIVATE_DOMAIN, EMPTY_PUBLIC_DOMAIN, combined_root, decode_hash, decode_hashes, hash_hex,
    normalize_ack_name, private_leaf_hash, public_leaf_hash,
};
use super::model::{
    ACK_HASH, ACK_NORMALIZATION, ACK_SCHEME, AckArtifact, AckEntryMode, AckError, AckPrivateSource,
    PrivateAckArtifact, PublicAckArtifact,
};
use super::tree::MerkleTree;

impl AckArtifact {
    pub fn from_json_str(body: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(body)
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
