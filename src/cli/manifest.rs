use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::FileSidecarMetadata;
use crate::models::manifest::{
    ContentManifestDirectory as GeneratedManifestDirectory,
    ContentManifestDocument as GeneratedManifest, ContentManifestFile as GeneratedManifestFile,
};
use crate::utils::content_routes::content_route_for_path;

use super::CliResult;
use super::io::write_json;

pub(crate) const DEFAULT_CONTENT_DIR: &str = "content";
pub(crate) const CONTENT_MANIFEST_FILE: &str = "manifest.json";

pub(crate) fn generate_content_manifest(
    root: &Path,
    content_dir: &Path,
) -> CliResult<GeneratedManifest> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)?;

    let mut files = Vec::new();
    collect_files_recursive(&content_root, &mut files)?;

    let mut directories = BTreeSet::from(["".to_string()]);
    let mut manifest_files = Vec::new();
    for file_path in files {
        let rel_path = relative_path_from(&content_root, &file_path)?;
        if should_skip_content_file(&rel_path) {
            continue;
        }
        for parent in content_parent_dirs(&rel_path) {
            directories.insert(parent);
        }
        manifest_files.push(manifest_file_entry(&content_root, &file_path, &rel_path)?);
    }
    manifest_files.sort_by(|left, right| left.path.cmp(&right.path));

    let mut manifest_directories = directories
        .into_iter()
        .map(|path| manifest_directory_entry(&content_root, &path))
        .collect::<CliResult<Vec<_>>>()?;
    manifest_directories.sort_by(|left, right| left.path.cmp(&right.path));

    let manifest = GeneratedManifest {
        files: manifest_files,
        directories: manifest_directories,
    };
    write_json(&content_root.join(CONTENT_MANIFEST_FILE), &manifest)?;
    Ok(manifest)
}

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

fn content_parent_dirs(rel_path: &str) -> Vec<String> {
    let parts = rel_path.split('/').collect::<Vec<_>>();
    if parts.len() <= 1 {
        return Vec::new();
    }

    let mut out = Vec::new();
    for index in 1..parts.len() {
        out.push(parts[..index].join("/"));
    }
    out
}

fn manifest_file_entry(
    content_root: &Path,
    path: &Path,
    rel_path: &str,
) -> CliResult<GeneratedManifestFile> {
    let metadata = fs::metadata(path)?;
    let fields = content_manifest_fields(content_root, path, rel_path)?;
    let title = fields
        .title
        .unwrap_or_else(|| fallback_file_title(rel_path));

    Ok(GeneratedManifestFile {
        path: rel_path.to_string(),
        title,
        size: Some(metadata.len()),
        modified: metadata.modified().ok().and_then(system_time_to_unix),
        date: fields.date,
        tags: fields.tags,
        access: None,
    })
}

fn manifest_directory_entry(
    content_root: &Path,
    rel_path: &str,
) -> CliResult<GeneratedManifestDirectory> {
    let title = if rel_path.is_empty() {
        "Home".to_string()
    } else {
        Path::new(rel_path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| rel_path.to_string())
    };
    let mut entry = GeneratedManifestDirectory {
        path: rel_path.to_string(),
        title,
        tags: Vec::new(),
        description: None,
        icon: None,
        thumbnail: None,
    };

    let sidecar = if rel_path.is_empty() {
        content_root.join("_index.dir.json")
    } else {
        content_root.join(rel_path).join("_index.dir.json")
    };
    if sidecar.exists() {
        let body = fs::read_to_string(sidecar)?;
        let value: serde_json::Value = serde_json::from_str(&body)?;
        if let Some(title) = value.get("title").and_then(|value| value.as_str()) {
            entry.title = title.to_string();
        }
        if let Some(description) = value.get("description").and_then(|value| value.as_str()) {
            entry.description = Some(description.to_string());
        }
        if let Some(icon) = value.get("icon").and_then(|value| value.as_str()) {
            entry.icon = Some(icon.to_string());
        }
        if let Some(thumbnail) = value.get("thumbnail").and_then(|value| value.as_str()) {
            entry.thumbnail = Some(thumbnail.to_string());
        }
        if let Some(tags) = value.get("tags").and_then(|value| value.as_array()) {
            entry.tags = tags
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect();
        }
    }

    Ok(entry)
}

#[derive(Default)]
struct ManifestFields {
    title: Option<String>,
    tags: Vec<String>,
    date: Option<String>,
}

