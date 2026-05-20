use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::support::normalize_locale_tag;

pub type BundleValidationResult<T = ()> = Result<T, BundleValidationError>;

/// Top-level metadata for a renderable directory bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleMetadata {
    pub default_variant: BundleDefaultVariant,
    pub variants: Vec<BundleVariant>,
}

/// Policy used when a request targets a bundle directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "snake_case", deny_unknown_fields)]
pub enum BundleDefaultVariant {
    /// The bundle root is the canonical route for one declared variant.
    Static { id: String },
    /// The bundle root selects a localized explicit variant route.
    Locale { fallback: String },
}

impl BundleMetadata {
    /// Declared default id used by metadata-only consumers.
    ///
    /// Locale selection is user-specific and belongs at request time; places
    /// like ledgers and directory summaries should use this stable id.
    pub fn default_variant_id(&self) -> &str {
        self.default_variant.variant_id()
    }

    pub fn static_default_variant_id(&self) -> Option<&str> {
        match &self.default_variant {
            BundleDefaultVariant::Static { id } => Some(id),
            BundleDefaultVariant::Locale { .. } => None,
        }
    }

    pub fn is_static_default_variant(&self, variant_id: &str) -> bool {
        self.static_default_variant_id() == Some(variant_id)
    }

    pub fn is_locale_default_strategy(&self) -> bool {
        matches!(self.default_variant, BundleDefaultVariant::Locale { .. })
    }

    pub fn variant_by_id(&self, variant_id: &str) -> Option<&BundleVariant> {
        self.variants
            .iter()
            .find(|variant| variant.id == variant_id)
    }

    pub fn selected_variant_for_locale(&self, raw_locale: Option<&str>) -> Option<&BundleVariant> {
        match &self.default_variant {
            BundleDefaultVariant::Static { id } => self.variant_by_id(id),
            BundleDefaultVariant::Locale { fallback } => {
                if let Some(requested) = raw_locale.and_then(normalize_locale_tag)
                    && let Some(variant) = self.variants.iter().find(|variant| {
                        variant
                            .locale
                            .as_deref()
                            .and_then(normalize_locale_tag)
                            .as_deref()
                            == Some(requested.as_str())
                    })
                {
                    return Some(variant);
                }
                self.variant_by_id(fallback)
            }
        }
    }
}

impl BundleDefaultVariant {
    pub fn variant_id(&self) -> &str {
        match self {
            Self::Static { id } => id,
            Self::Locale { fallback } => fallback,
        }
    }
}

/// One declared rendition inside a bundle directory.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleVariant {
    pub id: String,
    pub path: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BundleValidationError {
    #[error("bundle {bundle_path} has an empty default_variant id")]
    EmptyDefaultVariant { bundle_path: String },
    #[error("bundle {bundle_path} declares duplicate variant id `{variant_id}`")]
    DuplicateVariantId {
        bundle_path: String,
        variant_id: String,
    },
    #[error("bundle {bundle_path} declares duplicate variant path `{variant_path}`")]
    DuplicateVariantPath {
        bundle_path: String,
        variant_path: String,
    },
    #[error("bundle {bundle_path} default_variant `{default_variant}` is not declared")]
    DefaultVariantMissing {
        bundle_path: String,
        default_variant: String,
    },
    #[error(
        "bundle {bundle_path} variant id `{variant_id}` must use only ASCII letters, numbers, `_`, or `-`"
    )]
    InvalidVariantId {
        bundle_path: String,
        variant_id: String,
    },
    #[error("bundle {bundle_path} variant `{variant_id}` has an empty label")]
    EmptyVariantLabel {
        bundle_path: String,
        variant_id: String,
    },
    #[error("bundle {bundle_path} variant `{variant_id}` has an empty path")]
    EmptyVariantPath {
        bundle_path: String,
        variant_id: String,
    },
    #[error("bundle {bundle_path} variant `{variant_id}` path `{path}` is not portable")]
    NonPortableVariantPath {
        bundle_path: String,
        variant_id: String,
        path: String,
    },
    #[error(
        "bundle {bundle_path} variant `{variant_id}` path `{path}` must stay inside the bundle"
    )]
    VariantPathEscapesBundle {
        bundle_path: String,
        variant_id: String,
        path: String,
    },
    #[error("bundle {bundle_path} variant `{variant_id}` points to sidecar `{path}`")]
    VariantPointsToSidecar {
        bundle_path: String,
        variant_id: String,
        path: String,
    },
}

