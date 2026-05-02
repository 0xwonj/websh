use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use websh_core::content_routes::content_route_for_path;
use websh_core::domain::manifest::{ContentManifestDocument, ContentManifestEntry};
use websh_core::domain::{Fields, ImageDim, NodeKind, NodeMetadata, PageSize, SCHEMA_VERSION};

use super::CliResult;
use super::io::write_json;

pub(crate) const DEFAULT_CONTENT_DIR: &str = "content";
pub(crate) const CONTENT_MANIFEST_FILE: &str = "manifest.json";

/// Canonical entry point: walk the content tree, refresh every node's
/// sidecar (recompute `derived` fields and merge frontmatter into
/// `authored` for markdown files), then fold the sidecars into
/// `manifest.json`.
///
/// This is the only function callers should reach for when they need
/// the manifest to reflect the current on-disk content. The CLI's
/// `content manifest` subcommand and Trunk's pre-build hook both end
/// here. Internal callers that have *just* sync'd and only need to
/// re-fold the manifest after touching `.websh/ledger.json` may use
/// [`build_manifest_from_sidecars`] for the projection-only path.
pub(crate) fn sync_content(root: &Path, content_dir: &Path) -> CliResult<ContentManifestDocument> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)?;

    let mut all_files = Vec::new();
    collect_files_recursive(&content_root, &mut all_files)?;

    // First pass: refresh every primary file's sidecar.
    for file_path in &all_files {
        let rel_path = relative_path_from(&content_root, file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }
        sync_file_sidecar(&content_root, file_path, &rel_path)?;
    }

    // Second pass: refresh directory sidecars.
    let directories = enumerate_directories_from_files(&content_root, &all_files)?;
    for dir_rel in &directories {
        sync_directory_sidecar(&content_root, dir_rel)?;
    }

    // Third pass: build manifest from current sidecars + the file list
    // we already have on hand.
    bundle_manifest(&content_root, &all_files, &directories)
}

/// Internal-only: re-fold `manifest.json` from existing sidecars without
/// refreshing them. The caller is responsible for ensuring sidecars are
/// already current — this is intended for narrow situations like
/// "rewrote `.websh/ledger.json`, now re-bundle the manifest so the new
/// ledger hash propagates" where doing a full [`sync_content`] would be
/// wasted work.
///
/// Not exposed as a CLI subcommand: external invocations should always
/// go through `content manifest` (i.e. [`sync_content`]) so the manifest
/// is never ahead of the sidecars.
pub(crate) fn build_manifest_from_sidecars(
    root: &Path,
    content_dir: &Path,
) -> CliResult<ContentManifestDocument> {
    let content_root = resolve_path(root, content_dir);
    fs::create_dir_all(&content_root)?;

    let mut all_files = Vec::new();
    collect_files_recursive(&content_root, &mut all_files)?;
    let directories = enumerate_directories_from_files(&content_root, &all_files)?;
    bundle_manifest(&content_root, &all_files, &directories)
}

/// Project current sidecars + filesystem state into a `manifest.json`
/// document. Pure projection — does not modify sidecars.
fn bundle_manifest(
    content_root: &Path,
    all_files: &[PathBuf],
    directories: &[String],
) -> CliResult<ContentManifestDocument> {
    let mut entries = Vec::new();

    // Directory entries first (canonical order).
    for dir_rel in directories {
        let metadata = read_directory_sidecar(content_root, dir_rel)?
            .unwrap_or_else(|| default_directory_metadata(dir_rel));
        entries.push(ContentManifestEntry {
            path: dir_rel.clone(),
            metadata,
            mempool: None,
        });
    }

    // File entries. The manifest includes `.websh/*.json` artifacts (e.g.
    // ledger.json, attestations.json) so signed/derived data is reachable
    // through the same surface; only sidecars/manifest themselves are
    // skipped.
    let mut file_entries = Vec::new();
    for file_path in all_files {
        let rel_path = relative_path_from(content_root, file_path)?;
        if should_skip_content_file(&rel_path) {
            continue;
        }
        let metadata = read_file_sidecar(content_root, &rel_path)?
            .unwrap_or_else(|| default_file_metadata(file_path, &rel_path));
        file_entries.push(ContentManifestEntry {
            path: rel_path,
            metadata,
            mempool: None,
        });
    }
    file_entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries.extend(file_entries);

    let manifest = ContentManifestDocument { entries };
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
    let mut parts: Vec<&str> = rel_path.split('/').collect();
    parts.pop();
    let mut out = Vec::new();
    while !parts.is_empty() {
        out.push(parts.join("/"));
        parts.pop();
    }
    out
}

