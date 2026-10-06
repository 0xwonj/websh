//! Disposable exact source snapshots, revalidated before use.
mod idb;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::rc::Rc;
use websh_core::ports::{LocalBoxFuture, ScannedSubtree};

pub use idb::BrowserMountCache;

pub const RESTORE_TIMEOUT_MS: u32 = 500;
pub const OPERATION_TIMEOUT_MS: u32 = 2_000;
pub const MANIFEST_TIMEOUT_MS: u32 = 10_000;
pub const MAX_RECORD_BYTES: usize = websh_core::publication::MAX_MANIFEST_BYTES + 16 * 1024;
pub const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RECORDS: usize = 16;
pub const MAX_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;
pub const FUTURE_SKEW_MS: u64 = 5 * 60 * 1000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheDescriptor {
    pub root: String,
    pub repo: String,
    pub reference: String,
    pub prefix: String,
    pub trust: Option<String>,
}

impl CacheDescriptor {
    pub fn key(&self) -> String {
        // Struct field order provides a deterministic encoding; no unordered map is involved.
        let bytes = serde_json::to_vec(self).expect("string descriptor is serializable");
        format!("mount:{}", hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheRecord {
    pub key: String,
    pub descriptor: CacheDescriptor,
    pub source: websh_core::publication::SourceSnapshot,
    pub manifest_bytes: usize,
    pub request_started_at_ms: u64,
    pub observed_at_ms: u64,
}

impl CacheRecord {
    #[cfg(test)]
    pub fn from_scan(
        descriptor: CacheDescriptor,
        scan: &ScannedSubtree,
        started: u64,
        observed: u64,
    ) -> Option<Self> {
        let manifest_json = websh_core::ports::serialize_manifest_snapshot(scan).ok()?;
        let record = Self {
            key: descriptor.key(),
            descriptor,
            source: websh_core::publication::SourceSnapshot {
                commit: websh_core::publication::GitCommit::parse("0".repeat(40)).ok()?,
                manifest: manifest_json.clone(),
                signature: None,
            },
            manifest_bytes: manifest_json.len(),
            request_started_at_ms: started,
            observed_at_ms: observed,
        };
        record.validate(&record.descriptor, observed)?;
        Some(record)
    }

    pub fn from_source(
        descriptor: CacheDescriptor,
        source: websh_core::publication::SourceSnapshot,
        started: u64,
        observed: u64,
    ) -> Option<Self> {
        let record = Self {
            key: descriptor.key(),
            descriptor,
            manifest_bytes: source.manifest.len()
                + source.signature.as_ref().map_or(0, String::len),
            source,
            request_started_at_ms: started,
            observed_at_ms: observed,
        };
        record.validate(&record.descriptor, observed)?;
        Some(record)
    }

    pub fn validate(&self, descriptor: &CacheDescriptor, now: u64) -> Option<ScannedSubtree> {
        if &self.descriptor != descriptor
            || self.key != descriptor.key()
            || self.manifest_bytes
                != self.source.manifest.len()
                    + self.source.signature.as_ref().map_or(0, String::len)
            || self.manifest_bytes > MAX_RECORD_BYTES
            || self.request_started_at_ms > self.observed_at_ms
            || self.observed_at_ms > MAX_SAFE_INTEGER
            || self.observed_at_ms > now.saturating_add(FUTURE_SKEW_MS)
            || (descriptor.root != "/" && now.saturating_sub(self.observed_at_ms) > MAX_AGE_MS)
        {
            return None;
        }
        super::github_backend::restore_backend(descriptor, self.source.clone())
            .ok()?
            .loaded()
            .map(|s| s.scan.clone())
    }

    pub fn outranks(&self, existing: &Self) -> bool {
        if self.descriptor.root == "/" {
            let accepted = |record: &Self| {
                super::github_backend::restore_backend(&record.descriptor, record.source.clone())
                    .ok()?
                    .loaded()?
                    .verified
                    .as_ref()
                    .map(|r| r.accepted())
            };
            let (Some(candidate), Some(old)) = (accepted(self), accepted(existing)) else {
                return false;
            };
            if old.check_next(&candidate).is_err() {
                return false;
            }
            if candidate.sequence > old.sequence {
                return true;
            }
        }
        (self.request_started_at_ms, self.observed_at_ms)
            > (existing.request_started_at_ms, existing.observed_at_ms)
    }
}

pub struct CachedSnapshot {
    pub source: websh_core::publication::SourceSnapshot,
    pub scan: ScannedSubtree,
    pub observed_at_ms: u64,
}

/// The validity check is repeated just before opening a write transaction.
#[derive(Clone)]
pub struct CacheWrite {
    pub record: CacheRecord,
    pub is_current: Rc<dyn Fn() -> bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootAcceptance {
    Accepted,
    Rejected,
    Unavailable,
}

pub trait MountCache {
    fn accept_root(&self, _write: CacheWrite) -> LocalBoxFuture<'_, RootAcceptance> {
        Box::pin(async { RootAcceptance::Unavailable })
    }

    fn restore(&self, descriptor: CacheDescriptor) -> LocalBoxFuture<'_, Option<CachedSnapshot>>;
    fn persist(&self, write: CacheWrite) -> LocalBoxFuture<'_, ()>;
}

pub type MountCacheRef = Rc<dyn MountCache>;

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    pub fn descriptor() -> CacheDescriptor {
        CacheDescriptor {
            root: "/db".into(),
            repo: "owner/repo".into(),
            reference: "Main".into(),
            prefix: "content".into(),
            trust: None,
        }
    }

    #[wasm_bindgen_test]
    fn identity_changes_for_every_source_field_and_preserves_case() {
        let d = descriptor();
        let mut variants = vec![];
        macro_rules! vary {
            ($field:ident, $value:expr) => {{
                let mut n = d.clone();
                n.$field = $value.into();
                variants.push(n);
            }};
        }
        vary!(root, "/elsewhere");
        vary!(repo, "owner/other");
        vary!(reference, "main");
        vary!(prefix, "~");
        let mut other_trust = d.clone();
        other_trust.trust = Some("different trust policy".into());
        variants.push(other_trust);
        for changed in variants {
            assert_ne!(d.key(), changed.key());
        }
        assert_eq!(d.key(), d.clone().key());
    }

    #[wasm_bindgen_test]
    fn untrusted_records_and_clock_rollback_are_rejected() {
        let now = MAX_AGE_MS * 2;
        let record =
            CacheRecord::from_scan(descriptor(), &ScannedSubtree::default(), now - 10, now)
                .unwrap();
        assert!(record.validate(&descriptor(), now).is_some());
        assert!(
            record
                .validate(&descriptor(), now + MAX_AGE_MS + 1)
                .is_none()
        );
        assert!(
            record
                .validate(&descriptor(), now - FUTURE_SKEW_MS - 1)
                .is_none()
        );
        let mut invalid = record.clone();
        invalid.manifest_bytes += 1;
        assert!(invalid.validate(&descriptor(), now).is_none());
        invalid = record.clone();
        invalid.source.manifest = "{bad".into();
        invalid.manifest_bytes = invalid.source.manifest.len();
        assert!(invalid.validate(&descriptor(), now).is_none());
        invalid = record.clone();
        invalid.key = "other".into();
        assert!(invalid.validate(&descriptor(), now).is_none());
        assert!(
            CacheRecord::from_scan(descriptor(), &ScannedSubtree::default(), now + 1, now)
                .is_none()
        );
        assert!(!record.outranks(&record));
        let mut newer = record.clone();
        newer.request_started_at_ms += 1;
        assert!(newer.outranks(&record));
    }
    #[wasm_bindgen_test]
    fn oversized_payload_and_unsafe_timestamps_are_never_cached() {
        let now = MAX_AGE_MS * 2;
        let mut record =
            CacheRecord::from_scan(descriptor(), &ScannedSubtree::default(), now, now).unwrap();
        record.source.manifest = " ".repeat(MAX_RECORD_BYTES + 1);
        record.manifest_bytes = record.source.manifest.len();
        assert!(record.validate(&descriptor(), now).is_none());
        record =
            CacheRecord::from_scan(descriptor(), &ScannedSubtree::default(), now, now).unwrap();
        record.observed_at_ms = MAX_SAFE_INTEGER + 1;
        assert!(
            record
                .validate(&descriptor(), MAX_SAFE_INTEGER + 1)
                .is_none()
        );
        record.observed_at_ms = now + FUTURE_SKEW_MS + 1;
        assert!(record.validate(&descriptor(), now).is_none());
    }
}
