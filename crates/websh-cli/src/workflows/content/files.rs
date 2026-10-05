use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use websh_core::domain::NodeKind;
use websh_core::filesystem::content_route_for_path;

use crate::CliResult;

pub(crate) fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> CliResult {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("read directory {}", dir.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("read directory entry in {}", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type {}", path.display()))?;
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            collect_files_recursive(&path, out)?;
        } else if file_type.is_symlink() {
            bail!("content symlinks are not supported: {}", path.display());
        } else if file_type.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

pub(crate) fn route_for_content_path(rel_path: &str) -> String {
    content_route_for_path(rel_path)
}

pub(crate) fn kind_for_content_path(rel_path: &str) -> NodeKind {
    match Path::new(rel_path).extension().and_then(|ext| ext.to_str()) {
        Some("md" | "html" | "htm") => NodeKind::Page,
        Some("link") => NodeKind::Redirect,
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => NodeKind::Asset,
        Some("pdf") => NodeKind::Document,
        Some("app") => NodeKind::App,
        Some("json") => NodeKind::Data,
        _ => NodeKind::Document,
    }
}