/// Merge frontmatter-derived fields into the prior authored section
/// per-field: each field present in `frontmatter` wins; unmentioned
/// fields are preserved from `prior`. This protects user edits to the
/// sidecar that the markdown frontmatter doesn't speak to (e.g.
/// `access`, `route`, `trust`).
fn merge_authored(prior: Fields, frontmatter: Fields) -> Fields {
    Fields {
        title: frontmatter.title.or(prior.title),
        kind: frontmatter.kind.or(prior.kind),
        renderer: frontmatter.renderer.or(prior.renderer),
        route: frontmatter.route.or(prior.route),
        description: frontmatter.description.or(prior.description),
        date: frontmatter.date.or(prior.date),
        tags: frontmatter.tags.or(prior.tags),
        icon: frontmatter.icon.or(prior.icon),
        thumbnail: frontmatter.thumbnail.or(prior.thumbnail),
        sort: frontmatter.sort.or(prior.sort),
        trust: frontmatter.trust.or(prior.trust),
        access: frontmatter.access.or(prior.access),
        // The remaining fields are derive-only; frontmatter shouldn't
        // touch them, but we honor whatever it contains over `prior`
        // for symmetry.
        page_size: frontmatter.page_size.or(prior.page_size),
        page_count: frontmatter.page_count.or(prior.page_count),
        rotation: frontmatter.rotation.or(prior.rotation),
        image_dimensions: frontmatter.image_dimensions.or(prior.image_dimensions),
        size_bytes: frontmatter.size_bytes.or(prior.size_bytes),
        modified_at: frontmatter.modified_at.or(prior.modified_at),
        content_sha256: frontmatter.content_sha256.or(prior.content_sha256),
        word_count: frontmatter.word_count.or(prior.word_count),
        child_count: frontmatter.child_count.or(prior.child_count),
    }
}

/// Refresh the sidecar JSON for a primary file. Reads the file's
/// extension/contents to compute derived fields; for markdown files,
/// reads YAML frontmatter and stores it in the sidecar's `authored`
/// section. The previously authored fields are preserved when no
/// frontmatter exists (and for non-markdown files in general).
fn sync_file_sidecar(content_root: &Path, file_path: &Path, rel_path: &str) -> CliResult {
    let metadata = fs::metadata(file_path)?;
    let bytes = fs::read(file_path)?;

    let kind = kind_for_content_path(rel_path);
    let mut derived = derived_for_path(file_path, rel_path, &bytes)?;
    derived.title = Some(fallback_file_title(rel_path));
    derived.kind = Some(kind);
    derived.renderer = derived
        .renderer
        .or_else(|| default_renderer_for_kind(kind, rel_path));
    derived.size_bytes = Some(metadata.len());
    // `modified_at` is deliberately omitted for files: filesystem mtime
    // is the checkout wall-clock under git, so it diverges across clones
    // and breaks byte-stability of the sidecar (which feeds into signed
    // attestations). `content_sha256` is the canonical change-detection
    // signal.
    derived.content_sha256 = Some(format!("0x{}", hex::encode(Sha256::digest(&bytes))));

    let sidecar_path = sidecar_path_for(content_root, rel_path);
    let existing = read_sidecar_metadata(&sidecar_path)?;
    let prior_authored = existing
        .as_ref()
        .map(|m| m.authored.clone())
        .unwrap_or_default();

    // For markdown files, frontmatter is the authoring source — but it
    // wins per-field, not whole-cloth. Sidecar-only fields (e.g. `access`,
    // `route`, `trust`) that the frontmatter doesn't mention are
    // preserved.
    let authored = if rel_path.ends_with(".md") {
        match parse_yaml_frontmatter(std::str::from_utf8(&bytes).unwrap_or_default())? {
            Some(frontmatter) => merge_authored(prior_authored, frontmatter),
            None => prior_authored,
        }
    } else {
        prior_authored
    };

    let new_meta = NodeMetadata {
        schema: SCHEMA_VERSION,
        kind,
        authored,
        derived,
    };
    write_json(&sidecar_path, &new_meta)
}

