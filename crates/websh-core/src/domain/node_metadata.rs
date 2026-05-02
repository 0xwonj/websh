//! Unified node metadata model.
//!
//! A single [`NodeMetadata`] type carries all metadata for any node (file or
//! directory). It splits values into two parallel sections:
//!
//! - `authored`: hand-written by content authors (in markdown frontmatter
//!   for `.md` files, or directly in the sidecar JSON for everything else).
//! - `derived`: extracted/computed by `websh-cli content manifest` from the
//!   raw bytes (PDF page dimensions, image size, file hashes, etc.).
//!
//! The effective value of any field is `authored.X.or(derived.X)` —
//! authored wins. Accessor methods on [`NodeMetadata`] encapsulate the rule
//! so consumers don't have to remember it.
//!
//! Both sections share the same [`Fields`] struct; fields irrelevant to a
//! given section simply remain `None` (e.g. `content_sha256` is only ever
//! populated in `derived`; `access` is only ever populated in `authored`).
//! This symmetry keeps the model uniform and lets new derived fields be
//! added without API churn.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

/// Top-level metadata record for a node. Persisted as `<file>.meta.json`
/// (file sidecars), `_index.dir.json` (directory sidecars), and embedded
/// inline in the manifest bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeMetadata {
    #[serde(default = "default_schema")]
    pub schema: u32,
    pub kind: NodeKind,
    #[serde(default)]
    pub authored: Fields,
    #[serde(default)]
    pub derived: Fields,
}

impl Default for NodeMetadata {
    fn default() -> Self {
        Self {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Asset,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }
}

fn default_schema() -> u32 {
    SCHEMA_VERSION
}

/// Every metadata field the system understands. Both `authored` and
/// `derived` sections of [`NodeMetadata`] use this struct.
///
/// All fields are `Option<T>`. `None` means "no value in this section";
/// fields irrelevant to a given section's role stay `None` permanently.
/// `serde(skip_serializing_if = "Option::is_none")` keeps the on-disk
/// JSON compact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fields {
    // ── Identity / classification ──────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<NodeKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderer: Option<RendererKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,

    // ── Authoring / display ────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,

    // ── Trust / access ─────────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<TrustLevel>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<AccessFilter>,

    // ── Document / PDF derived ─────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_size: Option<PageSize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<u32>,

    // ── Image derived ──────────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_dimensions: Option<ImageDim>,

    // ── File integrity ─────────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,

    // ── Markdown derived ───────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub word_count: Option<u32>,

    // ── Directory derived ──────────────────────────────────────────────
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub child_count: Option<u32>,
}

/// Generates `pub fn name(&self) -> Option<&str>` accessors for
/// `Option<String>` fields, resolving `authored ?? derived`.
macro_rules! resolve_str_accessors {
    ($($name:ident),* $(,)?) => {
        impl NodeMetadata {
            $(
                pub fn $name(&self) -> Option<&str> {
                    self.authored.$name.as_deref().or(self.derived.$name.as_deref())
                }
            )*
        }
    };
}

/// Generates `pub fn name(&self) -> Option<&[T]>` accessors for
/// `Option<Vec<T>>` fields.
macro_rules! resolve_slice_accessors {
    ($($name:ident -> $elem:ty),* $(,)?) => {
        impl NodeMetadata {
            $(
                pub fn $name(&self) -> Option<&[$elem]> {
                    self.authored.$name.as_deref().or(self.derived.$name.as_deref())
                }
            )*
        }
    };
}

/// Generates `pub fn name(&self) -> Option<&T>` accessors for non-string
/// reference types.
macro_rules! resolve_ref_accessors {
    ($($name:ident -> $ty:ty),* $(,)?) => {
        impl NodeMetadata {
            $(
                pub fn $name(&self) -> Option<&$ty> {
                    self.authored.$name.as_ref().or(self.derived.$name.as_ref())
                }
            )*
        }
    };
}

/// Generates `pub fn name(&self) -> Option<T>` accessors for `Copy` scalar
/// fields.
macro_rules! resolve_copy_accessors {
    ($($name:ident -> $ty:ty),* $(,)?) => {
        impl NodeMetadata {
            $(
                pub fn $name(&self) -> Option<$ty> {
                    self.authored.$name.or(self.derived.$name)
                }
            )*
        }
    };
}

resolve_str_accessors! {
    title,
    description,
    date,
    route,
    icon,
    thumbnail,
    sort,
    content_sha256,
}

resolve_slice_accessors! {
    tags -> String,
}

resolve_ref_accessors! {
    access -> AccessFilter,
    page_size -> PageSize,
    image_dimensions -> ImageDim,
}

resolve_copy_accessors! {
    page_count -> u32,
    rotation -> u32,
    size_bytes -> u64,
    modified_at -> u64,
    word_count -> u32,
    child_count -> u32,
}

