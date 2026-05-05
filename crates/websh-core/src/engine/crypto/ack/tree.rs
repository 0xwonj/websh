use super::hash::{decode_hash, empty_root, hash_hex, node_hash};
use super::model::{AckError, AckProofStep, Hash};

pub(super) fn verify_steps(leaf: Hash, steps: &[AckProofStep]) -> Result<Hash, AckError> {
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

pub(super) struct MerkleTree {
    pub(super) leaves: Vec<Hash>,
    levels: Vec<Vec<Hash>>,
    pub(super) root: Hash,
}

impl MerkleTree {
    pub(super) fn new(mut leaves: Vec<Hash>, empty_domain: &[u8]) -> Self {
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

    pub(super) fn proof(&self, idx: usize) -> Vec<AckProofStep> {
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