fn sync_directory_sidecar(content_root: &Path, dir_rel: &str) -> CliResult {
    let sidecar_path = directory_sidecar_path_for(content_root, dir_rel);
    let existing = read_sidecar_metadata(&sidecar_path)?;
    let dir_path = if dir_rel.is_empty() {
        content_root.to_path_buf()
    } else {
        content_root.join(dir_rel)
    };

    // Directory mtime is deliberately omitted. Writing sidecars during a
    // sync bumps the directory's mtime, so storing it would make sidecars
    // non-byte-stable across consecutive sync runs (and would invalidate
    // attestations that signed the previous canonical content). The
    // `child_count` field is the cheap "did membership change" indicator.
    let derived = Fields {
        title: Some(dir_title_fallback(dir_rel)),
        kind: Some(NodeKind::Directory),
        child_count: Some(count_children(&dir_path)?),
        ..Fields::default()
    };

    let authored = existing
        .as_ref()
        .map(|m| m.authored.clone())
        .unwrap_or_default();

    let new_meta = NodeMetadata {
        schema: SCHEMA_VERSION,
        kind: NodeKind::Directory,
        authored,
        derived,
    };
    write_json(&sidecar_path, &new_meta)
}

/// Compute file-type-specific derived fields (page_size for PDFs,
/// dimensions for images, word_count for markdown). Filesystem-level
/// fields (`size_bytes`, `modified_at`, `content_sha256`) are populated
/// by the caller.
fn derived_for_path(file_path: &Path, rel_path: &str, bytes: &[u8]) -> CliResult<Fields> {
    let mut fields = Fields::default();
    let extension = Path::new(rel_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase());

    match extension.as_deref() {
        Some("pdf") => match read_pdf_dimensions(file_path) {
            Ok((page_size, page_count, rotation)) => {
                fields.page_size = Some(page_size);
                fields.page_count = Some(page_count);
                fields.rotation = Some(rotation);
            }
            Err(error) => {
                eprintln!("warn: pdf {}: {error}", rel_path);
            }
        },
        Some("png" | "jpg" | "jpeg" | "gif" | "webp") => match imagesize::blob_size(bytes) {
            Ok(dim) => {
                fields.image_dimensions = Some(ImageDim {
                    width: u32::try_from(dim.width).unwrap_or(u32::MAX),
                    height: u32::try_from(dim.height).unwrap_or(u32::MAX),
                });
            }
            Err(error) => {
                eprintln!("warn: image {}: {error}", rel_path);
            }
        },
        Some("md") => match std::str::from_utf8(bytes) {
            Ok(text) => {
                let body = strip_yaml_frontmatter(text);
                let count = body.split_whitespace().count();
                fields.word_count = Some(u32::try_from(count).unwrap_or(u32::MAX));
            }
            Err(error) => {
                eprintln!("warn: markdown {}: {error}", rel_path);
            }
        },
        _ => {}
    }
    Ok(fields)
}

fn read_pdf_dimensions(path: &Path) -> Result<(PageSize, u32, u32), String> {
    let doc = lopdf::Document::load(path).map_err(|e| format!("load: {e}"))?;
    let pages = doc.get_pages();
    let page_count = u32::try_from(pages.len()).unwrap_or(u32::MAX);
    let (_, page_id) = pages.iter().next().ok_or_else(|| "no pages".to_string())?;
    let page = doc
        .get_object(*page_id)
        .map_err(|e| format!("page object: {e}"))?
        .as_dict()
        .map_err(|e| format!("page dict: {e}"))?;
    let media_box = page
        .get(b"MediaBox")
        .map_err(|e| format!("MediaBox: {e}"))?
        .as_array()
        .map_err(|e| format!("MediaBox array: {e}"))?;
    if media_box.len() < 4 {
        return Err("MediaBox has < 4 entries".to_string());
    }
    let nums: Vec<f64> = media_box
        .iter()
        .map(|obj| {
            obj.as_float()
                .map(|f| f as f64)
                .or_else(|_| obj.as_i64().map(|i| i as f64))
                .unwrap_or(0.0)
        })
        .collect();
    let width = (nums[2] - nums[0]).abs();
    let height = (nums[3] - nums[1]).abs();
    let rotation = page
        .get(b"Rotate")
        .ok()
        .and_then(|obj| obj.as_i64().ok())
        .map(|r| r.rem_euclid(360))
        .unwrap_or(0) as u32;
    let (final_w, final_h) = if rotation % 180 == 90 {
        (height, width)
    } else {
        (width, height)
    };
    Ok((
        PageSize {
            width: final_w.round() as u32,
            height: final_h.round() as u32,
        },
        page_count,
        rotation,
    ))
}

