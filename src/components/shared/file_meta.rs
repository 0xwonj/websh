//! Manifest-driven file metadata projection.
//!
//! `FileMeta` lives here, in `shared`, because both the explorer preview
//! surface and the reader page consume it. The struct mirrors the subset of
//! `FsEntry::File` fields that surface UIs care about.
//!
//! Note: the similarly named `FileMetaStrip` (in `shared/file_meta_strip`)
//! is a render component, not a data type.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::models::{FsEntry, VirtualPath};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileMeta {
    pub description: String,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub date: Option<String>,
    pub tags: Vec<String>,
}

impl FileMeta {
    pub fn has_display_meta(&self) -> bool {
        self.date
            .as_ref()
            .is_some_and(|date| !date.trim().is_empty())
            || self.tags.iter().any(|tag| !tag.trim().is_empty())
    }

    pub fn clean_date(&self) -> Option<String> {
        self.date
            .as_ref()
            .map(|date| date.trim().to_string())
            .filter(|date| !date.is_empty())
    }

    pub fn clean_tags(&self) -> Vec<String> {
        self.tags
            .iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect()
    }
}

/// Project the `FsEntry` at `path` into a `FileMeta`. Returns `None` for
/// directories, missing entries, or non-`File` variants.
pub fn file_meta_for_path(ctx: AppContext, path: &VirtualPath) -> Option<FileMeta> {
    ctx.view_global_fs.with(|fs| {
        fs.get_entry(path).and_then(|entry| match entry {
            FsEntry::File {
                meta, description, ..
            } => Some(FileMeta {
                description: description.clone(),
                size: meta.size,
                modified: meta.modified,
                date: meta.date.clone(),
                tags: meta.tags.clone(),
            }),
            _ => None,
        })
    })
}
