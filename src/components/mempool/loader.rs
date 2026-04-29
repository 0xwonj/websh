//! Async fetcher for /mempool entries.

use leptos::prelude::With;

use crate::app::AppContext;
use crate::core::engine::GlobalFs;
use crate::models::{FsEntry, VirtualPath};

use super::model::LoadedMempoolFile;
use super::parse::parse_mempool_frontmatter;

const MEMPOOL_ROOT: &str = "/mempool";

pub fn mempool_root() -> VirtualPath {
    VirtualPath::from_absolute(MEMPOOL_ROOT).expect("mempool root is absolute")
}

/// Walk `/mempool`, fetch each file's body, and build the `LoadedMempoolFile`
/// list. Returns an empty vec if the mount is missing or the tree is empty.
/// Individual file fetch failures are logged and the file is skipped.
pub async fn load_mempool_files(ctx: AppContext) -> Vec<LoadedMempoolFile> {
    let root = mempool_root();
    let paths = ctx
        .view_global_fs
        .with(|fs| collect_mempool_files(fs, &root));

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        match ctx.read_text(&path).await {
            Ok(body) => {
                let Some(meta) = parse_mempool_frontmatter(&body) else {
                    leptos::logging::warn!(
                        "mempool: skipping {} — no recognizable frontmatter",
                        path.as_str()
                    );
                    continue;
                };
                let byte_len = body.as_bytes().len();
                let is_markdown = path.as_str().ends_with(".md");
                out.push(LoadedMempoolFile {
                    path,
                    meta,
                    body,
                    byte_len,
                    is_markdown,
                });
            }
            Err(error) => {
                leptos::logging::warn!("mempool: failed to read {}: {error}", path.as_str());
            }
        }
    }
    out
}

fn collect_mempool_files(fs: &GlobalFs, root: &VirtualPath) -> Vec<VirtualPath> {
    let mut out = Vec::new();
    walk(fs, root, &mut out);
    out
}

fn walk(fs: &GlobalFs, current: &VirtualPath, out: &mut Vec<VirtualPath>) {
    let Some(entry) = fs.get_entry(current) else {
        return;
    };
    match entry {
        FsEntry::Directory { children, .. } => {
            for (name, _child) in children.iter() {
                let child_path = current.join(name);
                walk(fs, &child_path, out);
            }
        }
        FsEntry::File { .. } => {
            if current.as_str().ends_with(".md") {
                out.push(current.clone());
            }
        }
    }
}