/// Split a markdown body into `(yaml_str, body_after_fence)` if it opens
/// with a YAML frontmatter block. Recognizes both LF and CRLF line
/// endings, and anchors the closing `---` fence to the start of a line
/// so an inline `---` in the body content can't false-close the block.
fn split_yaml_frontmatter(body: &str) -> Option<(&str, &str)> {
    let after_open = body
        .strip_prefix("---\n")
        .or_else(|| body.strip_prefix("---\r\n"))?;
    // Find a closing fence at the start of a line. Accept `---` followed
    // by any line terminator or by EOF.
    let mut search_from = 0usize;
    while let Some(rel) = after_open[search_from..].find("\n---") {
        let abs = search_from + rel + 1; // index of '-' in '---'
        let end_of_yaml = abs - 1; // exclude the leading '\n'
        let after_fence = &after_open[abs + 3..];
        // The character right after '---' must be a newline (LF/CRLF) or EOF.
        let is_terminated = after_fence.is_empty()
            || after_fence.starts_with('\n')
            || after_fence.starts_with("\r\n")
            // Tolerate trailing whitespace on the fence line.
            || after_fence
                .chars()
                .next()
                .map(|c| c == ' ' || c == '\t')
                .unwrap_or(false);
        if is_terminated {
            let yaml = &after_open[..end_of_yaml];
            // Skip past one trailing line terminator after the fence.
            let body_rest = if let Some(rest) = after_fence.strip_prefix("\r\n") {
                rest
            } else if let Some(rest) = after_fence.strip_prefix('\n') {
                rest
            } else {
                // Trailing whitespace before terminator — skip until newline.
                after_fence
                    .find('\n')
                    .map(|i| &after_fence[i + 1..])
                    .unwrap_or("")
            };
            return Some((yaml, body_rest));
        }
        search_from = abs + 3;
    }
    None
}

fn parse_yaml_frontmatter(body: &str) -> CliResult<Option<Fields>> {
    let Some((yaml, _)) = split_yaml_frontmatter(body) else {
        return Ok(None);
    };
    let fields: Fields =
        serde_yaml::from_str(yaml).map_err(|err| format!("frontmatter YAML parse: {err}"))?;
    Ok(Some(fields))
}

fn strip_yaml_frontmatter(body: &str) -> &str {
    split_yaml_frontmatter(body)
        .map(|(_, rest)| rest)
        .unwrap_or(body)
}

fn read_sidecar_metadata(sidecar_path: &Path) -> CliResult<Option<NodeMetadata>> {
    if !sidecar_path.exists() {
        return Ok(None);
    }
    let body = fs::read_to_string(sidecar_path)?;
    let metadata: NodeMetadata = serde_json::from_str(&body)
        .map_err(|err| format!("parse {}: {err}", sidecar_path.display()))?;
    Ok(Some(metadata))
}

fn read_file_sidecar(content_root: &Path, rel_path: &str) -> CliResult<Option<NodeMetadata>> {
    read_sidecar_metadata(&sidecar_path_for(content_root, rel_path))
}

fn read_directory_sidecar(content_root: &Path, dir_rel: &str) -> CliResult<Option<NodeMetadata>> {
    read_sidecar_metadata(&directory_sidecar_path_for(content_root, dir_rel))
}

fn sidecar_path_for(content_root: &Path, rel_path: &str) -> PathBuf {
    let stem_path = Path::new(rel_path);
    let parent = stem_path.parent().unwrap_or_else(|| Path::new(""));
    let stem = stem_path
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    content_root.join(parent).join(format!("{stem}.meta.json"))
}

