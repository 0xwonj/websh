//! Content manifest schema. The manifest is the bundled projection of
//! every sidecar in a mount's content tree. The runtime fetches it once
//! and reads metadata directly — no per-file sidecar fetches at runtime.

use serde::{Deserialize, Serialize};

use super::mempool::MempoolFields;
use super::metadata::NodeMetadata;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContentManifestDocument {
    pub entries: Vec<ContentManifestEntry>,
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
    fn content_manifest_document_round_trips_existing_shape() {
        let body = include_str!("../../../../tests/fixtures/manifest_golden.json");
        let manifest: ContentManifestDocument = serde_json::from_str(body).expect("parse");
        let encoded = serde_json::to_string_pretty(&manifest).expect("serialize");
        assert_eq!(encoded.trim_end(), body.trim_end());
    }

    #[test]
    fn content_manifest_requires_entries() {
        let parsed = serde_json::from_str::<ContentManifestDocument>("{}");
        assert!(parsed.is_err());
    }

    #[test]
    fn content_manifest_rejects_unknown_entry_fields() {
        let body = include_str!("../../../../tests/fixtures/manifest_golden.json");
        let mut value: serde_json::Value = serde_json::from_str(body).unwrap();
        value["entries"][0]["unknown"] = true.into();
        assert!(serde_json::from_value::<ContentManifestDocument>(value).is_err());
    }

    #[test]
    fn metadata_has_one_unversioned_shape() {
        let body = include_str!("../../../../tests/fixtures/manifest_golden.json");
        let mut value: serde_json::Value = serde_json::from_str(body).unwrap();
        value["entries"][0]["metadata"]["schema"] = 1.into();
        assert!(serde_json::from_value::<ContentManifestDocument>(value).is_err());
    }
}
