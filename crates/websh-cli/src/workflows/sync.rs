use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, bail};
use websh_core::attestation::artifact::AttestationArtifact;
use websh_core::crypto::ack::{AckArtifact, AckPrivateSource, build_artifact_from_source};
use websh_core::domain::{
    ContentManifestEntry, DerivedMetadata, GitHubMount, NodeKind, NodeMetadata,
};
use websh_core::publication::{
    CONTENT_PURPOSE, HomeProjection, Manifest, Now, Profile, ReleaseMetadata,
};
use websh_site::{ACK_ARTIFACT_PATH, APP_NAME};

use super::content::ContentSnapshot;
use super::{ack, attest};
use crate::CliResult;
use crate::infra::json::{json_bytes, read_json, write_bytes};

pub(crate) const MANIFEST_PATH: &str = "content/manifest.json";
pub(crate) const SIGNATURE_PATH: &str = "content/manifest.sig";

/// A frozen public input set. Generating or exporting it has no signing effects.
pub(crate) struct Prepared {
    pub(crate) artifact: AttestationArtifact,
    pub(crate) content: ContentSnapshot,
    ack: AckArtifact,
    prior_release: Option<ReleaseMetadata>,
    retained_bytes: Option<Vec<u8>>,
}

pub(crate) struct SyncOutcome {
    pub(crate) entries: usize,
    pub(crate) subjects: usize,
}

impl Prepared {
    pub(crate) fn load(root: &Path) -> CliResult<Self> {
        let retained_bytes = match fs::read(root.join(MANIFEST_PATH)) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("read prior manifest"),
        };
        let prior_release = retained_bytes
            .as_deref()
            .map(Manifest::from_bytes)
            .transpose()?
            .and_then(|manifest| manifest.release);
        let existing = prior_release
            .as_ref()
            .map(|release| release.attestations.clone())
            .unwrap_or_default();
        attest::verify::verify_artifact(&existing, false)?;
        let ack = match ack::prepare(root)? {
            Some(artifact) => artifact,
            None if root.join(ACK_ARTIFACT_PATH).exists() => {
                read_json(&root.join(ACK_ARTIFACT_PATH))?
            }
            None => build_artifact_from_source(&AckPrivateSource::default())?,
        };
        ack.validate()?;
        let content = ContentSnapshot::load(root)?;
        let mut prepared = Self {
            artifact: AttestationArtifact::default(),
            content,
            ack,
            prior_release,
            retained_bytes,
        };
        prepared.artifact = attest::prepare(&prepared.manifest()?, &existing)?;
        prepared.manifest()?;
        Ok(prepared)
    }

    pub(crate) fn manifest(&self) -> CliResult<Manifest> {
        let files = &self.content.files;
        let ack_bytes = json_bytes(&self.ack)?;
        let mut entries = self.content.manifest.entries.clone();
        for (path, bytes) in files
            .iter()
            .map(|(p, b)| (p.as_str(), b))
            .chain(std::iter::once((".websh/ack.commitment.json", &ack_bytes)))
        {
            if entries.iter().any(|entry| entry.path == path) {
                continue;
            }
            entries.push(ContentManifestEntry {
                path: path.to_owned(),
                metadata: NodeMetadata {
                    kind: NodeKind::Data,
                    derived: DerivedMetadata {
                        size_bytes: Some(bytes.len() as u64),
                        content_sha256: Some(
                            websh_core::publication::ReleaseId::of(bytes)
                                .as_str()
                                .to_owned(),
                        ),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                mempool: None,
            });
        }
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let source = |path: &str| -> CliResult<&str> {
            std::str::from_utf8(files.get(path).with_context(|| format!("missing {path}"))?)
                .with_context(|| format!("{path} must be UTF-8"))
        };
        let profile: Profile =
            toml::from_str(source(".site/profile.toml")?).context("parse profile")?;
        let now: Now = toml::from_str(source(".site/now.toml")?).context("parse Now")?;
        let mounts = files
            .iter()
            .filter(|(path, _)| path.starts_with(".websh/mounts/") && path.ends_with(".mount.json"))
            .map(|(path, bytes)| {
                serde_json::from_slice::<GitHubMount>(bytes)
                    .with_context(|| format!("parse {path}"))
            })
            .collect::<CliResult<Vec<_>>>()?;
        let manifest = Manifest {
            entries,
            release: Some(ReleaseMetadata {
                attestations: self.artifact.clone(),
                purpose: CONTENT_PURPOSE.to_owned(),
                site: APP_NAME.to_owned(),
                sequence: self
                    .prior_release
                    .as_ref()
                    .map_or(0, |release| release.sequence),
                issued_at: self
                    .prior_release
                    .as_ref()
                    .map_or(0, |release| release.issued_at),
                home: HomeProjection {
                    profile,
                    now,
                    ack: self.ack.clone(),
                },
                mounts,
                publications: self
                    .content
                    .units
                    .iter()
                    .map(|unit| unit.path.clone())
                    .collect(),
            }),
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Paths are relative to the public content root, including authored sidecars.
    pub(crate) fn files(&self) -> CliResult<BTreeMap<String, Vec<u8>>> {
        let mut files = self.content.files.clone();
        files.insert(".websh/ack.commitment.json".into(), json_bytes(&self.ack)?);
        Ok(files)
    }

    pub(crate) fn ensure_current(&self, root: &Path) -> CliResult {
        let current = Self::load(root)?;
        if self.retained_bytes != current.retained_bytes
            || self.content.files != current.content.files
            || self.ack != current.ack
        {
            bail!("content inputs changed during signing; retry against the current sources");
        }
        Ok(())
    }

    pub(crate) fn publish(&self, root: &Path) -> CliResult {
        let manifest = json_bytes(&self.manifest()?)?;
        write_bytes(&root.join(ACK_ARTIFACT_PATH), &json_bytes(&self.ack)?)?;
        write_bytes(&root.join(MANIFEST_PATH), &manifest)
    }

    pub(crate) fn check_outputs(&self, root: &Path) -> CliResult {
        for (path, expected) in [
            (ACK_ARTIFACT_PATH, json_bytes(&self.ack)?),
            (MANIFEST_PATH, json_bytes(&self.manifest()?)?),
        ] {
            if fs::read(root.join(path)).with_context(|| format!("read generated {path}"))?
                != expected
            {
                bail!("{path} is stale; run websh-cli sync");
            }
        }
        Ok(())
    }

    pub(crate) fn outcome(&self) -> SyncOutcome {
        SyncOutcome {
            entries: self.content.manifest.entries.len(),
            subjects: self.artifact.subjects.len(),
        }
    }
}

pub(crate) fn sync(root: &Path) -> CliResult<SyncOutcome> {
    let prepared = Prepared::load(root)?;
    prepared.publish(root)?;
    Ok(prepared.outcome())
}
