//! `ReaderMeta` — combined intent + manifest projection consumed by views.
//!
//! `reader_meta` is the public entry; `build_reader_meta` is the pure
//! inner combinator unit-tested below.

use crate::app::AppContext;
use crate::components::shared::{FileMeta, file_meta_for_path, size_summary_parts};
use crate::models::{ImageDim, NodeKind, PageSize, VirtualPath};
use crate::utils::format::{format_date_iso, format_size};

use super::intent::ReaderIntent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderMeta {
    pub title: String,
    pub canonical_path: VirtualPath,
    pub modified_iso: Option<String>,
    pub date: Option<String>,
    pub size_pretty: Option<String>,
    pub tags: Vec<String>,
    pub description: String,
    pub media_type_hint: Option<&'static str>,
    /// Effective kind, used by the title strip to render a friendly label
    /// (e.g. `Page` → "Note") and by view dispatch to pick the right
    /// metric for the right-hand side of the strip.
    pub kind: NodeKind,
    /// PDF MediaBox geometry (points). Drives iframe `aspect-ratio` in
    /// [`super::views::pdf::PdfReaderView`] and the `· N pages` chip.
    pub page_size: Option<PageSize>,
    pub page_count: Option<u32>,
    /// Pixel dimensions for raster images. Drives `<img width/height>` in
    /// [`super::views::asset::AssetReaderView`] (preventing layout shift)
    /// and the `· W×H` chip in the title strip.
    pub image_dimensions: Option<ImageDim>,
    /// Markdown word count (frontmatter excluded). Drives the
    /// `N words · M min` chip on the right side of the title strip.
    pub word_count: Option<u32>,
}

impl ReaderMeta {
    /// Display value for the single `Date` row — author-declared `date`
    /// preferred, mechanical `modified_iso` as fallback, `None` if neither.
    pub fn display_date(&self) -> Option<String> {
        self.date.clone().or_else(|| self.modified_iso.clone())
    }

    /// Kind-aware size chunks, sharing logic with
    /// [`FileMeta::size_summary_parts`] so the same file produces the
    /// same chunks in the title strip and the ledger entry meta line.
    pub fn size_summary_parts(&self) -> Vec<String> {
        size_summary_parts(
            self.kind,
            self.word_count,
            self.page_count,
            self.image_dimensions.as_ref(),
        )
    }
}

pub fn reader_meta(ctx: AppContext, intent: &ReaderIntent) -> ReaderMeta {
    let node_path = node_path_for(intent);
    let file_meta = file_meta_for_path(ctx, &node_path).unwrap_or_default();
    build_reader_meta(intent, &node_path, file_meta)
}

fn node_path_for(intent: &ReaderIntent) -> VirtualPath {
    match intent {
        ReaderIntent::Markdown { node_path }
        | ReaderIntent::Html { node_path }
        | ReaderIntent::Plain { node_path }
        | ReaderIntent::Redirect { node_path }
        | ReaderIntent::Asset { node_path, .. } => node_path.clone(),
    }
}

fn build_reader_meta(intent: &ReaderIntent, node_path: &VirtualPath, meta: FileMeta) -> ReaderMeta {
    let title = node_path
        .file_name()
        .map(|name| {
            name.rsplit_once('.')
                .map(|(stem, _ext)| stem.to_string())
                .unwrap_or_else(|| name.to_string())
        })
        .unwrap_or_else(|| node_path.as_str().trim_matches('/').to_string());

    let modified_iso = meta.modified.map(format_date_iso);
    let date = meta.clean_date();
    let size_pretty = meta.size.map(|size| format_size(Some(size), false));
    let tags = meta.clean_tags();
    let description = meta.description.as_deref().unwrap_or("").trim().to_string();
    let media_type_hint = media_type_hint_for(intent);

    ReaderMeta {
        title,
        canonical_path: node_path.clone(),
        modified_iso,
        date,
        size_pretty,
        tags,
        description,
        media_type_hint,
        kind: meta.kind,
        page_size: meta.page_size,
        page_count: meta.page_count,
        image_dimensions: meta.image_dimensions,
        word_count: meta.word_count,
    }
}