fn directory_sidecar_path_for(content_root: &Path, dir_rel: &str) -> PathBuf {
    if dir_rel.is_empty() {
        content_root.join("_index.dir.json")
    } else {
        content_root.join(dir_rel).join("_index.dir.json")
    }
}

/// Build the sorted directory list from a pre-walked file list. Caller
/// passes `all_files` so the tree isn't walked twice during sync.
fn enumerate_directories_from_files(
    content_root: &Path,
    all_files: &[PathBuf],
) -> CliResult<Vec<String>> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    seen.insert(String::new());
    for file in all_files {
        let rel = relative_path_from(content_root, file)?;
        for parent in content_parent_dirs(&rel) {
            seen.insert(parent);
        }
    }
    Ok(seen.into_iter().collect())
}

fn count_children(dir: &Path) -> CliResult<u32> {
    let mut count = 0u32;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Filter list mirrors `should_skip_content_file` so the count
        // matches the manifest entry count for this directory.
        if name == ".git" || should_skip_content_file(&name) {
            continue;
        }
        count += 1;
    }
    Ok(count)
}

fn dir_title_fallback(dir_rel: &str) -> String {
    if dir_rel.is_empty() {
        "Home".to_string()
    } else {
        Path::new(dir_rel)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| dir_rel.to_string())
    }
}

fn default_file_metadata(file_path: &Path, rel_path: &str) -> NodeMetadata {
    let kind = kind_for_content_path(rel_path);
    let size = fs::metadata(file_path).ok().map(|m| m.len());

    NodeMetadata {
        schema: SCHEMA_VERSION,
        kind,
        authored: Fields::default(),
        derived: Fields {
            title: Some(fallback_file_title(rel_path)),
            kind: Some(kind),
            renderer: default_renderer_for_kind(kind, rel_path),
            size_bytes: size,
            ..Fields::default()
        },
    }
}

fn default_directory_metadata(dir_rel: &str) -> NodeMetadata {
    NodeMetadata {
        schema: SCHEMA_VERSION,
        kind: NodeKind::Directory,
        authored: Fields::default(),
        derived: Fields {
            title: Some(dir_title_fallback(dir_rel)),
            kind: Some(NodeKind::Directory),
            ..Fields::default()
        },
    }
}

fn default_renderer_for_kind(
    kind: NodeKind,
    rel_path: &str,
) -> Option<websh_core::domain::RendererKind> {
    use websh_core::domain::RendererKind;
    let ext = Path::new(rel_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_lowercase());
    match (kind, ext.as_deref()) {
        (NodeKind::Page, Some("md")) => Some(RendererKind::MarkdownPage),
        (NodeKind::Page, Some("html" | "htm")) => Some(RendererKind::HtmlPage),
        (NodeKind::Document, Some("pdf")) => Some(RendererKind::Pdf),
        (NodeKind::Asset, Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")) => {
            Some(RendererKind::Image)
        }
        (NodeKind::Redirect, _) => Some(RendererKind::Redirect),
        (NodeKind::App, _) => Some(RendererKind::TerminalApp),
        (NodeKind::Directory, _) => Some(RendererKind::DirectoryListing),
        _ => None,
    }
}

fn fallback_file_title(rel_path: &str) -> String {
    Path::new(rel_path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| rel_path.to_string())
}

/// Resolve the `<rel_path>.meta.json` sidecar for a primary file, if it
/// exists. Returns `None` for `.meta.json` paths themselves and for
/// `_index.dir.json`.
pub(crate) fn matching_file_sidecar(content_root: &Path, rel_path: &str) -> Option<PathBuf> {
    let path = Path::new(rel_path);
    let name = path.file_name()?.to_string_lossy();
    if name.ends_with(".meta.json") || name == "_index.dir.json" {
        return None;
    }
    let sidecar = sidecar_path_for(content_root, rel_path);
    sidecar.exists().then_some(sidecar)
}

