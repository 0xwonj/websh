use std::collections::BTreeMap;

use crate::domain::{BundleVariant, FileType, NodeKind, NodeMetadata, VirtualPath};
use crate::filesystem::{GlobalFs, content_href_for_path};
use crate::mempool::LEDGER_CATEGORIES;
use crate::publication::{PublicationBlock, PublicationChain};
use crate::support::format::iso_date_prefix;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LedgerModel {
    pub filter: LedgerFilter,
    pub entries: Vec<LedgerEntry>,
    pub counts: BTreeMap<String, usize>,
    pub total_count: usize,
    pub restricted_count: usize,
    pub head_hash: String,
    pub genesis_date: String,
    pub latest_date: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LedgerFilter {
    All,
    Category(String),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct LedgerEntry {
    pub block_height: u64,
    pub hash: String,
    pub previous_hash: String,
    pub path: String,
    pub href: String,
    pub title: String,
    pub description: Option<String>,
    pub date: String,
    pub category: String,
    pub kind_chips: Vec<String>,
    pub metrics: NodeMetadata,
    pub size: Option<u64>,
    pub tags: Vec<String>,
    pub variants: Vec<String>,
    pub restricted: bool,
}

pub fn ledger_filter_for_route(request_path: &str, node_path: &VirtualPath) -> LedgerFilter {
    if request_path.trim_matches('/') == "ledger" {
        return LedgerFilter::All;
    }
    node_path
        .segments()
        .next()
        .map(|segment| LedgerFilter::Category(segment.to_string()))
        .unwrap_or(LedgerFilter::All)
}

pub fn build_ledger_model(
    fs: &GlobalFs,
    chain: &PublicationChain,
    filter: &LedgerFilter,
) -> LedgerModel {
    let all_entries = chain
        .blocks
        .iter()
        .rev()
        .filter_map(|block| ledger_entry_for_block(fs, block))
        .collect::<Vec<_>>();
    let total_count = all_entries.len();

    let mut counts = BTreeMap::new();
    for category in LEDGER_CATEGORIES {
        counts.insert((*category).to_string(), 0usize);
    }
    for entry in &all_entries {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }

    let entries = all_entries
        .iter()
        .filter(|entry| filter.includes(entry))
        .cloned()
        .collect::<Vec<_>>();
    let restricted_count = entries.iter().filter(|entry| entry.restricted).count();
    let latest_date = entries
        .first()
        .map(|entry| entry.date.clone())
        .unwrap_or_else(|| "—".to_string());

    let genesis_date = all_entries
        .iter()
        .filter_map(|entry| iso_date_prefix(&entry.date).map(str::to_string))
        .min()
        .unwrap_or_else(|| "—".to_string());

    LedgerModel {
        filter: filter.clone(),
        entries,
        counts,
        total_count,
        restricted_count,
        head_hash: chain.head.clone(),
        genesis_date,
        latest_date,
    }
}

fn ledger_entry_for_block(fs: &GlobalFs, block: &PublicationBlock) -> Option<LedgerEntry> {
    let path = block.path.as_str();
    let node_path = VirtualPath::from_absolute(format!("/{path}")).ok()?;
    let node_meta = fs.node_metadata(&node_path);
    let fallback_title = fallback_file_title(path);
    let title = node_meta
        .and_then(|meta| meta.title())
        .map(str::to_string)
        .unwrap_or(fallback_title);
    let description = node_meta
        .and_then(|meta| meta.description())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let date = node_meta
        .and_then(|meta| meta.date())
        .map(str::to_string)
        .unwrap_or_else(|| "undated".to_string());
    let category = path.split('/').next()?.to_string();
    let kind_chips = kind_chips_for_entry(fs, &node_path, node_meta, &category, path);
    let tags = node_meta.map(NodeMetadata::tags_owned).unwrap_or_default();
    let metric_meta = metric_metadata_for_entry(fs, &node_path, node_meta);
    let size = metric_meta
        .and_then(|meta| meta.size_bytes())
        .or(Some(block.content_bytes));
    let restricted = node_meta.and_then(|meta| meta.access()).is_some();
    let variants = node_meta
        .and_then(|meta| meta.bundle.as_ref())
        .map(|bundle| {
            bundle
                .variants
                .iter()
                .map(|variant| variant.label.clone())
                .collect()
        })
        .unwrap_or_default();
    Some(LedgerEntry {
        block_height: block.height,
        hash: block.hash.clone(),
        previous_hash: block.previous_hash.clone(),
        path: path.to_string(),
        href: content_href_for_path(path),
        title,
        description,
        date,
        category,
        kind_chips,
        metrics: metric_meta.cloned().unwrap_or_default(),
        size,
        tags,
        variants,
        restricted,
    })
}

fn metric_metadata_for_entry<'a>(
    fs: &'a GlobalFs,
    node_path: &VirtualPath,
    node_meta: Option<&'a NodeMetadata>,
) -> Option<&'a NodeMetadata> {
    let meta = node_meta?;
    if meta.is_bundle()
        && let Some(default_meta) = resolve_bundle_default_variant_metadata(fs, node_path, meta)
    {
        return Some(default_meta);
    }
    Some(meta)
}

