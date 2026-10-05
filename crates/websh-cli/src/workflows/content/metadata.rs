use anyhow::{Context, bail};
use serde::Deserialize;
use websh_core::domain::{
    AuthoredMetadata, BundleMetadata, DerivedMetadata, NodeKind, NodeMetadata,
};

use crate::CliResult;

/// File authoring input; classification is resolved once into the runtime node.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SourceMetadata {
    pub kind: Option<NodeKind>,
    #[serde(flatten)]
    pub authored: AuthoredMetadata,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectoryDeclaration {
    pub kind: NodeKind,
    pub bundle: Option<BundleMetadata>,
    #[serde(default)]
    pub group: bool,
    #[serde(default)]
    pub authored: AuthoredMetadata,
}

impl DirectoryDeclaration {
    pub(super) fn parse(bytes: &[u8], path: &str) -> CliResult<Self> {
        let declaration: Self = serde_json::from_slice(bytes)
            .with_context(|| format!("parse directory declaration {path}"))?;
        if !declaration.kind.is_directory_like() {
            bail!("directory declaration {path} must have kind directory or bundle");
        }
        if (declaration.kind == NodeKind::Bundle) != declaration.bundle.is_some() {
            bail!(
                "directory declaration {path} requires bundle metadata exactly when kind is bundle"
            );
        }
        if declaration.group && declaration.kind != NodeKind::Directory {
            bail!("directory declaration {path}: only plain directories need explicit grouping");
        }
        Ok(declaration)
    }
}

pub(super) fn file_metadata(
    path: &str,
    bytes: &[u8],
    source: SourceMetadata,
) -> CliResult<NodeMetadata> {
    let kind = source
        .kind
        .unwrap_or_else(|| super::kind_for_content_path(path));
    if kind.is_directory_like() {
        bail!("file {path} cannot have directory or bundle kind");
    }
    let mut derived = super::media::derived_for_bytes(path, bytes)?;
    derived.title = Some(title(path));
    derived.size_bytes = Some(bytes.len() as u64);
    derived.content_sha256 = Some(websh_core::attestation::artifact::sha256_hex(bytes));
    Ok(NodeMetadata {
        kind,
        bundle: None,
        authored: source.authored,
        derived,
    })
}

pub(super) fn directory_metadata(
    path: &str,
    declaration: Option<DirectoryDeclaration>,
) -> NodeMetadata {
    let (kind, bundle, authored) = declaration
        .map(|d| (d.kind, d.bundle, d.authored))
        .unwrap_or((NodeKind::Directory, None, AuthoredMetadata::default()));
    NodeMetadata {
        kind,
        bundle,
        authored,
        derived: DerivedMetadata {
            title: Some(if path.is_empty() {
                "Home".to_string()
            } else {
                path.rsplit('/').next().unwrap_or(path).to_string()
            }),
            ..DerivedMetadata::default()
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
