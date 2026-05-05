use std::fs;
use std::path::{Component, Path, PathBuf};

use websh_core::attestation::artifact::{ContentFile, sha256_hex};
use websh_core::domain::NodeKind;
use websh_core::filesystem::content_route_for_path;

use crate::CliResult;

pub(crate) const CONTENT_MANIFEST_FILE: &str = "manifest.json";

pub(crate) fn collect_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> CliResult {
    if !dir.exists() {
        return Ok(());
    }

    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            collect_files_recursive(&path, out)?;
        } else if file_type.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

pub(crate) fn should_skip_content_file(rel_path: &str) -> bool {
    rel_path == CONTENT_MANIFEST_FILE
        || rel_path.ends_with(".meta.json")
        || rel_path.ends_with("_index.dir.json")
        || rel_path
            .split('/')
            .any(|part| matches!(part, ".DS_Store" | ".gitkeep"))
}

pub(crate) fn should_skip_primary_content_file(rel_path: &str) -> bool {
    should_skip_content_file(rel_path) || rel_path.split('/').any(|part| part == ".websh")
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

pub(crate) fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

pub(crate) fn relative_path_from(root: &Path, path: &Path) -> CliResult<String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|err| format!("path not under root: {err}"))?;
    Ok(rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

pub(crate) fn build_content_files(root: &Path, paths: &[PathBuf]) -> CliResult<Vec<ContentFile>> {
    let mut files = paths
        .iter()
        .map(|path| {
            let artifact_path = artifact_path(root, path)?;
            let bytes = fs::read(resolve_path(root, path))?;
            Ok(ContentFile {
                path: artifact_path,
                sha256: sha256_hex(&bytes),
                bytes: bytes.len() as u64,
            })
        })
        .collect::<CliResult<Vec<_>>>()?;

    files.sort_by(|left, right| left.path.cmp(&right.path));
    if files.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err("duplicate content path".into());
    }

    Ok(files)
}

pub(crate) fn artifact_path(root: &Path, path: &Path) -> CliResult<String> {
    let relative = if path.is_absolute() {
        path.strip_prefix(root)
            .map_err(|_| format!("path {} is outside root {}", path.display(), root.display()))?
            .to_path_buf()
    } else {
        path.to_path_buf()
    };

    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!("path {} escapes the project root", path.display()).into());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("unsupported path {}", path.display()).into());
            }
        }
    }

    if parts.is_empty() {
        return Err("empty content path".into());
    }
    Ok(parts.join("/"))
}