impl NodeMetadata {
    /// Effective tags as an owned `Vec<String>`. Returns an empty vec
    /// when neither section has tags. Convenience for callers that need
    /// owned data (e.g. cloning into UI structs).
    pub fn tags_owned(&self) -> Vec<String> {
        self.tags().map(<[String]>::to_vec).unwrap_or_default()
    }

    /// Effective node kind. Falls back from authored → derived → top-level.
    pub fn effective_kind(&self) -> NodeKind {
        self.authored
            .kind
            .or(self.derived.kind)
            .unwrap_or(self.kind)
    }

    /// Effective renderer (authored ?? derived).
    pub fn renderer(&self) -> Option<RendererKind> {
        self.authored.renderer.or(self.derived.renderer)
    }

    /// Effective trust level (authored ?? derived).
    pub fn trust(&self) -> Option<TrustLevel> {
        self.authored.trust.or(self.derived.trust)
    }

    /// True iff this node has any access filter (authored or derived).
    pub fn is_restricted(&self) -> bool {
        self.access().is_some()
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
}

/// Renderer family the engine should instantiate. Optional override; the
/// engine derives a sensible default from `kind` + extension when absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererKind {
    HtmlPage,
    MarkdownPage,
    DirectoryListing,
    TerminalApp,
    DocumentReader,
    Image,
    Pdf,
    Redirect,
    RawText,
}

/// Trust assertion attached to a node or subtree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    Trusted,
    Untrusted,
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

/// Builders for minimal `NodeMetadata` fixtures used by sibling tests
/// across the crate.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    pub fn blank_meta(kind: NodeKind) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }

    pub fn directory_meta(title: &str) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Directory,
            authored: Fields {
                title: Some(title.to_string()),
                ..Fields::default()
            },
            derived: Fields::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_round_trip_in_snake_case() {
        let kind = serde_json::to_string(&NodeKind::Page).unwrap();
        let renderer = serde_json::to_string(&RendererKind::HtmlPage).unwrap();
        assert_eq!(kind, "\"page\"");
        assert_eq!(renderer, "\"html_page\"");
    }

    #[test]
    fn skips_none_fields_on_serialization() {
        let meta = NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields {
                title: Some("Hello".to_string()),
                ..Fields::default()
            },
            derived: Fields::default(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        // Should not contain other field keys.
        assert!(json.contains("\"title\":\"Hello\""));
        assert!(!json.contains("\"description\""));
        assert!(!json.contains("\"page_size\""));
    }

    #[test]
    fn authored_wins_over_derived() {
        let meta = NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Document,
            authored: Fields {
                title: Some("My Override".to_string()),
                ..Fields::default()
            },
            derived: Fields {
                title: Some("autogenerated".to_string()),
                page_count: Some(7),
                ..Fields::default()
            },
        };
        assert_eq!(meta.title(), Some("My Override"));
        assert_eq!(meta.page_count(), Some(7)); // only in derived
    }

    #[test]
    fn derived_used_when_authored_is_none() {
        let meta = NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Document,
            authored: Fields::default(),
            derived: Fields {
                title: Some("derived-only".to_string()),
                page_size: Some(PageSize {
                    width: 612,
                    height: 792,
                }),
                ..Fields::default()
            },
        };
        assert_eq!(meta.title(), Some("derived-only"));
        assert_eq!(
            meta.page_size().copied(),
            Some(PageSize {
                width: 612,
                height: 792
            })
        );
    }

    #[test]
    fn deny_unknown_fields_on_top_level() {
        let bad = r#"{"schema":1,"kind":"page","authored":{},"derived":{},"unexpected":"value"}"#;
        let parsed = serde_json::from_str::<NodeMetadata>(bad);
        assert!(parsed.is_err());
    }

    #[test]
    fn deny_unknown_fields_on_fields_section() {
        let bad =
            r#"{"schema":1,"kind":"page","authored":{"unexpected_key":"value"},"derived":{}}"#;
        let parsed = serde_json::from_str::<NodeMetadata>(bad);
        assert!(parsed.is_err());
    }

    #[test]
    fn round_trip_full_metadata() {
        let meta = NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Document,
            authored: Fields {
                title: Some("Tabula Recta".to_string()),
                date: Some("2024-09-12".to_string()),
                tags: Some(vec!["paper".to_string(), "rust".to_string()]),
                ..Fields::default()
            },
            derived: Fields {
                kind: Some(NodeKind::Document),
                renderer: Some(RendererKind::Pdf),
                size_bytes: Some(287654),
                modified_at: Some(1726099200),
                content_sha256: Some("0xabc".to_string()),
                page_size: Some(PageSize {
                    width: 612,
                    height: 792,
                }),
                page_count: Some(14),
                rotation: Some(0),
                ..Fields::default()
            },
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: NodeMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta, back);
    }
}
