use std::collections::BTreeMap;

use crate::shared::components::size_summary_parts;
use websh_core::domain::{BundleVariant, FileType, NodeKind, NodeMetadata, VirtualPath};
use websh_core::filesystem::{GlobalFs, content_href_for_path};
use websh_core::mempool::LEDGER_CATEGORIES;
use websh_core::support::format::format_size;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LedgerModel {
    pub(super) filter: LedgerFilter,
    pub(super) entries: Vec<LedgerEntry>,
    pub(super) counts: BTreeMap<String, usize>,
    pub(super) total_count: usize,
    pub(super) restricted_count: usize,
    pub(super) release_id: String,
    pub(super) latest_date: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum LedgerFilter {
    All,
    Category(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LedgerEntry {
    pub(super) path: String,
    pub(super) href: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) date: String,
    pub(super) category: String,
    pub(super) kind_chips: Vec<String>,
    pub(super) meta_line: Vec<String>,
    pub(super) variants: Vec<String>,
    pub(super) restricted: bool,
}

pub(super) fn ledger_filter_for_route(request_path: &str, node_path: &VirtualPath) -> LedgerFilter {
    if request_path.trim_matches('/') == "ledger" {
        return LedgerFilter::All;
    }
    node_path
        .segments()
        .next()
        .map(|segment| LedgerFilter::Category(segment.to_string()))
        .unwrap_or(LedgerFilter::All)
}

pub(super) fn build_ledger_model(
    fs: &GlobalFs,
    publications: &[String],
    release_id: &str,
    filter: &LedgerFilter,
) -> LedgerModel {
    let mut all_entries = publications
        .iter()
        .filter_map(|path| ledger_entry_for_path(fs, path))
        .collect::<Vec<_>>();
    all_entries.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| left.path.cmp(&right.path))
    });
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

    LedgerModel {
        filter: filter.clone(),
        entries,
        counts,
        total_count,
        restricted_count,
        release_id: release_id.to_string(),
        latest_date,
    }
}

fn ledger_entry_for_path(fs: &GlobalFs, path: &str) -> Option<LedgerEntry> {
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
    let size = metric_meta.and_then(|meta| meta.size_bytes());
    let summary_parts = metric_meta
        .map(|meta| {
            size_summary_parts(
                meta.kind,
                meta.word_count(),
                meta.page_count(),
                meta.image_dimensions(),
            )
        })
        .unwrap_or_default();
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
        path: path.to_string(),
        href: content_href_for_path(path),
        title,
        description,
        date,
        category,
        kind_chips,
        meta_line: meta_line_for_entry(summary_parts, size, &tags),
        variants,
        restricted,
    })
}

fn meta_line_for_entry(
    summary_parts: Vec<String>,
    size: Option<u64>,
    tags: &[String],
) -> Vec<String> {
    let mut out = summary_parts;
    if out.is_empty()
        && let Some(bytes) = size
    {
        out.push(format_size(Some(bytes), false));
    }
    out.extend(tags.iter().take(3).cloned());
    if out.is_empty() {
        out.push("content".to_string());
    }
    out
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
    pub(super) fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }

    pub(super) fn matches(&self, category: &str) -> bool {
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
    use wasm_bindgen_test::*;
    use websh_core::domain::{
        AuthoredMetadata, BundleMetadata, BundleVariant, DerivedMetadata, EntryExtensions,
        ImageDim, NodeKind,
    };

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
            default_variant: websh_core::domain::BundleDefaultVariant::Static {
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
            default_variant: websh_core::domain::BundleDefaultVariant::Locale {
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
        build_ledger_model(fs, &[path.to_string()], "release", &LedgerFilter::All)
            .entries
            .into_iter()
            .next()
            .unwrap()
    }

    #[wasm_bindgen_test]
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
        assert_eq!(entry.meta_line, labels(&["2,140 words", "9 min"]));
        assert_eq!(entry.variants, labels(&["English", "Korean"]));
    }

    #[wasm_bindgen_test]
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

    #[wasm_bindgen_test]
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

        assert_eq!(entry.meta_line, labels(&["12 pages"]));
    }

    #[wasm_bindgen_test]
    fn missing_default_variant_metadata_uses_content_label() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(
            vp("/writing/missing"),
            bundle_meta("en", vec![variant("en", "en.md", "English")]),
        );

        let entry = single_entry(&fs, "writing/missing");

        assert_eq!(entry.kind_chips, labels(&["bundle"]));
        assert_eq!(entry.meta_line, labels(&["content"]));
    }
}