/// Resolve the human-authored content date for an entry, preferring the
/// `.meta.json` sidecar over markdown frontmatter, mirroring
/// [`content_manifest_fields`].
pub(super) fn content_entry_raw_date(
    content_root: &Path,
    path: &Path,
    rel_path: &str,
) -> Option<String> {
    let mut date = markdown_manifest_fields(path, rel_path).and_then(|fields| fields.date);
    if let Some(sidecar) = matching_file_sidecar(content_root, rel_path)
        && let Ok(body) = fs::read_to_string(&sidecar)
        && let Ok(metadata) = serde_json::from_str::<FileSidecarMetadata>(&body)
        && let Some(sidecar_date) = non_empty_string(metadata.date)
    {
        date = Some(sidecar_date);
    }
    date
}

fn content_manifest_fields(
    content_root: &Path,
    path: &Path,
    rel_path: &str,
) -> CliResult<ManifestFields> {
    let mut fields = markdown_manifest_fields(path, rel_path).unwrap_or_default();

    if let Some(sidecar) = matching_file_sidecar(content_root, rel_path) {
        let body = fs::read_to_string(&sidecar)?;
        let metadata: FileSidecarMetadata = serde_json::from_str(&body)
            .map_err(|error| format!("parse {}: {error}", sidecar.display()))?;

        if let Some(title) = non_empty_string(metadata.title) {
            fields.title = Some(title);
        }
        if let Some(date) = non_empty_string(metadata.date) {
            fields.date = Some(date);
        }

        let tags = metadata
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();
        if !tags.is_empty() {
            fields.tags = tags;
        }
    }

    Ok(fields)
}

fn fallback_file_title(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

fn markdown_manifest_fields(path: &Path, rel_path: &str) -> Option<ManifestFields> {
    if !matches!(
        Path::new(rel_path).extension().and_then(|ext| ext.to_str()),
        Some("md")
    ) {
        return None;
    }
    let body = fs::read_to_string(path).ok()?;
    let frontmatter = parse_frontmatter(&body);
    let title = frontmatter
        .iter()
        .find_map(|(key, value)| (key == "title").then(|| value.to_string()))
        .or_else(|| {
            body.lines().find_map(|line| {
                line.strip_prefix("# ")
                    .map(|title| title.trim().to_string())
            })
        })
        .unwrap_or_else(|| fallback_file_title(rel_path));
    let tags = frontmatter
        .iter()
        .find_map(|(key, value)| (key == "tags").then(|| parse_inline_tags(value)))
        .unwrap_or_default();
    let date = frontmatter
        .iter()
        .find_map(|(key, value)| (key == "date").then(|| value.trim().to_string()))
        .filter(|value| !value.is_empty());

    Some(ManifestFields {
        title: Some(title),
        tags,
        date,
    })
}

fn parse_frontmatter(body: &str) -> Vec<(String, String)> {
    let mut lines = body.lines();
    if lines.next() != Some("---") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for line in lines {
        if line == "---" {
            break;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        out.push((
            key.trim().to_string(),
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        ));
    }
    out
}

fn parse_inline_tags(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
    {
        return inner
            .split(',')
            .map(|tag| tag.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|tag| !tag.is_empty())
            .collect();
    }
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    }
}

fn non_empty_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn system_time_to_unix(time: SystemTime) -> Option<u64> {
    time.duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

pub(crate) fn matching_file_sidecar(content_root: &Path, rel_path: &str) -> Option<PathBuf> {
    let path = Path::new(rel_path);
    let name = path.file_name()?.to_string_lossy();
    if name.ends_with(".meta.json") || name == "_index.dir.json" {
        return None;
    }
    let stem = path.file_stem()?.to_string_lossy();
    let sidecar_name = format!("{stem}.meta.json");
    let sidecar_rel = path
        .parent()
        .map(|parent| parent.join(&sidecar_name))
        .unwrap_or_else(|| PathBuf::from(sidecar_name));
    let sidecar = content_root.join(sidecar_rel);
    sidecar.exists().then_some(sidecar)
}

pub(crate) fn route_for_content_path(rel_path: &str) -> String {
    content_route_for_path(rel_path)
}

pub(crate) fn kind_for_content_path(rel_path: &str) -> &'static str {
    match Path::new(rel_path).extension().and_then(|ext| ext.to_str()) {
        Some("md" | "html") => "page",
        Some("link") => "redirect",
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") => "asset",
        Some("pdf") => "document",
        Some("app") => "app",
        Some("json") => "data",
        _ => "document",
    }
}

pub(crate) fn resolve_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    root.join(path)
}

pub(crate) fn relative_path_from(root: &Path, path: &Path) -> CliResult<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("path {} is outside {}", path.display(), root.display()))?;

    let mut parts = Vec::new();
    for component in relative.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_string_lossy().to_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                return Err(format!("path {} escapes {}", path.display(), root.display()).into());
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                return Err(format!("unsupported path {}", path.display()).into());
            }
        }
    }

    if parts.is_empty() {
        return Err("empty content path".into());
    }
    Ok(parts.join("/"))
}
