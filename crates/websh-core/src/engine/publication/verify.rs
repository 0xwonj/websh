use serde::{Deserialize, Serialize};

use crate::{
    crypto::pgp::{PgpPolicy, SignatureEvidence, verify_detached},
    ports::ScannedSubtree,
};

use super::{Manifest, ReleaseError, ReleaseId, ReleaseMetadata};

#[derive(Clone, Copy, Debug)]
pub struct TrustPolicy<'a> {
    pub site: &'a str,
    pub public_key: &'a str,
    pub primary_fingerprint: &'a str,
    pub signer_fingerprints: &'a [&'a str],
}

/// Runtime authentication evidence. No deserializer or unchecked constructor exists.
#[derive(Clone, Debug)]
pub struct VerifiedRelease {
    manifest: Manifest,
    id: ReleaseId,
    signature: SignatureEvidence,
    snapshot: ScannedSubtree,
}

impl VerifiedRelease {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn release(&self) -> &ReleaseMetadata {
        self.manifest
            .release
            .as_ref()
            .expect("verifier requires root release")
    }
    pub fn id(&self) -> &ReleaseId {
        &self.id
    }
    pub fn signer(&self) -> &str {
        self.signature.signer()
    }
    pub fn snapshot(&self) -> &ScannedSubtree {
        &self.snapshot
    }
    pub fn accepted(&self) -> AcceptedRelease {
        AcceptedRelease {
            sequence: self.release().sequence,
            id: self.id.clone(),
        }
    }
}

pub fn verify_release(
    bytes: &[u8],
    signature: &[u8],
    policy: &TrustPolicy<'_>,
    now: u64,
) -> Result<VerifiedRelease, ReleaseError> {
    let manifest = Manifest::decode(bytes)?;
    let snapshot = manifest.validate()?;
    let release = manifest
        .release
        .as_ref()
        .ok_or_else(|| ReleaseError::Invalid("root release metadata is required".into()))?;
    if release.site != policy.site
        || release.sequence == 0
        || release.issued_at == 0
        || release.issued_at > now.saturating_add(300)
    {
        return Err(ReleaseError::Invalid(
            "wrong site or unissued/future release".into(),
        ));
    }
    let signature = verify_detached(
        bytes,
        signature,
        &PgpPolicy {
            public_key: policy.public_key,
            primary_fingerprint: policy.primary_fingerprint,
            signer_fingerprints: policy.signer_fingerprints,
        },
        now,
    )?;
    if release.issued_at > signature.created_at().saturating_add(300) {
        return Err(ReleaseError::Invalid(
            "release issuance postdates its signature".into(),
        ));
    }
    Ok(VerifiedRelease {
        manifest,
        id: ReleaseId::of(bytes),
        signature,
        snapshot,
    })
}

/// Persisted high-water mark, scoped by the caller to the pinned site/trust identity.
/// Only construct/update this from a verified release, never remote metadata.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AcceptedRelease {
    pub sequence: u64,
    pub id: ReleaseId,
}

impl AcceptedRelease {
    pub fn check(&self, candidate: &VerifiedRelease) -> Result<(), ReleaseError> {
        self.check_next(&candidate.accepted())
    }
    /// Also used inside the IndexedDB compare-and-update transaction across tabs.
    pub fn check_next(&self, candidate: &Self) -> Result<(), ReleaseError> {
        if candidate.sequence < self.sequence {
            return Err(ReleaseError::Rollback);
        }
        if candidate.sequence == self.sequence && candidate.id != self.id {
            return Err(ReleaseError::SequenceConflict);
        }
        Ok(())
    }
}
