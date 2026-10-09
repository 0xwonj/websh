use crate::shared::components::size_summary_parts;
pub(super) use websh_core::publication::{
    LedgerEntry, LedgerFilter, LedgerModel, build_ledger_model, ledger_filter_for_route,
};
use websh_core::support::format::format_size;

pub(super) fn entry_meta_line(entry: &LedgerEntry) -> Vec<String> {
    let meta = &entry.metrics;
    meta_line_for_entry(
        size_summary_parts(
            meta.kind,
            meta.word_count(),
            meta.page_count(),
            meta.image_dimensions(),
        ),
        entry.size,
        &entry.tags,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use websh_core::domain::{
        AuthoredMetadata, BundleMetadata, BundleVariant, DerivedMetadata, EntryExtensions,
        ImageDim, NodeKind,
    };
    use websh_core::domain::{NodeMetadata, VirtualPath};
    use websh_core::filesystem::GlobalFs;
    use websh_core::publication::{PublicationBlock, PublicationChain};

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

    #[wasm_bindgen_test]
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
        assert_eq!(entry_meta_line(&entry), labels(&["2,140 words", "9 min"]));
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

        assert_eq!(entry_meta_line(&entry), labels(&["12 pages"]));
    }

    #[wasm_bindgen_test]
    fn missing_default_variant_metadata_uses_committed_content_size() {
        let mut fs = GlobalFs::empty();
        fs.upsert_directory(
            vp("/writing/missing"),
            bundle_meta("en", vec![variant("en", "en.md", "English")]),
        );

        let entry = single_entry(&fs, "writing/missing");

        assert_eq!(entry.kind_chips, labels(&["bundle"]));
        assert_eq!(entry_meta_line(&entry), labels(&["12.3K"]));
    }
}