fn kind_chips_for_entry(
    fs: &GlobalFs,
    node_path: &VirtualPath,
    node_meta: Option<&NodeMetadata>,
    category: &str,
    path: &str,
) -> Vec<String> {
    if let Some(meta) = node_meta
        && meta.is_bundle()
    {
        let chips = bundle_variant_kind_chips(fs, node_path, meta);
        if !chips.is_empty() {
            return chips;
        }
    }
    vec![kind_for_entry(node_meta, category, path)]
}

fn resolve_bundle_default_variant_metadata<'a>(
    fs: &'a GlobalFs,
    bundle_path: &VirtualPath,
    meta: &NodeMetadata,
) -> Option<&'a NodeMetadata> {
    let bundle = meta.bundle.as_ref()?;
    let default_variant = bundle
        .variants
        .iter()
        .find(|variant| variant.id == bundle.default_variant_id())?;
    resolve_bundle_variant_metadata(fs, bundle_path, default_variant)
}

fn bundle_variant_kind_chips(
    fs: &GlobalFs,
    bundle_path: &VirtualPath,
    meta: &NodeMetadata,
) -> Vec<String> {
    let mut chips = Vec::new();
    let Some(bundle) = meta.bundle.as_ref() else {
        return chips;
    };
    for variant in &bundle.variants {
        let Some((target_path, target_meta)) =
            resolve_bundle_variant_target(fs, bundle_path, variant)
        else {
            continue;
        };
        let label = variant_target_kind_label(&target_path, target_meta).to_string();
        if !chips.contains(&label) {
            chips.push(label);
        }
    }
    chips
}

fn resolve_bundle_variant_metadata<'a>(
    fs: &'a GlobalFs,
    bundle_path: &VirtualPath,
    variant: &BundleVariant,
) -> Option<&'a NodeMetadata> {
    let target_path = bundle_child_path(bundle_path, &variant.path)?;
    fs.node_metadata(&target_path)
}

fn resolve_bundle_variant_target<'a>(
    fs: &'a GlobalFs,
    bundle_path: &VirtualPath,
    variant: &BundleVariant,
) -> Option<(VirtualPath, &'a NodeMetadata)> {
    let target_path = bundle_child_path(bundle_path, &variant.path)?;
    let target_meta = fs.node_metadata(&target_path)?;
    Some((target_path, target_meta))
}

fn bundle_child_path(bundle_path: &VirtualPath, rel_path: &str) -> Option<VirtualPath> {
    if rel_path.is_empty()
        || rel_path.starts_with('/')
        || rel_path.contains('\\')
        || rel_path.chars().any(char::is_control)
    {
        return None;
    }
    if rel_path
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    let path = bundle_path.join(rel_path);
    path.starts_with(bundle_path).then_some(path)
}

fn variant_target_kind_label(path: &VirtualPath, meta: &NodeMetadata) -> &'static str {
    match meta.kind {
        NodeKind::Page => match FileType::from_path(path.as_str()) {
            FileType::Markdown => "markdown",
            _ => "document",
        },
        NodeKind::Document => "document",
        NodeKind::Directory | NodeKind::Bundle => "directory",
        NodeKind::App => "app",
        NodeKind::Asset => match FileType::from_path(path.as_str()) {
            FileType::Image => "image",
            _ => "asset",
        },
        NodeKind::Redirect => "link",
        NodeKind::Data => "data",
    }
}

fn fallback_file_title(path: &str) -> String {
    path.rsplit('/')
        .next()
        .and_then(|name| name.split('.').next())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn kind_for_entry(node_meta: Option<&NodeMetadata>, category: &str, path: &str) -> String {
    if let Some(kind) = node_meta.map(|meta| meta.kind) {
        return match kind {
            NodeKind::Bundle => "bundle",
            NodeKind::Directory => "directory",
            NodeKind::Page => "note",
            NodeKind::Document => "document",
            NodeKind::App => "app",
            NodeKind::Asset => "asset",
            NodeKind::Redirect => "link",
            NodeKind::Data => "data",
        }
        .to_string();
    }

    match category {
        "papers" => "paper",
        "projects" => "project",
        "talks" => "talk",
        "writing" => "writing",
        _ if path.ends_with(".asc") => "key",
        _ if path.ends_with(".toml") || path.ends_with(".json") => "data",
        _ => "note",
    }
    .to_string()
}

impl LedgerFilter {
    pub fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }

    pub fn matches(&self, category: &str) -> bool {
        matches!(self, Self::Category(active) if active == category)
    }

    fn includes(&self, entry: &LedgerEntry) -> bool {
        match self {
            Self::All => true,
            Self::Category(category) if LEDGER_CATEGORIES.contains(&category.as_str()) => {
                entry.category == *category
            }
            Self::Category(category) => entry.path.starts_with(&format!("{category}/")),
        }
    }
}