fn media_type_hint_for(intent: &ReaderIntent) -> Option<&'static str> {
    match intent {
        ReaderIntent::Markdown { .. } => Some("UTF-8 · CommonMark"),
        ReaderIntent::Html { .. } => Some("UTF-8 · sanitized"),
        ReaderIntent::Plain { .. } => Some("UTF-8 · LF"),
        ReaderIntent::Asset { .. } | ReaderIntent::Redirect { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp(path: &str) -> VirtualPath {
        VirtualPath::from_absolute(path).expect("test path")
    }

    fn populated_meta() -> FileMeta {
        FileMeta {
            title: "Sample".to_string(),
            description: Some("An abstract.".to_string()),
            size: Some(1024),
            modified: Some(1_704_067_200),
            date: Some("2026-04-22".to_string()),
            tags: vec!["paper".to_string(), "draft".to_string()],
            ..FileMeta::default()
        }
    }

    #[test]
    fn markdown_intent_with_full_meta() {
        let intent = ReaderIntent::Markdown {
            node_path: vp("/blog/hello.md"),
        };
        let meta = build_reader_meta(&intent, &vp("/blog/hello.md"), populated_meta());
        assert_eq!(meta.title, "hello");
        assert_eq!(meta.media_type_hint, Some("UTF-8 · CommonMark"));
        assert_eq!(meta.date.as_deref(), Some("2026-04-22"));
        assert!(meta.modified_iso.is_some());
        assert_eq!(meta.tags, vec!["paper", "draft"]);
    }

    #[test]
    fn plain_intent_with_size_only() {
        let intent = ReaderIntent::Plain {
            node_path: vp("/notes/x.txt"),
        };
        let meta = FileMeta {
            size: Some(2048),
            ..FileMeta::default()
        };
        let result = build_reader_meta(&intent, &vp("/notes/x.txt"), meta);
        assert_eq!(result.title, "x");
        assert_eq!(result.media_type_hint, Some("UTF-8 · LF"));
        assert!(result.size_pretty.is_some());
        assert!(result.date.is_none());
        assert!(result.modified_iso.is_none());
        assert!(result.tags.is_empty());
        assert_eq!(result.description, "");
    }

    #[test]
    fn pdf_intent_preserves_description() {
        let intent = ReaderIntent::Asset {
            node_path: vp("/papers/x.pdf"),
            media_type: "application/pdf".to_string(),
        };
        let meta = FileMeta {
            description: Some("  We present a thing.  ".to_string()),
            ..FileMeta::default()
        };
        let result = build_reader_meta(&intent, &vp("/papers/x.pdf"), meta);
        assert_eq!(result.title, "x");
        assert_eq!(result.media_type_hint, None);
        assert_eq!(result.description, "We present a thing.");
    }

    #[test]
    fn image_intent_with_empty_meta_has_no_description() {
        let intent = ReaderIntent::Asset {
            node_path: vp("/cover.png"),
            media_type: "image/png".to_string(),
        };
        let result = build_reader_meta(&intent, &vp("/cover.png"), FileMeta::default());
        assert_eq!(result.title, "cover");
        assert!(result.description.is_empty());
        assert!(result.size_pretty.is_none());
    }

    #[test]
    fn redirect_intent_constructs() {
        let intent = ReaderIntent::Redirect {
            node_path: vp("/x.link"),
        };
        let result = build_reader_meta(&intent, &vp("/x.link"), FileMeta::default());
        assert_eq!(result.title, "x");
        assert_eq!(result.media_type_hint, None);
    }

    fn reader_meta_with(date: Option<&str>, modified_iso: Option<&str>) -> ReaderMeta {
        ReaderMeta {
            title: "x".to_string(),
            canonical_path: vp("/x"),
            modified_iso: modified_iso.map(String::from),
            date: date.map(String::from),
            size_pretty: None,
            tags: vec![],
            description: String::new(),
            media_type_hint: None,
            kind: NodeKind::Page,
            page_size: None,
            page_count: None,
            image_dimensions: None,
            word_count: None,
        }
    }

    #[test]
    fn display_date_prefers_author_declared() {
        let m = reader_meta_with(Some("2026-04-22"), Some("2026-04-30"));
        assert_eq!(m.display_date().as_deref(), Some("2026-04-22"));
    }

    #[test]
    fn display_date_falls_back_to_modified() {
        let m = reader_meta_with(None, Some("2026-04-30"));
        assert_eq!(m.display_date().as_deref(), Some("2026-04-30"));
    }

    #[test]
    fn display_date_none_when_both_absent() {
        let m = reader_meta_with(None, None);
        assert!(m.display_date().is_none());
    }

    #[test]
    fn title_strips_extension() {
        let intent = ReaderIntent::Markdown {
            node_path: vp("/blog/some.thing.md"),
        };
        let result = build_reader_meta(&intent, &vp("/blog/some.thing.md"), FileMeta::default());
        assert_eq!(result.title, "some.thing"); // rsplit only trims last extension
    }
}
