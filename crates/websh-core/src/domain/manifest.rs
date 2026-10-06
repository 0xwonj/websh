//! Content manifest schema. The manifest is the bundled projection of
//! every sidecar in a mount's content tree. The runtime fetches it once
//! and reads metadata directly — no per-file sidecar fetches at runtime.

use serde::{Deserialize, Serialize};

use super::mempool::MempoolFields;
use super::metadata::NodeMetadata;
use super::publication::ReleaseMetadata;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub entries: Vec<ContentManifestEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseMetadata>,
}

/// A content node with optional mempool metadata.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifestEntry {
    pub path: String,
    pub metadata: NodeMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mempool: Option<MempoolFields>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_manifest_round_trips_valid_index() {
        let body = include_str!("../../../../tests/fixtures/manifest_golden.json");
        let manifest = Manifest::from_bytes(body.as_bytes()).expect("valid native index");
        let encoded = serde_json::to_string_pretty(&manifest).expect("serialize");
        assert_eq!(encoded.trim_end(), body.trim_end());
    }

    #[test]
    fn content_manifest_requires_entries() {
        let parsed = serde_json::from_str::<Manifest>("{}");
        assert!(parsed.is_err());
    }

    #[test]
    fn content_manifest_rejects_unknown_entry_fields() {
        let body = include_str!("../../../../tests/fixtures/manifest_golden.json");
        let mut value: serde_json::Value = serde_json::from_str(body).unwrap();
        value["entries"][0]["unknown"] = true.into();
        assert!(serde_json::from_value::<Manifest>(value).is_err());
    }
}
