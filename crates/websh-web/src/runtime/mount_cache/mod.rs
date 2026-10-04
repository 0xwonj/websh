//! Disposable external listing snapshots. Content bodies and root discovery never enter this cache.
mod idb;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::rc::Rc;
use websh_core::ports::{
    LocalBoxFuture, ScannedSubtree, parse_manifest_snapshot, serialize_manifest_snapshot,
};

pub use idb::BrowserMountCache;

pub const RESTORE_TIMEOUT_MS: u32 = 500;
pub const OPERATION_TIMEOUT_MS: u32 = 2_000;
pub const MANIFEST_TIMEOUT_MS: u32 = 10_000;
pub const MAX_RECORD_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RECORDS: usize = 16;
pub const MAX_AGE_MS: u64 = 30 * 24 * 60 * 60 * 1000;
pub const FUTURE_SKEW_MS: u64 = 5 * 60 * 1000;
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheDescriptor {
    pub descriptor_version: u32,
    pub backend_kind: String,
    pub canonical_mount_root: String,
    pub repository_owner_and_name: String,
    pub branch_or_ref: String,
    pub normalized_content_prefix: String,
    pub resolved_manifest_url: String,
    pub resolved_content_base_url: String,
}

impl CacheDescriptor {
    pub fn key(&self) -> String {
        // Struct field order provides a deterministic encoding; no unordered map is involved.
        let bytes = serde_json::to_vec(self).expect("string descriptor is serializable");
        format!("mount-v1:{}", hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheRecord {
    pub key: String,
    pub record_version: u32,
    pub descriptor: CacheDescriptor,
    pub manifest_json: String,
    pub manifest_bytes: usize,
    pub request_started_at_ms: u64,
    pub observed_at_ms: u64,
}

impl CacheRecord {
    pub fn from_scan(
        descriptor: CacheDescriptor,
        scan: &ScannedSubtree,
        started: u64,
        observed: u64,
    ) -> Option<Self> {
        let manifest_json = serialize_manifest_snapshot(scan).ok()?;
        let record = Self {
            key: descriptor.key(),
            record_version: 1,
            descriptor,
            manifest_bytes: manifest_json.len(),
            manifest_json,
            request_started_at_ms: started,
            observed_at_ms: observed,
        };
        record.validate(&record.descriptor, observed)?;
        Some(record)
    }

    pub fn validate(&self, descriptor: &CacheDescriptor, now: u64) -> Option<ScannedSubtree> {
        if self.record_version != 1
            || descriptor.descriptor_version != 1
            || descriptor.backend_kind != "github"
            || descriptor.canonical_mount_root == "/"
            || &self.descriptor != descriptor
            || self.key != descriptor.key()
            || self.manifest_bytes != self.manifest_json.len()
            || self.manifest_bytes > MAX_RECORD_BYTES
            || self.request_started_at_ms > self.observed_at_ms
            || self.observed_at_ms > MAX_SAFE_INTEGER
            || self.observed_at_ms > now.saturating_add(FUTURE_SKEW_MS)
            || now.saturating_sub(self.observed_at_ms) > MAX_AGE_MS
        {
            return None;
        }
        parse_manifest_snapshot(&self.manifest_json).ok()
    }

    pub fn outranks(&self, existing: &Self) -> bool {
        (self.request_started_at_ms, self.observed_at_ms)
            > (existing.request_started_at_ms, existing.observed_at_ms)
    }
}

pub struct CachedSnapshot {
    pub scan: ScannedSubtree,
    pub observed_at_ms: u64,
}

/// The validity check is repeated just before opening a write transaction.
#[derive(Clone)]
pub struct CacheWrite {
    pub record: CacheRecord,
    pub is_current: Rc<dyn Fn() -> bool>,
}

pub trait MountCache {
    fn restore(&self, descriptor: CacheDescriptor) -> LocalBoxFuture<'_, Option<CachedSnapshot>>;
    fn persist(&self, write: CacheWrite) -> LocalBoxFuture<'_, ()>;
}

pub type MountCacheRef = Rc<dyn MountCache>;

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    wasm_bindgen_test_configure!(run_in_browser);

    pub fn descriptor() -> CacheDescriptor {
        CacheDescriptor {
            descriptor_version: 1,
            backend_kind: "github".into(),
            canonical_mount_root: "/db".into(),
            repository_owner_and_name: "owner/repo".into(),
            branch_or_ref: "Main".into(),
            normalized_content_prefix: "content".into(),
            resolved_manifest_url:
                "https://raw.githubusercontent.com/owner/repo/Main/content/manifest.json".into(),
            resolved_content_base_url: "https://raw.githubusercontent.com/owner/repo/Main/content/"
                .into(),
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
        vary!(canonical_mount_root, "/elsewhere");
        vary!(repository_owner_and_name, "owner/other");
        vary!(branch_or_ref, "main");
        vary!(normalized_content_prefix, "~");
        vary!(
            resolved_manifest_url,
            "https://example.org/ipfs/new/content/manifest.json"
        );
        vary!(
            resolved_content_base_url,
            "https://example.org/ipfs/new/content/"
        );
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
        invalid.manifest_json = "{bad".into();
        invalid.manifest_bytes = invalid.manifest_json.len();
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
        record.manifest_json = " ".repeat(MAX_RECORD_BYTES + 1);
        record.manifest_bytes = record.manifest_json.len();
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
