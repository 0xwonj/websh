use anyhow::{Context, bail};
use serde::Deserialize;
use websh_core::domain::{BundleMetadata, Fields, NodeKind, NodeMetadata, RendererKind};

use crate::CliResult;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectoryDeclaration {
    pub kind: NodeKind,
    pub bundle: Option<BundleMetadata>,
    #[serde(default)]
    pub authored: Fields,
}

impl DirectoryDeclaration {
    pub(super) fn parse(bytes: &[u8], path: &str) -> CliResult<Self> {
        let declaration: Self = serde_json::from_slice(bytes)
            .with_context(|| format!("parse directory declaration {path}"))?;
        validate_authored(&declaration.authored)?;
        if !declaration.kind.is_directory_like() {
            bail!("directory declaration {path} must have kind directory or bundle");
        }
        if (declaration.kind == NodeKind::Bundle) != declaration.bundle.is_some() {
            bail!(
                "directory declaration {path} requires bundle metadata exactly when kind is bundle"
            );
        }
        if let Some(kind) = declaration.authored.kind
            && kind != declaration.kind
        {
            bail!("directory declaration {path} has conflicting authored kind");
        }
        Ok(declaration)
    }
}

/// Source metadata contains only author decisions. Integrity and media fields
/// always come from the bytes in the snapshot.
pub(super) fn validate_authored(fields: &Fields) -> CliResult {
    if fields.page_size.is_some()
        || fields.page_count.is_some()
        || fields.rotation.is_some()
        || fields.image_dimensions.is_some()
        || fields.size_bytes.is_some()
        || fields.modified_at.is_some()
        || fields.content_sha256.is_some()
        || fields.word_count.is_some()
        || fields.child_count.is_some()
    {
        bail!("authored metadata contains computed fields");
    }
    Ok(())
}

pub(super) fn file_metadata(path: &str, bytes: &[u8], authored: Fields) -> CliResult<NodeMetadata> {
    validate_authored(&authored)?;
    let kind = authored
        .kind
        .unwrap_or_else(|| super::kind_for_content_path(path));
    if kind.is_directory_like() {
        bail!("file {path} cannot have directory or bundle kind");
    }
    let mut derived = super::media::derived_for_bytes(path, bytes)?;
    derived.title = Some(title(path));
    derived.kind = Some(kind);
    derived.renderer = renderer(kind, path);
    derived.size_bytes = Some(bytes.len() as u64);
    derived.content_sha256 = Some(websh_core::attestation::artifact::sha256_hex(bytes));
    Ok(NodeMetadata {
        kind,
        bundle: None,
        authored,
        derived,
    })
}

pub(super) fn directory_metadata(
    path: &str,
    declaration: Option<DirectoryDeclaration>,
) -> NodeMetadata {
    let (kind, bundle, authored) = declaration
        .map(|d| (d.kind, d.bundle, d.authored))
        .unwrap_or((NodeKind::Directory, None, Fields::default()));
    NodeMetadata {
        kind,
        bundle,
        authored,
        derived: Fields {
            title: Some(if path.is_empty() {
                "Home".to_string()
            } else {
                path.rsplit('/').next().unwrap_or(path).to_string()
            }),
            kind: Some(kind),
            renderer: renderer(kind, path),
            ..Fields::default()
        },
    }
}

fn title(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

fn renderer(kind: NodeKind, path: &str) -> Option<RendererKind> {
    use RendererKind::*;
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|v| v.to_str());
    match (kind, extension) {
        (NodeKind::Page, Some("md")) => Some(MarkdownPage),
        (NodeKind::Page, Some("html" | "htm")) => Some(HtmlPage),
        (NodeKind::Document, Some("pdf")) => Some(Pdf),
        (NodeKind::Asset, Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")) => Some(Image),
        (NodeKind::Redirect, _) => Some(Redirect),
        (NodeKind::App, _) => Some(TerminalApp),
        (NodeKind::Directory, _) => Some(DirectoryListing),
        _ => None,
    }
}