pub fn validate_bundle_metadata(
    bundle_path: &str,
    bundle: &BundleMetadata,
) -> BundleValidationResult {
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let bundle_path = display_bundle_path(bundle_path).to_string();

    let default_variant_id = bundle.default_variant_id();
    if default_variant_id.trim().is_empty() {
        return Err(BundleValidationError::EmptyDefaultVariant { bundle_path });
    }

    for variant in &bundle.variants {
        validate_bundle_variant(&bundle_path, variant)?;
        if !ids.insert(variant.id.clone()) {
            return Err(BundleValidationError::DuplicateVariantId {
                bundle_path,
                variant_id: variant.id.clone(),
            });
        }
        if !paths.insert(variant.path.clone()) {
            return Err(BundleValidationError::DuplicateVariantPath {
                bundle_path,
                variant_path: variant.path.clone(),
            });
        }
    }

    if !ids.contains(default_variant_id) {
        return Err(BundleValidationError::DefaultVariantMissing {
            bundle_path,
            default_variant: default_variant_id.to_string(),
        });
    }

    Ok(())
}

pub fn validate_bundle_metadata_with_targets<E>(
    bundle_path: &str,
    bundle: &BundleMetadata,
    mut validate_variant_target: impl FnMut(&BundleVariant) -> Result<(), E>,
) -> Result<(), E>
where
    E: From<BundleValidationError>,
{
    validate_bundle_metadata(bundle_path, bundle).map_err(E::from)?;
    for variant in &bundle.variants {
        validate_variant_target(variant)?;
    }
    Ok(())
}

pub fn validate_bundle_variant(
    bundle_path: &str,
    variant: &BundleVariant,
) -> BundleValidationResult {
    validate_bundle_variant_id(bundle_path, &variant.id)?;
    if variant.label.trim().is_empty() {
        return Err(BundleValidationError::EmptyVariantLabel {
            bundle_path: display_bundle_path(bundle_path).to_string(),
            variant_id: variant.id.clone(),
        });
    }
    validate_relative_bundle_path(bundle_path, &variant.id, &variant.path)
}

pub fn validate_bundle_variant_id(bundle_path: &str, variant_id: &str) -> BundleValidationResult {
    if variant_id.is_empty()
        || !variant_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err(BundleValidationError::InvalidVariantId {
            bundle_path: display_bundle_path(bundle_path).to_string(),
            variant_id: variant_id.to_string(),
        });
    }
    Ok(())
}

pub fn validate_relative_bundle_path(
    bundle_path: &str,
    variant_id: &str,
    rel_path: &str,
) -> BundleValidationResult {
    let bundle_path = display_bundle_path(bundle_path).to_string();
    let variant_id = variant_id.to_string();
    if rel_path.trim().is_empty() {
        return Err(BundleValidationError::EmptyVariantPath {
            bundle_path,
            variant_id,
        });
    }
    if rel_path.starts_with('/')
        || rel_path.contains('\\')
        || rel_path.chars().any(char::is_control)
    {
        return Err(BundleValidationError::NonPortableVariantPath {
            bundle_path,
            variant_id,
            path: rel_path.to_string(),
        });
    }
    for segment in rel_path.split('/') {
        if segment.is_empty() || matches!(segment, "." | "..") {
            return Err(BundleValidationError::VariantPathEscapesBundle {
                bundle_path,
                variant_id,
                path: rel_path.to_string(),
            });
        }
    }
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    if name == "_index.dir.json" || name.ends_with(".meta.json") {
        return Err(BundleValidationError::VariantPointsToSidecar {
            bundle_path,
            variant_id,
            path: rel_path.to_string(),
        });
    }
    Ok(())
}

