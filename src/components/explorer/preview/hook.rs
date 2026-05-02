//! Shared preview logic hook.
//!
//! Extracts common signal derivations and data fetching logic used by both
//! PreviewPanel (desktop) and BottomSheet (mobile).

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::shared::{FileMeta, file_meta_for_path};
use crate::models::{FileType, NodeMetadata, Selection};
use crate::utils::{RenderedMarkdown, data_url_for_bytes, media_type_for_path, render_markdown};

/// Fetched content for preview.
#[derive(Clone)]
pub enum PreviewContent {
    /// Rendered HTML from markdown
    Html(RenderedMarkdown),
    /// Raw text content
    Text(String),
    /// Binary asset rendered from the engine read surface
    AssetUrl(String),
    /// Error occurred while fetching
    Error(String),
}

/// Directory metadata for preview display (includes runtime counts).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DirMeta {
    /// Title from .meta.json
    pub title: String,
    /// Description text
    pub description: Option<String>,
    /// Icon identifier
    #[allow(dead_code)]
    pub icon: Option<String>,
    /// Thumbnail image path
    pub thumbnail: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Item counts: (file_count, dir_count)
    pub counts: Option<(usize, usize)>,
}

impl From<&NodeMetadata> for DirMeta {
    fn from(meta: &NodeMetadata) -> Self {
        Self {
            title: meta.title().unwrap_or("").to_string(),
            description: meta.description().map(str::to_string),
            icon: meta.icon().map(str::to_string),
            thumbnail: meta.thumbnail().map(str::to_string),
            tags: meta.tags_owned(),
            counts: None,
        }
    }
}

/// All derived data needed for preview rendering.
///
/// This struct bundles all the signals that both PreviewPanel and BottomSheet need,
/// eliminating code duplication.
#[derive(Clone, Copy)]
pub struct PreviewData {
    /// The name of the selected item (extracted from path)
    pub item_name: Signal<String>,
    /// Whether the selection is a directory
    pub is_dir: Signal<bool>,
    /// Whether the file is access-restricted
    pub is_restricted: Signal<bool>,
    /// The file type (Markdown, Image, etc.)
    pub file_type: Signal<FileType>,
    /// Directory metadata from manifest (includes item counts)
    pub dir_meta: Signal<Option<DirMeta>>,
    /// File metadata used by file-specific previews.
    pub file_meta: Signal<Option<FileMeta>>,
    /// URL for image preview
    pub image_url: Signal<Option<String>>,
    /// Async content resource for text/markdown preview
    pub content: LocalResource<Option<PreviewContent>>,
    /// Selection signal (for open button and clearing)
    pub selection: RwSignal<Option<Selection>>,
}

impl PreviewData {
    /// Clear the current selection (close preview).
    pub fn close(&self) {
        self.selection.set(None);
    }
}

/// Hook that provides all preview-related derived signals.
///
/// Call this once in your preview component to get all the data you need.
pub fn use_preview() -> PreviewData {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let selection = ctx.explorer.selection;

    // Extract name from selection path
    let item_name = Signal::derive(move || {
        selection
            .get()
            .and_then(|s| s.path.file_name().map(String::from))
            .unwrap_or_default()
    });

    // Check if selection is a directory
    let is_dir = Signal::derive(move || selection.get().map(|s| s.is_dir).unwrap_or(false));

    let is_restricted = Signal::derive(move || {
        selection
            .get()
            .filter(|s| !s.is_dir)
            .map(|s| {
                ctx.view_global_fs.with(|fs| {
                    fs.get_entry(&s.path)
                        .map(|entry| entry.is_restricted())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    // Get content path for fetching (files only)
    let content_path =
        Signal::derive(move || selection.get().filter(|s| !s.is_dir).map(|s| s.path));

    // Detect file type
    let file_type = Signal::derive(move || {
        content_path
            .get()
            .map(|p| FileType::from_path(p.as_str()))
            .unwrap_or(FileType::Unknown)
    });

    // Memoized so dependent views skip re-rendering when the selection
    // changes but the projected metadata is byte-identical (common when
    // re-selecting the same file after a sync).
    let file_meta = Memo::new(move |_| {
        selection
            .get()
            .filter(|s| !s.is_dir)
            .and_then(|s| file_meta_for_path(ctx, &s.path))
    });

    // Memoized: the wrapped fs walk produces an `Eq` projection, so the
    // expensive list_dir + count loop only fires when the result changes.
    let dir_meta = Memo::new(move |_| {
        selection.get().filter(|s| s.is_dir).map(|s| {
            ctx.view_global_fs.with(|fs| {
                let mut meta = fs
                    .get_entry(&s.path)
                    .filter(|e| e.is_directory())
                    .map(|e| DirMeta::from(e.meta()))
                    .unwrap_or_else(|| DirMeta {
                        title: s.path.file_name().unwrap_or("").to_string(),
                        ..Default::default()
                    });

                // Add item counts
                meta.counts = fs.list_dir(&s.path).map(|entries| {
                    let files = entries.iter().filter(|e| !e.is_dir).count();
                    let dirs = entries.iter().filter(|e| e.is_dir).count();
                    (files, dirs)
                });

                meta
            })
        })
    });

    // Fetch content for preview (files only)
    let content = LocalResource::new(move || {
        let path = content_path.get();
        let ftype = file_type.get();
        let encrypted = is_restricted.get();

        async move {
            if encrypted {
                return None;
            }
            let path = path?;

            match ftype {
                FileType::Markdown => match ctx.read_text(&path).await {
                    Ok(content) => {
                        let rendered = render_markdown(&content);
                        Some(PreviewContent::Html(rendered))
                    }
                    Err(e) => Some(PreviewContent::Error(e.to_string())),
                },
                FileType::Unknown => match ctx.read_text(&path).await {
                    Ok(content) => Some(PreviewContent::Text(content)),
                    Err(e) => Some(PreviewContent::Error(e.to_string())),
                },
                FileType::Image => match ctx.read_bytes(&path).await {
                    Ok(bytes) => Some(PreviewContent::AssetUrl(data_url_for_bytes(
                        &bytes,
                        media_type_for_path(path.as_str()),
                    ))),
                    Err(e) => Some(PreviewContent::Error(e.to_string())),
                },
                _ => None,
            }
        }
    });

    let image_url = Signal::derive(move || match content.get() {
        Some(Some(PreviewContent::AssetUrl(url))) => Some(url),
        _ => None,
    });

    PreviewData {
        item_name,
        is_dir,
        is_restricted,
        file_type,
        dir_meta: dir_meta.into(),
        file_meta: file_meta.into(),
        image_url,
        content,
        selection,
    }
}
