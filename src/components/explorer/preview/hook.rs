//! Shared preview logic hook.
//!
//! Extracts common signal derivations and data fetching logic used by both
//! PreviewPanel (desktop) and BottomSheet (mobile).

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::terminal::RouteContext;
use crate::models::{DirectoryMetadata, FileType, FsEntry, Selection};
use crate::utils::{fetch_content, markdown_to_html};

/// File metadata tuple: (description, size, modified timestamp)
pub type FileMeta = (String, Option<u64>, Option<u64>);

/// Fetched content for preview.
#[derive(Clone)]
pub enum PreviewContent {
    /// Rendered HTML from markdown
    Html(String),
    /// Raw text content
    Text(String),
    /// Error occurred while fetching
    Error(String),
}

/// Directory metadata for preview display (includes runtime counts).
#[derive(Clone, Default)]
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

impl From<&DirectoryMetadata> for DirMeta {
    fn from(meta: &DirectoryMetadata) -> Self {
        Self {
            title: meta.title.clone(),
            description: meta.description.clone(),
            icon: meta.icon.clone(),
            thumbnail: meta.thumbnail.clone(),
            tags: meta.tags.clone(),
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
    /// Whether the file is encrypted
    pub is_encrypted: Signal<bool>,
    /// The file type (Markdown, Image, etc.)
    pub file_type: Signal<FileType>,
    /// Directory metadata from manifest (includes item counts)
    pub dir_meta: Signal<Option<DirMeta>>,
    /// File metadata: (description, size, modified)
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
    let route_ctx = use_context::<RouteContext>().expect("RouteContext must be provided");

    let selection = ctx.explorer.selection;

    // Extract name from selection path
    let item_name = Signal::derive(move || {
        selection
            .get()
            .and_then(|s| s.path.rsplit('/').next().map(String::from))
            .unwrap_or_default()
    });

    // Check if selection is a directory
    let is_dir = Signal::derive(move || selection.get().map(|s| s.is_dir).unwrap_or(false));

    // Check if file is encrypted
    let is_encrypted = Signal::derive(move || {
        selection
            .get()
            .filter(|s| !s.is_dir)
            .map(|s| {
                ctx.fs.with(|fs| {
                    fs.get_entry(&s.path)
                        .map(|entry| entry.is_encrypted())
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });

    // Get content path for fetching (files only)
    let content_path = Signal::derive(move || {
        selection
            .get()
            .filter(|s| !s.is_dir)
            .and_then(|s| ctx.fs.with(|fs| fs.get_file_content_path(&s.path)))
    });

    // Detect file type
    let file_type = Signal::derive(move || {
        content_path
            .get()
            .map(|p| FileType::from_path(&p))
            .unwrap_or(FileType::Unknown)
    });

    // Get file metadata
    let file_meta = Signal::derive(move || {
        selection.get().filter(|s| !s.is_dir).and_then(|s| {
            ctx.fs.with(|fs| {
                fs.get_entry(&s.path).and_then(|entry| match entry {
                    FsEntry::File {
                        meta, description, ..
                    } => Some((description.clone(), meta.size, meta.modified)),
                    _ => None,
                })
            })
        })
    });

    // Get directory metadata from FsEntry
    let dir_meta = Signal::derive(move || {
        selection.get().filter(|s| s.is_dir).map(|s| {
            ctx.fs.with(|fs| {
                let mut meta = fs
                    .get_entry(&s.path)
                    .and_then(|e| e.dir_meta())
                    .map(DirMeta::from)
                    .unwrap_or_else(|| DirMeta {
                        title: s.path.rsplit('/').next().unwrap_or("").to_string(),
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

    // Helper to get content base URL from route
    let base_url = Signal::derive(move || {
        let route = route_ctx.0.get();
        route
            .mount()
            .map(|m| m.content_base_url())
            .unwrap_or_else(crate::config::default_base_url)
    });

    // Build image URL for thumbnails
    let image_url = Signal::derive(move || {
        content_path
            .get()
            .map(|p| format!("{}/{}", base_url.get(), p))
    });

    // Fetch content for preview (files only)
    let content = LocalResource::new(move || {
        let path = content_path.get();
        let ftype = file_type.get();
        let encrypted = is_encrypted.get();
        let url_base = base_url.get();

        async move {
            if encrypted {
                return None;
            }
            let path = path?;
            let url = format!("{}/{}", url_base, path);

            match ftype {
                FileType::Markdown => match fetch_content(&url).await {
                    Ok(content) => {
                        let html = markdown_to_html(&content);
                        Some(PreviewContent::Html(html))
                    }
                    Err(e) => Some(PreviewContent::Error(e.to_string())),
                },
                FileType::Unknown => match fetch_content(&url).await {
                    Ok(content) => Some(PreviewContent::Text(content)),
                    Err(e) => Some(PreviewContent::Error(e.to_string())),
                },
                _ => None,
            }
        }
    });

    PreviewData {
        item_name,
        is_dir,
        is_encrypted,
        file_type,
        dir_meta,
        file_meta,
        image_url,
        content,
        selection,
    }
}