/// Resolve the human-authored content date for a file. Sidecar metadata
/// (if present) wins; markdown files without a sidecar fall back to YAML
/// frontmatter.
pub(super) fn content_entry_raw_date(
    content_root: &Path,
    path: &Path,
    rel_path: &str,
) -> Option<String> {
    if let Some(sidecar) = matching_file_sidecar(content_root, rel_path)
        && let Ok(body) = fs::read_to_string(&sidecar)
        && let Ok(metadata) = serde_json::from_str::<NodeMetadata>(&body)
        && let Some(date) = metadata.date()
        && !date.trim().is_empty()
    {
        return Some(date.to_string());
    }
    // Fallback for markdown: read frontmatter directly.
    if rel_path.ends_with(".md")
        && let Ok(body) = fs::read_to_string(path)
        && let Ok(Some(fields)) = parse_yaml_frontmatter(&body)
        && let Some(date) = fields.date.filter(|d| !d.trim().is_empty())
    {
        return Some(date);
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use websh_core::domain::{AccessFilter, Recipient};

    fn tempdir() -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut d = std::env::temp_dir();
        d.push(format!("websh-manifest-test-{}-{}", std::process::id(), id));
        if d.exists() {
            fs::remove_dir_all(&d).unwrap();
        }
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn read_sidecar(path: &Path) -> NodeMetadata {
        let body = fs::read_to_string(path).expect("sidecar exists");
        serde_json::from_str(&body).expect("sidecar parses")
    }

    #[test]
    fn populates_authored_from_frontmatter() {
        let dir = tempdir();
        fs::write(
            dir.join("hello.md"),
            "---\ntitle: Greeting\ntags:\n  - intro\n  - sample\ndate: 2026-04-22\n---\n\nbody\n",
        )
        .unwrap();

        let manifest = sync_content(&dir, Path::new(".")).expect("sync ok");

        let sidecar = read_sidecar(&dir.join("hello.meta.json"));
        assert_eq!(sidecar.authored.title.as_deref(), Some("Greeting"));
        assert_eq!(sidecar.authored.date.as_deref(), Some("2026-04-22"));
        assert_eq!(
            sidecar.authored.tags.as_deref(),
            Some(&["intro".to_string(), "sample".to_string()][..]),
        );

        let entry = manifest
            .entries
            .iter()
            .find(|e| e.path == "hello.md")
            .expect("hello.md in manifest");
        assert_eq!(entry.metadata.authored.title.as_deref(), Some("Greeting"));
        assert_eq!(entry.metadata.kind, NodeKind::Page);
    }

    #[test]
    fn idempotent_across_repeated_runs() {
        let dir = tempdir();
        fs::write(dir.join("note.md"), "---\ntitle: Note\n---\n\ncontent\n").unwrap();

        sync_content(&dir, Path::new(".")).expect("first sync");
        let bytes_a = fs::read(dir.join("manifest.json")).unwrap();
        let sidecar_a = fs::read(dir.join("note.meta.json")).unwrap();

        sync_content(&dir, Path::new(".")).expect("second sync");
        let bytes_b = fs::read(dir.join("manifest.json")).unwrap();
        let sidecar_b = fs::read(dir.join("note.meta.json")).unwrap();

        assert_eq!(bytes_a, bytes_b, "manifest must be byte-equal across syncs");
        assert_eq!(
            sidecar_a, sidecar_b,
            "sidecar must be byte-equal across syncs"
        );
    }

    #[test]
    fn preserves_sidecar_only_authored_fields() {
        let dir = tempdir();

        // Pre-existing sidecar carries an `access` recipient list — the
        // sort of field a user authors directly in the JSON, not via
        // markdown frontmatter. Sync must not clobber it.
        let prior = NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields {
                access: Some(AccessFilter {
                    recipients: vec![Recipient {
                        address: "0xabc".to_string(),
                    }],
                }),
                ..Fields::default()
            },
            derived: Fields::default(),
        };
        fs::write(
            dir.join("scoped.meta.json"),
            format!("{}\n", serde_json::to_string_pretty(&prior).unwrap()),
        )
        .unwrap();

        // Frontmatter sets `title` only — no `access` key.
        fs::write(dir.join("scoped.md"), "---\ntitle: Scoped\n---\n\nbody\n").unwrap();

        sync_content(&dir, Path::new(".")).expect("sync ok");

        let after = read_sidecar(&dir.join("scoped.meta.json"));
        assert_eq!(after.authored.title.as_deref(), Some("Scoped"));
        let access = after.authored.access.expect("access preserved");
        assert_eq!(access.recipients.len(), 1);
        assert_eq!(access.recipients[0].address, "0xabc");
    }
}
