//! Deterministic ledger presentation of a signed publication catalog.
//!
//! This is a projection of one manifest, not a separately stored ledger or a claim
//! that successive releases preserve an append-only history.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::domain::ContentManifestEntry;

use super::{Manifest, ReleaseError, ReleaseId};

const GENESIS_HASH: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationChain {
    /// Oldest first. Filtering the view must not renumber or rehash these blocks.
    pub blocks: Vec<PublicationBlock>,
    pub head: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationBlock {
    pub path: String,
    pub height: u64,
    pub hash: String,
    pub previous_hash: String,
    pub content_bytes: u64,
}

#[derive(Serialize)]
struct BlockCommitment<'a> {
    // This domain identifies the cryptographic encoding, not an application version.
    scheme: &'static str,
    site: &'a str,
    height: u64,
    previous_hash: &'a str,
    path: &'a str,
    entries: Vec<&'a ContentManifestEntry>,
}

impl PublicationChain {
    /// Project the catalog and file commitments of a validated root manifest.
    /// Authentication remains the caller's responsibility (`VerifiedRelease`).
    pub fn from_manifest(manifest: &Manifest) -> Result<Self, ReleaseError> {
        let release = manifest.release.as_ref().ok_or_else(|| {
            ReleaseError::Invalid("publication chain needs a root release".into())
        })?;
        let index = manifest
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let mut publications = release
            .publications
            .iter()
            .map(|path| {
                index.get(path.as_str()).copied().ok_or_else(|| {
                    ReleaseError::Invalid(format!("publication is not indexed: {path}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        publications.sort_by(|left, right| {
            left.metadata
                .date()
                .cmp(&right.metadata.date())
                .then_with(|| left.path.cmp(&right.path))
        });

        let mut head = GENESIS_HASH.to_string();
        let mut blocks = Vec::with_capacity(publications.len());
        for (position, publication) in publications.into_iter().enumerate() {
            let prefix = format!("{}/", publication.path);
            let mut entries = vec![publication];
            entries.extend(
                index
                    .range(prefix.as_str()..)
                    .take_while(|(path, _)| path.starts_with(&prefix))
                    .map(|(_, entry)| *entry),
            );
            let content_bytes = entries
                .iter()
                .filter(|entry| !entry.metadata.kind.is_directory_like())
                .filter_map(|entry| entry.metadata.size_bytes())
                .sum();
            let height = position as u64 + 1;
            let bytes = serde_json::to_vec(&BlockCommitment {
                scheme: "websh.publication-chain.v1",
                site: &release.site,
                height,
                previous_hash: &head,
                path: &publication.path,
                entries,
            })?;
            let hash = format!("0x{}", ReleaseId::of(&bytes));
            blocks.push(PublicationBlock {
                path: publication.path.clone(),
                height,
                previous_hash: head,
                hash: hash.clone(),
                content_bytes,
            });
            head = hash;
        }
        Ok(Self { blocks, head })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::NodeMetadata;
    use crate::publication::{CONTENT_PURPOSE, ReleaseMetadata};

    fn manifest() -> Manifest {
        let entries = [
            ("writing/old.md", "2024-01-01"),
            ("papers/middle.md", "2025-01-01"),
            ("writing/new.md", "2026-01-01"),
        ]
        .map(|(path, date)| {
            let mut metadata = NodeMetadata::default();
            metadata.authored.date = Some(date.into());
            metadata.derived.size_bytes = Some(3);
            metadata.derived.content_sha256 = Some(ReleaseId::of(b"one").to_string());
            ContentManifestEntry {
                path: path.into(),
                metadata,
                mempool: None,
            }
        })
        .to_vec();
        Manifest {
            release: Some(ReleaseMetadata {
                attestations: Default::default(),
                purpose: CONTENT_PURPOSE.into(),
                site: "example.eth".into(),
                sequence: 1,
                issued_at: 1,
                home: super::super::tests::home(),
                mounts: vec![],
                publications: entries.iter().map(|entry| entry.path.clone()).collect(),
            }),
            entries,
        }
    }

    #[test]
    fn chain_is_deterministic_and_independent_of_home_edits() {
        let mut manifest = manifest();
        let chain = PublicationChain::from_manifest(&manifest).unwrap();
        assert_eq!(chain.blocks[0].previous_hash, GENESIS_HASH);
        assert_eq!(chain.head, chain.blocks[2].hash);
        manifest.entries.reverse();
        let release = manifest.release.as_mut().unwrap();
        release.publications.reverse();
        release.sequence += 1;
        release.home.now.items[0].text = "Changed Now".into();
        assert_eq!(PublicationChain::from_manifest(&manifest).unwrap(), chain);
    }

    #[test]
    fn a_publication_change_rehashes_only_it_and_later_blocks() {
        let mut manifest = manifest();
        let before = PublicationChain::from_manifest(&manifest).unwrap();
        manifest.entries[1].metadata.derived.content_sha256 =
            Some(ReleaseId::of(b"two").to_string());
        let after = PublicationChain::from_manifest(&manifest).unwrap();
        assert_eq!(before.blocks[0], after.blocks[0]);
        assert_ne!(before.blocks[1].hash, after.blocks[1].hash);
        assert_eq!(after.blocks[2].previous_hash, after.blocks[1].hash);
        assert_ne!(before.head, after.head);
    }
}
