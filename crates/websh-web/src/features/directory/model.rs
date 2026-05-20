use websh_core::domain::{DirEntry, FileType, NodeKind, NodeMetadata, VirtualPath};
use websh_core::filesystem::{
    BundleVariantContext, GlobalFs, bundle_variant_href, content_href_for_path,
};
use websh_core::support::format::{
    format_date_iso, format_size, format_thousands_u32, reading_time_minutes,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DirectoryModel {
    pub(super) kind: NodeKind,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) date: Option<String>,
    pub(super) tags: Vec<String>,
    pub(super) variants: Vec<DirectoryVariantLink>,
    pub(super) child_count: Option<u32>,
    pub(super) entry_count: usize,
    pub(super) groups: Vec<DirectoryListingGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DirectoryVariantLink {
    pub(super) label: String,
    pub(super) href: String,
    pub(super) active: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DirectoryListingGroup {
    pub(super) kind: DirectoryEntryGroupKind,
    pub(super) entries: Vec<DirectoryListingEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DirectoryEntryGroupKind {
    Directories,
    Bundles,
    Markdown,
    Documents,
    Images,
    Apps,
    Links,
    Data,
    Assets,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DirectoryListingEntry {
    pub(super) group_kind: DirectoryEntryGroupKind,
    pub(super) name: String,
    pub(super) href: String,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    pub(super) extent: String,
    pub(super) secondary_meta: Option<String>,
    pub(super) tags: Vec<String>,
}

const GROUP_ORDER: [DirectoryEntryGroupKind; 9] = [
    DirectoryEntryGroupKind::Directories,
    DirectoryEntryGroupKind::Bundles,
    DirectoryEntryGroupKind::Markdown,
    DirectoryEntryGroupKind::Documents,
    DirectoryEntryGroupKind::Images,
    DirectoryEntryGroupKind::Apps,
    DirectoryEntryGroupKind::Links,
    DirectoryEntryGroupKind::Data,
    DirectoryEntryGroupKind::Assets,
];

pub(super) fn build_directory_model(fs: &GlobalFs, path: &VirtualPath) -> DirectoryModel {
    build_directory_model_with_bundle_context(fs, path, None)
}

pub(super) fn build_directory_model_with_bundle_context(
    fs: &GlobalFs,
    path: &VirtualPath,
    bundle_context: Option<&BundleVariantContext>,
) -> DirectoryModel {
    let node_meta = fs.node_metadata(path);
    let kind = node_meta
        .map(NodeMetadata::effective_kind)
        .unwrap_or(NodeKind::Directory);
    let fallback_title = title_for_path(path);
    let mut title = node_meta
        .and_then(NodeMetadata::title)
        .map(str::to_string)
        .unwrap_or(fallback_title);
    let mut description = node_meta
        .and_then(NodeMetadata::description)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let mut date = node_meta.and_then(display_date);
    let mut tags = node_meta.map(NodeMetadata::tags_owned).unwrap_or_default();
    let variants = bundle_context
        .and_then(|context| {
            let bundle_meta = fs.node_metadata(&context.bundle_path)?;
            if let Some(variant_title) = node_meta
                .and_then(|meta| meta.authored.title.as_deref())
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                title = variant_title.to_string();
            } else if let Some(bundle_title) = bundle_meta.title() {
                title = bundle_title.to_string();
            }
            date = display_date(bundle_meta).or_else(|| date.clone());
            let bundle_tags = bundle_meta.tags_owned();
            if !bundle_tags.is_empty() {
                tags = bundle_tags;
            }
            let bundle_description = bundle_meta
                .description()
                .map(|text| text.trim().to_string())
                .filter(|text| !text.is_empty());
            description = description.clone().or(bundle_description);
            bundle_meta.bundle.as_ref().map(|bundle| {
                bundle
                    .variants
                    .iter()
                    .map(|variant| DirectoryVariantLink {
                        label: variant.label.clone(),
                        href: bundle_variant_href(&context.bundle_path, bundle, variant),
                        active: variant.id == context.variant_id,
                    })
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    let child_count = node_meta.and_then(NodeMetadata::child_count);
    let entries: Vec<_> = fs
        .list_dir(path)
        .unwrap_or_default()
        .into_iter()
        .filter(is_visible_directory_entry)
        .map(|entry| directory_listing_entry(fs, entry))
        .collect();
    let entry_count = entries.len();
    let groups = group_directory_entries(entries);

    DirectoryModel {
        kind,
        title,
        description,
        date,
        tags,
        variants,
        child_count,
        entry_count,
        groups,
    }
}

fn directory_listing_entry(fs: &GlobalFs, entry: DirEntry) -> DirectoryListingEntry {
    let kind = entry_kind(&entry);
    let group_kind = DirectoryEntryGroupKind::for_entry(&entry, kind);
    let name = display_name(&entry);
    let title = entry
        .meta
        .as_ref()
        .and_then(NodeMetadata::title)
        .map(str::to_string)
        .filter(|title| !same_title_as_name(title, &entry.name));
    let description = entry
        .meta
        .as_ref()
        .and_then(NodeMetadata::description)
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    let (extent, secondary_meta) = entry_metrics(fs, &entry, group_kind, kind);
    let tags = entry
        .meta
        .as_ref()
        .map(NodeMetadata::tags_owned)
        .unwrap_or_default()
        .into_iter()
        .take(3)
        .collect();

    DirectoryListingEntry {
        group_kind,
        name,
        href: content_href_for_path(entry.path.as_str()),
        title,
        description,
        extent,
        secondary_meta,
        tags,
    }
}

fn group_directory_entries(entries: Vec<DirectoryListingEntry>) -> Vec<DirectoryListingGroup> {
    GROUP_ORDER
        .into_iter()
        .filter_map(|kind| {
            let group_entries = entries
                .iter()
                .filter(|entry| entry.group_kind == kind)
                .cloned()
                .collect::<Vec<_>>();
            (!group_entries.is_empty()).then_some(DirectoryListingGroup {
                kind,
                entries: group_entries,
            })
        })
        .collect()
}

fn entry_kind(entry: &DirEntry) -> NodeKind {
    entry
        .meta
        .as_ref()
        .map(NodeMetadata::effective_kind)
        .unwrap_or(if entry.is_dir {
            NodeKind::Directory
        } else {
            NodeKind::Asset
        })
}

fn entry_metrics(
    fs: &GlobalFs,
    entry: &DirEntry,
    group_kind: DirectoryEntryGroupKind,
    kind: NodeKind,
) -> (String, Option<String>) {
    let meta = entry.meta.as_ref();

    if matches!(
        group_kind,
        DirectoryEntryGroupKind::Directories | DirectoryEntryGroupKind::Bundles
    ) {
        let visible_count = fs
            .list_dir(&entry.path)
            .unwrap_or_default()
            .into_iter()
            .filter(is_visible_directory_entry)
            .count();
        let label = if group_kind == DirectoryEntryGroupKind::Bundles {
            meta.and_then(|meta| meta.bundle.as_ref())
                .map(|bundle| count_label(bundle.variants.len(), "variant", "variants"))
                .unwrap_or_else(|| count_label(visible_count, "item", "items"))
        } else {
            count_label(visible_count, "item", "items")
        };
        return (label, meta.and_then(display_date));
    }

    let Some(meta) = meta else {
        return (kind_label(kind).to_string(), None);
    };

    match group_kind {
        DirectoryEntryGroupKind::Markdown => {
            if let Some(words) = meta.word_count() {
                (
                    format!(
                        "{} words · {} min",
                        format_thousands_u32(words),
                        reading_time_minutes(words)
                    ),
                    display_date(meta),
                )
            } else {
                (fallback_size_or_kind(meta, kind), display_date(meta))
            }
        }
        DirectoryEntryGroupKind::Documents => {
            if let Some(pages) = meta.page_count() {
                (
                    count_label(pages as usize, "page", "pages"),
                    meta.size_bytes()
                        .map(|bytes| format_size(Some(bytes), false)),
                )
            } else {
                (fallback_size_or_kind(meta, kind), display_date(meta))
            }
        }
        DirectoryEntryGroupKind::Images => {
            if let Some(dim) = meta.image_dimensions() {
                (
                    format!("{}×{}", dim.width, dim.height),
                    meta.size_bytes()
                        .map(|bytes| format_size(Some(bytes), false)),
                )
            } else {
                (fallback_size_or_kind(meta, kind), display_date(meta))
            }
        }
        DirectoryEntryGroupKind::Apps
        | DirectoryEntryGroupKind::Links
        | DirectoryEntryGroupKind::Data
        | DirectoryEntryGroupKind::Assets => {
            (fallback_size_or_kind(meta, kind), display_date(meta))
        }
        DirectoryEntryGroupKind::Directories | DirectoryEntryGroupKind::Bundles => {
            unreachable!("directory-like entries returned before metadata-specific metrics")
        }
    }
}

fn display_date(meta: &NodeMetadata) -> Option<String> {
    meta.date()
        .map(str::to_string)
        .filter(|text| !text.trim().is_empty())
        .or_else(|| meta.modified_at().map(format_date_iso))
}

fn fallback_size_or_kind(meta: &NodeMetadata, kind: NodeKind) -> String {
    if let Some(bytes) = meta.size_bytes() {
        return format_size(Some(bytes), false);
    }
    kind_label(kind).to_string()
}

fn count_label(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

fn is_visible_directory_entry(entry: &DirEntry) -> bool {
    let name = entry.name.as_str();
    !(name == "_index.dir.json"
        || name == "manifest.json"
        || name == ".DS_Store"
        || name == ".gitkeep"
        || name.ends_with(".meta.json"))
}

fn display_name(entry: &DirEntry) -> String {
    entry.name.clone()
}

fn same_title_as_name(title: &str, name: &str) -> bool {
    let title = title.trim();
    if title.is_empty() || title == name {
        return true;
    }
    name.rsplit_once('.')
        .map(|(stem, _)| title == stem)
        .unwrap_or(false)
}

pub(super) fn kind_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Page => "page",
        NodeKind::Document => "document",
        NodeKind::Asset => "asset",
        NodeKind::App => "app",
        NodeKind::Redirect => "redirect",
        NodeKind::Data => "data",
        NodeKind::Directory => "directory",
        NodeKind::Bundle => "bundle",
    }
}

pub(super) fn surface_kind_label(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Page => "Note",
        NodeKind::Document => "Doc",
        NodeKind::App => "App",
        NodeKind::Asset => "Asset",
        NodeKind::Redirect => "Link",
        NodeKind::Data => "Data",
        NodeKind::Directory => "Folder",
        NodeKind::Bundle => "Bundle",
    }
}

impl DirectoryEntryGroupKind {
    pub(super) fn attr(self) -> &'static str {
        match self {
            Self::Directories => "directories",
            Self::Bundles => "bundles",
            Self::Markdown => "markdown",
            Self::Documents => "documents",
            Self::Images => "images",
            Self::Apps => "apps",
            Self::Links => "links",
            Self::Data => "data",
            Self::Assets => "assets",
        }
    }

    pub(super) fn label(self) -> &'static str {
        self.attr()
    }

    pub(super) fn glyph(self) -> &'static str {
        match self {
            Self::Directories | Self::Bundles => "▸",
            Self::Markdown => "¶",
            Self::Documents => "▤",
            Self::Images => "▧",
            Self::Apps => "λ",
            Self::Links => "↗",
            Self::Data => "{}",
            Self::Assets => "·",
        }
    }

    pub(super) fn summary(self) -> &'static str {
        match self {
            Self::Directories => "listing view",
            Self::Bundles => "bundle reader",
            Self::Markdown => "markdown reader",
            Self::Documents => "document reader",
            Self::Images => "image preview",
            Self::Apps => "app surface",
            Self::Links => "redirect target",
            Self::Data => "raw data",
            Self::Assets => "asset file",
        }
    }

    fn for_entry(entry: &DirEntry, kind: NodeKind) -> Self {
        if kind == NodeKind::Bundle {
            return Self::Bundles;
        }
        if entry.is_dir {
            return Self::Directories;
        }

        match kind {
            NodeKind::Page if FileType::from_path(entry.path.as_str()) == FileType::Markdown => {
                Self::Markdown
            }
            NodeKind::Page => Self::Markdown,
            NodeKind::Document => Self::Documents,
            NodeKind::Asset if FileType::from_path(entry.path.as_str()) == FileType::Image => {
                Self::Images
            }
            NodeKind::Asset => Self::Assets,
            NodeKind::App => Self::Apps,
            NodeKind::Redirect => Self::Links,
            NodeKind::Data => Self::Data,
            NodeKind::Directory | NodeKind::Bundle => Self::Directories,
        }
    }
}

fn title_for_path(path: &VirtualPath) -> String {
    if path.is_root() {
        return "Home".to_string();
    }
    path.file_name()
        .map(str::to_string)
        .unwrap_or_else(|| path.as_str().trim_matches('/').to_string())
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;
    use websh_core::domain::{
        BundleMetadata, BundleVariant, EntryExtensions, Fields, ImageDim, NodeMetadata,
        SCHEMA_VERSION,
    };
    use websh_core::ports::{ScannedDirectory, ScannedFile, ScannedSubtree};

    wasm_bindgen_test_configure!(run_in_browser);

    fn meta(kind: NodeKind, title: Option<&str>) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind,
            bundle: None,
            authored: Fields {
                title: title.map(str::to_string),
                ..Fields::default()
            },
            derived: Fields {
                kind: Some(kind),
                ..Fields::default()
            },
        }
    }

    fn bundle_meta(title: &str) -> NodeMetadata {
        NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Bundle,
            bundle: Some(BundleMetadata {
                default_variant: websh_core::domain::BundleDefaultVariant::Static {
                    id: "en".to_string(),
                },
                variants: vec![
                    BundleVariant {
                        id: "en".to_string(),
                        path: "en.md".to_string(),
                        label: "English".to_string(),
                        locale: Some("en".to_string()),
                        media_type: None,
                    },
                    BundleVariant {
                        id: "ko".to_string(),
                        path: "ko.md".to_string(),
                        label: "Korean".to_string(),
                        locale: Some("ko".to_string()),
                        media_type: None,
                    },
                ],
            }),
            authored: Fields {
                title: Some(title.to_string()),
                ..Fields::default()
            },
            derived: Fields {
                kind: Some(NodeKind::Bundle),
                child_count: Some(2),
                ..Fields::default()
            },
        }
    }

    fn file(path: &str, meta: NodeMetadata) -> ScannedFile {
        ScannedFile {
            path: path.to_string(),
            meta,
            extensions: EntryExtensions::default(),
        }
    }

    fn dir(path: &str, meta: NodeMetadata) -> ScannedDirectory {
        ScannedDirectory {
            path: path.to_string(),
            meta,
        }
    }

    fn group(model: &DirectoryModel, kind: DirectoryEntryGroupKind) -> &DirectoryListingGroup {
        model
            .groups
            .iter()
            .find(|group| group.kind == kind)
            .expect("catalog group")
    }

    #[wasm_bindgen_test]
    fn directory_model_hides_sidecars_and_keeps_visible_site_children() {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: ".site/now.toml".to_string(),
                    meta: meta(NodeKind::Document, Some("now")),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: ".site/now.meta.json".to_string(),
                    meta: meta(NodeKind::Data, None),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: ".site/_index.dir.json".to_string(),
                    meta: meta(NodeKind::Data, None),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: ".site/.gitkeep".to_string(),
                    meta: meta(NodeKind::Data, None),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: ".site/errors/404.md".to_string(),
                    meta: meta(NodeKind::Page, Some("404 response policy")),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: ".site".to_string(),
                    meta: meta(NodeKind::Directory, Some("Site")),
                },
                ScannedDirectory {
                    path: ".site/errors".to_string(),
                    meta: meta(NodeKind::Directory, Some("errors")),
                },
            ],
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        let model = build_directory_model(&fs, &VirtualPath::from_absolute("/.site").unwrap());

        assert_eq!(model.kind, NodeKind::Directory);
        assert_eq!(model.title, "Site");
        assert_eq!(model.entry_count, 2);
        assert_eq!(
            model
                .groups
                .iter()
                .map(|group| group.kind)
                .collect::<Vec<_>>(),
            vec![
                DirectoryEntryGroupKind::Directories,
                DirectoryEntryGroupKind::Documents
            ]
        );
        assert_eq!(
            group(&model, DirectoryEntryGroupKind::Directories)
                .entries
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["errors"]
        );
        assert_eq!(
            group(&model, DirectoryEntryGroupKind::Directories).entries[0].href,
            "#/.site/errors"
        );
        assert_eq!(
            group(&model, DirectoryEntryGroupKind::Documents).entries[0].href,
            "#/.site/now.toml"
        );
    }

    #[wasm_bindgen_test]
    fn directory_model_groups_catalog_kinds_and_prefers_reader_metrics() {
        let mut markdown = meta(NodeKind::Page, Some("Readme"));
        markdown.authored.date = Some("2026-05-20".to_string());
        markdown.derived.word_count = Some(2_140);
        markdown.derived.size_bytes = Some(12_000);

        let mut document = meta(NodeKind::Document, Some("Slides"));
        document.derived.page_count = Some(31);
        document.derived.size_bytes = Some(3_417_199);

        let mut image = meta(NodeKind::Asset, Some("Cover"));
        image.derived.image_dimensions = Some(ImageDim {
            width: 1920,
            height: 1080,
        });
        image.derived.size_bytes = Some(640_000);

        let mut data = meta(NodeKind::Data, Some("Config"));
        data.derived.size_bytes = Some(161);

        let mut asset = meta(NodeKind::Asset, Some("Archive"));
        asset.derived.size_bytes = Some(2048);

        let snapshot = ScannedSubtree {
            files: vec![
                file("README.md", markdown),
                file("slides.pdf", document),
                file("cover.png", image),
                file("demo.app", meta(NodeKind::App, Some("Demo"))),
                file("site.link", meta(NodeKind::Redirect, Some("Site"))),
                file("config.json", data),
                file("archive.bin", asset),
                file("article/en.md", meta(NodeKind::Page, Some("English"))),
                file("article/ko.md", meta(NodeKind::Page, Some("Korean"))),
            ],
            directories: vec![
                dir("docs", meta(NodeKind::Directory, Some("docs"))),
                dir("article", bundle_meta("Article")),
            ],
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        let model = build_directory_model(&fs, &VirtualPath::root());

        assert_eq!(model.entry_count, 9);
        assert_eq!(
            model
                .groups
                .iter()
                .map(|group| group.kind)
                .collect::<Vec<_>>(),
            vec![
                DirectoryEntryGroupKind::Directories,
                DirectoryEntryGroupKind::Bundles,
                DirectoryEntryGroupKind::Markdown,
                DirectoryEntryGroupKind::Documents,
                DirectoryEntryGroupKind::Images,
                DirectoryEntryGroupKind::Apps,
                DirectoryEntryGroupKind::Links,
                DirectoryEntryGroupKind::Data,
                DirectoryEntryGroupKind::Assets,
            ]
        );

        let bundle = &group(&model, DirectoryEntryGroupKind::Bundles).entries[0];
        assert_eq!(bundle.name, "article");
        assert_eq!(bundle.extent, "2 variants");

        let markdown = &group(&model, DirectoryEntryGroupKind::Markdown).entries[0];
        assert_eq!(markdown.name, "README.md");
        assert_eq!(markdown.extent, "2,140 words · 9 min");
        assert_eq!(markdown.secondary_meta.as_deref(), Some("2026-05-20"));

        let document = &group(&model, DirectoryEntryGroupKind::Documents).entries[0];
        assert_eq!(document.extent, "31 pages");
        assert_eq!(document.secondary_meta.as_deref(), Some("3.4M"));

        let image = &group(&model, DirectoryEntryGroupKind::Images).entries[0];
        assert_eq!(image.extent, "1920×1080");

        let data = &group(&model, DirectoryEntryGroupKind::Data).entries[0];
        assert_eq!(data.extent, "161B");
    }

    #[wasm_bindgen_test]
    fn directory_model_uses_modified_at_as_date_fallback() {
        let mut directory = meta(NodeKind::Directory, Some("docs"));
        directory.derived.modified_at = Some(1_704_153_600);

        let mut note = meta(NodeKind::Page, Some("Note"));
        note.derived.word_count = Some(79);
        note.derived.modified_at = Some(1_704_240_000);

        let mut dated = meta(NodeKind::Page, Some("Dated"));
        dated.authored.date = Some("2026-05-20".to_string());
        dated.derived.word_count = Some(10);
        dated.derived.modified_at = Some(1_704_326_400);

        let snapshot = ScannedSubtree {
            files: vec![file("docs/note.md", note), file("docs/dated.md", dated)],
            directories: vec![dir("docs", directory)],
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        let model = build_directory_model(&fs, &VirtualPath::from_absolute("/docs").unwrap());

        assert_eq!(model.date.as_deref(), Some("2024-01-02"));
        let markdown = &group(&model, DirectoryEntryGroupKind::Markdown).entries;
        let note = markdown
            .iter()
            .find(|entry| entry.name == "note.md")
            .expect("note entry");
        assert_eq!(note.secondary_meta.as_deref(), Some("2024-01-03"));
        let dated = markdown
            .iter()
            .find(|entry| entry.name == "dated.md")
            .expect("dated entry");
        assert_eq!(dated.secondary_meta.as_deref(), Some("2026-05-20"));
    }
}
