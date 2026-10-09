use std::collections::BTreeMap;

use crate::domain::{BundleVariant, FileType, NodeKind, NodeMetadata, VirtualPath};
use crate::filesystem::{GlobalFs, content_href_for_path};
use crate::mempool::LEDGER_CATEGORIES;
use crate::publication::{PublicationBlock, PublicationChain};
use crate::support::format::iso_date_prefix;

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerFilter {
    All,
    Category(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

// Cryptographic input is explicit and independent of display-model serialization.
// Field order is part of this native contract; hrefs and duplicate metadata are excluded.
pub(super) fn commitment_bytes(model: &LedgerModel) -> Result<Vec<u8>, serde_json::Error> {
    #[derive(serde::Serialize)]
    struct Entry<'a> {
        height: u64,
        hash: &'a str,
        previous_hash: &'a str,
        path: &'a str,
        title: &'a str,
        description: &'a Option<String>,
        date: &'a str,
        category: &'a str,
        kinds: &'a [String],
        metric_kind: NodeKind,
        words: Option<u32>,
        pages: Option<u32>,
        dimensions: Option<&'a crate::domain::ImageDim>,
        bytes: Option<u64>,
        tags: &'a [String],
        variants: &'a [String],
        restricted: bool,
    }
    #[derive(serde::Serialize)]
    struct Commitment<'a> {
        category: Option<&'a str>,
        entries: Vec<Entry<'a>>,
        counts: &'a BTreeMap<String, usize>,
        total: usize,
        restricted: usize,
        head: &'a str,
        genesis: &'a str,
        latest: &'a str,
    }
    serde_json::to_vec(&Commitment {
        category: match &model.filter {
            LedgerFilter::All => None,
            LedgerFilter::Category(c) => Some(c),
        },
        entries: model
            .entries
            .iter()
            .map(|e| Entry {
                height: e.block_height,
                hash: &e.hash,
                previous_hash: &e.previous_hash,
                path: &e.path,
                title: &e.title,
                description: &e.description,
                date: &e.date,
                category: &e.category,
                kinds: &e.kind_chips,
                metric_kind: e.metrics.kind,
                words: e.metrics.word_count(),
                pages: e.metrics.page_count(),
                dimensions: e.metrics.image_dimensions(),
                bytes: e.size,
                tags: &e.tags,
                variants: &e.variants,
                restricted: e.restricted,
            })
            .collect(),
        counts: &model.counts,
        total: model.total_count,
        restricted: model.restricted_count,
        head: &model.head_hash,
        genesis: &model.genesis_date,
        latest: &model.latest_date,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        AuthoredMetadata, BundleMetadata, BundleVariant, DerivedMetadata, EntryExtensions,
        ImageDim, NodeKind,
    };
    use crate::domain::{NodeMetadata, VirtualPath};
    use crate::filesystem::GlobalFs;
    use crate::publication::{PublicationBlock, PublicationChain};

    fn labels(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn vp(path: &str) -> VirtualPath {
        VirtualPath::from_absolute(path).unwrap()
    }

    fn variant(id: &str, path: &str, label: &str) -> BundleVariant {
        BundleVariant {
            id: id.to_string(),
            path: path.to_string(),
            label: label.to_string(),
            locale: None,
            media_type: None,
        }
    }

    fn meta(kind: NodeKind) -> NodeMetadata {
        NodeMetadata {
            kind,
            bundle: None,
            authored: AuthoredMetadata::default(),
            derived: DerivedMetadata::default(),
        }
    }

    fn bundle_meta(default_variant: &str, variants: Vec<BundleVariant>) -> NodeMetadata {
        let mut meta = meta(NodeKind::Bundle);
        meta.bundle = Some(BundleMetadata {
            default_variant: crate::domain::BundleDefaultVariant::Static {
                id: default_variant.to_string(),
            },
            variants,
        });
        meta.authored.title = Some("Bundle".to_string());
        meta.authored.date = Some("2026-01-01".to_string());
        meta
    }

    fn locale_bundle_meta(fallback: &str, variants: Vec<BundleVariant>) -> NodeMetadata {
        let mut meta = meta(NodeKind::Bundle);
        meta.bundle = Some(BundleMetadata {
            default_variant: crate::domain::BundleDefaultVariant::Locale {
                fallback: fallback.to_string(),
            },
            variants,
        });
        meta.authored.title = Some("Bundle".to_string());
        meta.authored.date = Some("2026-01-01".to_string());
        meta
    }

    fn markdown_meta(words: u32) -> NodeMetadata {
        let mut meta = meta(NodeKind::Page);
        meta.derived.word_count = Some(words);
        meta.derived.size_bytes = Some(1_000);
        meta
    }

    fn pdf_meta(pages: u32) -> NodeMetadata {
        let mut meta = meta(NodeKind::Document);
        meta.derived.page_count = Some(pages);
        meta.derived.size_bytes = Some(10_000);
        meta
    }

    fn image_meta() -> NodeMetadata {
        let mut meta = meta(NodeKind::Asset);
        meta.derived.image_dimensions = Some(ImageDim {
            width: 640,
            height: 480,
        });
        meta.derived.size_bytes = Some(12_000);
        meta
    }

    fn upsert_file(fs: &mut GlobalFs, path: &str, meta: NodeMetadata) {
        fs.upsert_file(vp(path), String::new(), meta, EntryExtensions::default());
    }

    fn single_entry(fs: &GlobalFs, path: &str) -> LedgerEntry {
        let chain = PublicationChain {
            blocks: vec![PublicationBlock {
                path: path.into(),
                height: 1,
                hash: "0x01".into(),
                previous_hash: "0x00".into(),
                content_bytes: 12_345,
            }],
            head: "0x01".into(),
        };
        build_ledger_model(fs, &chain, &LedgerFilter::All)
            .entries
            .into_iter()
            .next()
            .unwrap()
    }

    #[test]
    fn category_filter_preserves_chain_head_heights_and_links() {
        let mut fs = GlobalFs::empty();
        let paths = ["writing/old.md", "papers/middle.md", "writing/new.md"];
        let blocks = paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                upsert_file(&mut fs, &format!("/{path}"), markdown_meta(100));
                PublicationBlock {
                    path: (*path).into(),
                    height: index as u64 + 1,
                    hash: format!("hash{}", index + 1),
                    previous_hash: format!("hash{index}"),
                    content_bytes: 1_000,
                }
            })
            .collect();
        let chain = PublicationChain {
            blocks,
            head: "hash3".into(),
        };
        let all = build_ledger_model(&fs, &chain, &LedgerFilter::All);
        let writing = build_ledger_model(&fs, &chain, &LedgerFilter::Category("writing".into()));

        assert_eq!(writing.head_hash, all.head_hash);
        assert_eq!(writing.total_count, 3);
        assert_eq!(
            writing.entries,
            vec![all.entries[0].clone(), all.entries[2].clone()]
        );
        assert_eq!(writing.entries[0].block_height, 3);
        assert_eq!(writing.entries[0].previous_hash, "hash2");
        assert_eq!(writing.entries[1].block_height, 1);
    }

    #[test]
    fn markdown_bundle_dedupes_kind_chip_and_uses_default_variant_metrics() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(
            vp("/writing/foo"),
            locale_bundle_meta(
                "en",
                vec![
                    variant("en", "en.md", "English"),
                    variant("ko", "ko.md", "Korean"),
                ],
            ),
        );
        upsert_file(&mut fs, "/writing/foo/en.md", markdown_meta(2_140));
        upsert_file(&mut fs, "/writing/foo/ko.md", markdown_meta(1_200));

        let entry = single_entry(&fs, "writing/foo");

        assert_eq!(entry.kind_chips, labels(&["markdown"]));
        assert_eq!(entry.metrics.word_count(), Some(2_140));
        assert_eq!(entry.variants, labels(&["English", "Korean"]));
    }

    #[test]
    fn mixed_bundle_kind_chips_follow_variant_declaration_order() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(
            vp("/writing/mixed"),
            bundle_meta(
                "cover",
                vec![
                    variant("cover", "cover.png", "Cover"),
                    variant("en", "en.md", "English"),
                    variant("print", "print.pdf", "Print"),
                    variant("ko", "ko.md", "Korean"),
                ],
            ),
        );
        upsert_file(&mut fs, "/writing/mixed/cover.png", image_meta());
        upsert_file(&mut fs, "/writing/mixed/en.md", markdown_meta(900));
        upsert_file(&mut fs, "/writing/mixed/print.pdf", pdf_meta(4));
        upsert_file(&mut fs, "/writing/mixed/ko.md", markdown_meta(850));

        let entry = single_entry(&fs, "writing/mixed");

        assert_eq!(entry.kind_chips, labels(&["image", "markdown", "document"]));
    }

    #[test]
    fn pdf_default_variant_uses_page_count_metric() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(
            vp("/papers/foo"),
            bundle_meta(
                "print",
                vec![
                    variant("en", "en.md", "English"),
                    variant("print", "print.pdf", "Print"),
                ],
            ),
        );
        upsert_file(&mut fs, "/papers/foo/en.md", markdown_meta(2_140));
        upsert_file(&mut fs, "/papers/foo/print.pdf", pdf_meta(12));

        let entry = single_entry(&fs, "papers/foo");

        assert_eq!(entry.metrics.page_count(), Some(12));
    }

    #[test]
    fn missing_default_variant_metadata_uses_committed_content_size() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(
            vp("/writing/missing"),
            bundle_meta("en", vec![variant("en", "en.md", "English")]),
        );

        let entry = single_entry(&fs, "writing/missing");

        assert_eq!(entry.kind_chips, labels(&["bundle"]));
        assert_eq!(entry.size, Some(12_345));
    }
}
