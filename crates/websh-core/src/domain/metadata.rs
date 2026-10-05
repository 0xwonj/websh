//! Authored display choices and computed content facts have separate contracts.
//! Every node has one structural kind; title may have a generated fallback.

use super::bundle::BundleMetadata;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeMetadata {
    pub kind: NodeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle: Option<BundleMetadata>,
    pub authored: AuthoredMetadata,
    pub derived: DerivedMetadata,
}

/// User decisions accepted in frontmatter and authored declarations.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<Vec<LinkRef>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessFilter>,
}

/// Facts and display defaults computed from the current content bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivedMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    // ── Document / PDF derived ─────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<PageSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,

    // ── Image derived ──────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_dimensions: Option<ImageDim>,

    // ── File integrity ─────────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,

    // ── Markdown derived ───────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub word_count: Option<u32>,

    // ── Directory derived ──────────────────────────────────────────────
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<u32>,
}

/// External or internal resource attached to a content node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinkRef {
    pub label: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl NodeMetadata {
    pub fn title(&self) -> Option<&str> {
        self.authored
            .title
            .as_deref()
            .or(self.derived.title.as_deref())
    }

    pub fn description(&self) -> Option<&str> {
        self.authored.description.as_deref()
    }

    pub fn date(&self) -> Option<&str> {
        self.authored.date.as_deref()
    }

    pub fn content_sha256(&self) -> Option<&str> {
        self.derived.content_sha256.as_deref()
    }

    pub fn tags(&self) -> Option<&[String]> {
        self.authored.tags.as_deref()
    }

    pub fn tags_owned(&self) -> Vec<String> {
        self.authored.tags.clone().unwrap_or_default()
    }

    pub fn links(&self) -> Option<&[LinkRef]> {
        self.authored.links.as_deref()
    }

    pub fn links_owned(&self) -> Vec<LinkRef> {
        self.authored.links.clone().unwrap_or_default()
    }

    pub fn access(&self) -> Option<&AccessFilter> {
        self.authored.access.as_ref()
    }

    pub fn page_size(&self) -> Option<&PageSize> {
        self.derived.page_size.as_ref()
    }

    pub fn image_dimensions(&self) -> Option<&ImageDim> {
        self.derived.image_dimensions.as_ref()
    }

    pub fn page_count(&self) -> Option<u32> {
        self.derived.page_count
    }

    pub fn size_bytes(&self) -> Option<u64> {
        self.derived.size_bytes
    }

    pub fn word_count(&self) -> Option<u32> {
        self.derived.word_count
    }

    pub fn child_count(&self) -> Option<u32> {
        self.derived.child_count
    }

    pub fn is_restricted(&self) -> bool {
        self.authored.access.is_some()
    }

    pub fn is_bundle(&self) -> bool {
        self.kind == NodeKind::Bundle
    }
}

/// Semantic role of a node. Top-level field on [`NodeMetadata`] (not
/// optional) so every record commits to a kind. Derived defaults from file
/// extension; authoring can override.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Page,
    Document,
    App,
    #[default]
    Asset,
    Redirect,
    Data,
    Directory,
    Bundle,
}

impl NodeKind {
    /// Filesystem entries represented by [`FsEntry::Directory`].
    pub fn is_directory_like(self) -> bool {
        matches!(self, Self::Directory | Self::Bundle)
    }
}

/// Advisory access filter. The engine uses it to hide entries from
/// non-recipient viewers; it is not cryptographic confidentiality.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccessFilter {
    pub recipients: Vec<Recipient>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub address: String,
}

/// PDF page geometry in PostScript points (1/72 inch), rounded to the
/// nearest integer. Stored as `u32` (not `f64`) so on-disk JSON is
/// byte-stable across `lopdf` versions and platforms — float
/// representations like `959.760009765625` would otherwise leak
/// precision artifacts into signed canonical messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageDim {
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_resolves_title_without_changing_structural_kind_or_computed_facts() {
        let metadata: NodeMetadata = serde_json::from_str(
            r#"{
            "kind":"directory",
            "authored":{"title":"Authored"},
            "derived":{"title":"Generated","size_bytes":42}
        }"#,
        )
        .unwrap();
        assert_eq!(metadata.title(), Some("Authored"));
        assert_eq!(metadata.size_bytes(), Some(42));
        assert_eq!(metadata.kind, NodeKind::Directory);
        assert!(!metadata.is_bundle());
        let encoded = serde_json::to_string(&metadata).unwrap();
        assert_eq!(
            serde_json::from_str::<NodeMetadata>(&encoded).unwrap(),
            metadata
        );
    }

    #[test]
    fn metadata_rejects_values_outside_their_owner_and_incomplete_records() {
        for invalid in [
            r#"{"kind":"page","authored":{"size_bytes":1},"derived":{}}"#,
            r#"{"kind":"page","authored":{},"derived":{"access":{"recipients":[]}}}"#,
            r#"{"kind":"page","authored":{"kind":"asset"},"derived":{}}"#,
            r#"{"kind":"page","derived":{}}"#,
            r#"{"kind":"page","authored":{}}"#,
            r#"{"authored":{},"derived":{}}"#,
        ] {
            assert!(
                serde_json::from_str::<NodeMetadata>(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