fn display_bundle_path(path: &str) -> &str {
    if path.is_empty() { "/" } else { path }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(id: &str, path: &str) -> BundleVariant {
        BundleVariant {
            id: id.to_string(),
            path: path.to_string(),
            label: id.to_string(),
            locale: None,
            media_type: None,
        }
    }

    fn static_default(id: &str) -> BundleDefaultVariant {
        BundleDefaultVariant::Static { id: id.to_string() }
    }

    fn locale_default(fallback: &str) -> BundleDefaultVariant {
        BundleDefaultVariant::Locale {
            fallback: fallback.to_string(),
        }
    }

    #[test]
    fn deserializes_object_default_variant_schema() {
        let bundle = serde_json::from_str::<BundleMetadata>(
            r#"{
              "default_variant":{"strategy":"locale","fallback":"en"},
              "variants":[
                {"id":"en","path":"en.md","label":"English","locale":"en"},
                {"id":"ko","path":"ko.md","label":"Korean","locale":"ko"}
              ]
            }"#,
        )
        .unwrap();

        assert_eq!(bundle.default_variant, locale_default("en"));
        assert_eq!(
            bundle
                .selected_variant_for_locale(Some("ko-KR"))
                .map(|variant| variant.id.as_str()),
            Some("ko")
        );
        assert_eq!(
            bundle
                .selected_variant_for_locale(Some("fr-FR"))
                .map(|variant| variant.id.as_str()),
            Some("en")
        );
    }

    #[test]
    fn rejects_legacy_string_default_variant_schema() {
        let parsed = serde_json::from_str::<BundleMetadata>(
            r#"{
              "default_variant":"en",
              "variants":[{"id":"en","path":"en.md","label":"English"}]
            }"#,
        );

        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_unknown_default_variant_fields() {
        let parsed = serde_json::from_str::<BundleMetadata>(
            r#"{
              "default_variant":{"strategy":"static","id":"en","extra":true},
              "variants":[{"id":"en","path":"en.md","label":"English"}]
            }"#,
        );

        assert!(parsed.is_err());
    }

    #[test]
    fn rejects_missing_strategy_payload_fields() {
        let missing_id = serde_json::from_str::<BundleMetadata>(
            r#"{
              "default_variant":{"strategy":"static"},
              "variants":[{"id":"en","path":"en.md","label":"English"}]
            }"#,
        );
        let missing_fallback = serde_json::from_str::<BundleMetadata>(
            r#"{
              "default_variant":{"strategy":"locale"},
              "variants":[{"id":"en","path":"en.md","label":"English"}]
            }"#,
        );

        assert!(missing_id.is_err());
        assert!(missing_fallback.is_err());
    }

    #[test]
    fn rejects_variant_ids_with_dots() {
        let bundle = BundleMetadata {
            default_variant: static_default("ko.md"),
            variants: vec![variant("ko.md", "ko.md")],
        };

        let err = validate_bundle_metadata("writing/foo", &bundle).unwrap_err();
        assert!(matches!(
            err,
            BundleValidationError::InvalidVariantId { variant_id, .. }
                if variant_id == "ko.md"
        ));
    }

    #[test]
    fn accepts_slug_like_non_language_variant_ids() {
        let bundle = BundleMetadata {
            default_variant: static_default("print_pdf"),
            variants: vec![variant("print_pdf", "print.pdf")],
        };

        validate_bundle_metadata("writing/foo", &bundle).unwrap();
    }

    #[test]
    fn rejects_sidecar_variant_paths() {
        let bundle = BundleMetadata {
            default_variant: static_default("en"),
            variants: vec![variant("en", "en.meta.json")],
        };

        let err = validate_bundle_metadata("writing/foo", &bundle).unwrap_err();
        assert!(matches!(
            err,
            BundleValidationError::VariantPointsToSidecar { path, .. }
                if path == "en.meta.json"
        ));
    }

    #[test]
    fn rejects_undeclared_static_default_variant() {
        let bundle = BundleMetadata {
            default_variant: static_default("fr"),
            variants: vec![variant("en", "en.md")],
        };

        let err = validate_bundle_metadata("writing/foo", &bundle).unwrap_err();
        assert!(matches!(
            err,
            BundleValidationError::DefaultVariantMissing { default_variant, .. }
                if default_variant == "fr"
        ));
    }

    #[test]
    fn rejects_undeclared_locale_fallback_variant() {
        let bundle = BundleMetadata {
            default_variant: locale_default("fr"),
            variants: vec![variant("en", "en.md")],
        };

        let err = validate_bundle_metadata("writing/foo", &bundle).unwrap_err();
        assert!(matches!(
            err,
            BundleValidationError::DefaultVariantMissing { default_variant, .. }
                if default_variant == "fr"
        ));
    }
}
