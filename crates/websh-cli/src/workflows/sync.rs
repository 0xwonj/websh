use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use websh_core::attestation::artifact::AttestationArtifact;
use websh_core::attestation::ledger::CONTENT_LEDGER_PATH;
use websh_core::crypto::ack::AckArtifact;
use websh_site::{ACK_ARTIFACT_PATH, ATTESTATIONS_PATH};

use super::content::ContentSnapshot;
use super::{ack, attest};
use crate::CliResult;
use crate::infra::json::json_bytes;
use crate::infra::json::{read_json, write_bytes};

pub(crate) struct Prepared {
    pub(crate) artifact: AttestationArtifact,
    pub(crate) content: ContentSnapshot,
    outputs: Vec<(PathBuf, Vec<u8>)>,
    retained_bytes: Vec<u8>,
}

pub(crate) struct SyncOutcome {
    pub(crate) entries: usize,
    pub(crate) blocks: usize,
    pub(crate) subjects: usize,
}

impl Prepared {
    pub(crate) fn load(root: &Path) -> CliResult<Self> {
        attest::verify::verify_site_key(root)?;
        let retained_bytes = fs::read(root.join(ATTESTATIONS_PATH)).context(
            "read retained attestation records; restore a missing or damaged file from Git",
        )?;
        let existing: AttestationArtifact = serde_json::from_slice(&retained_bytes)
            .context("parse retained attestation records; restore a damaged file from Git")?;
        attest::verify::verify_artifact(root, &existing, false)?;
        let ack: AckArtifact = match ack::prepare(root)? {
            Some(artifact) => artifact,
            None => read_json(&root.join(ACK_ARTIFACT_PATH))?,
        };
        ack.validate()?;
        let content = ContentSnapshot::load(root)?;
        let artifact = attest::prepare(root, &content, &ack, &existing)?;
        let outputs = vec![
            (PathBuf::from(ACK_ARTIFACT_PATH), json_bytes(&ack)?),
            (
                PathBuf::from(CONTENT_LEDGER_PATH),
                json_bytes(&content.ledger)?,
            ),
            (
                PathBuf::from("content/manifest.json"),
                json_bytes(&content.manifest)?,
            ),
        ];
        Ok(Self {
            artifact,
            content,
            outputs,
            retained_bytes,
        })
    }

    /// External signing may take time. Refuse to overwrite new authored inputs
    /// or another operation's retained signatures when it returns.
    pub(crate) fn ensure_current(&self, root: &Path) -> CliResult {
        let current = Self::load(root)?;
        let same_subjects = self.artifact.subjects.len() == current.artifact.subjects.len()
            && self
                .artifact
                .subjects
                .iter()
                .zip(&current.artifact.subjects)
                .all(|(left, right)| attest::same_payload(left, right));
        if self.retained_bytes != current.retained_bytes
            || self.outputs != current.outputs
            || !same_subjects
        {
            bail!("project inputs changed during signing; retry against the current sources");
        }
        Ok(())
    }

    pub(crate) fn publish(&self, root: &Path) -> CliResult {
        let artifact = json_bytes(&self.artifact)?;
        // Serialization and validation finish before the first replacement.
        for (path, bytes) in &self.outputs {
            write_bytes(&root.join(path), bytes)?;
        }
        write_bytes(&root.join(ATTESTATIONS_PATH), &artifact)
    }

    pub(crate) fn check_outputs(&self, root: &Path) -> CliResult {
        for (path, expected) in self.outputs.iter().cloned().chain(std::iter::once((
            PathBuf::from(ATTESTATIONS_PATH),
            json_bytes(&self.artifact)?,
        ))) {
            let actual = fs::read(root.join(&path))
                .with_context(|| format!("read generated {}", path.display()))?;
            if actual != expected {
                bail!("{} is stale; run websh-cli sync", path.display());
            }
        }
        Ok(())
    }

    pub(crate) fn outcome(&self) -> SyncOutcome {
        SyncOutcome {
            entries: self.content.manifest.entries.len(),
            blocks: self.content.ledger.blocks.len(),
            subjects: self.artifact.subjects.len(),
        }
    }
}

pub(crate) fn sync(root: &Path) -> CliResult<SyncOutcome> {
    let prepared = Prepared::load(root)?;
    prepared.publish(root)?;
    Ok(prepared.outcome())
}
