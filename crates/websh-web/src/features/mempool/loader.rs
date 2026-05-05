//! Manifest-driven projection of `/mempool` entries — no body fetch.

use leptos::prelude::With;

use super::model::LoadedMempoolFile;
use crate::app::AppContext;
use websh_core::domain::{FsEntry, VirtualPath};
use websh_core::filesystem::GlobalFs;
use websh_core::mempool::mempool_root;

/// Manifest entries missing the `mempool` block are skipped (no
/// body-fetch fallback) — re-commit them via compose to repopulate.
pub fn load_mempool_files(ctx: AppContext) -> Vec<LoadedMempoolFile> {
    let root = mempool_root();
    ctx.view_global_fs.with(|fs| collect_loaded(fs, root))
}

fn collect_loaded(fs: &GlobalFs, root: &VirtualPath) -> Vec<LoadedMempoolFile> {
    let mut out = Vec::new();
    walk(fs, root, &mut out);
    out
}

fn walk(fs: &GlobalFs, current: &VirtualPath, out: &mut Vec<LoadedMempoolFile>) {
    let Some(entry) = fs.get_entry(current) else {
        return;
    };
    match entry {
        FsEntry::Directory { children, .. } => {
            for (name, _child) in children.iter() {
                walk(fs, &current.join(name), out);
            }
        }
        FsEntry::File {
            meta, extensions, ..
        } => {
            if !current.as_str().ends_with(".md") {
                return;
            }
            let Some(mempool) = extensions.mempool.clone() else {
                leptos::logging::warn!(
                    "mempool: skipping {} — manifest entry is missing the `mempool` block",
                    current.as_str()
                );
                return;
            };
            out.push(LoadedMempoolFile {
                path: current.clone(),
                meta: meta.clone(),
                mempool,
            });
        }
    }
}
