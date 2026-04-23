use std::collections::BTreeMap;

use crate::models::{LoadedNodeMetadata, NodeKind, RendererKind, VirtualPath};
use crate::utils::dom;

use super::global_fs::GlobalFs;
use super::intent::RenderIntent;

/// Browser request normalized into a filesystem-first input shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteRequest {
    pub url_path: String,
}

impl RouteRequest {
    pub fn new(url_path: impl Into<String>) -> Self {
        let raw = url_path.into();
        if raw.is_empty() {
            return Self {
                url_path: "/".to_string(),
            };
        }
        if raw.starts_with('/') {
            return Self {
                url_path: normalize_request_path(&raw),
            };
        }
        Self {
            url_path: normalize_request_path(&format!("/{}", raw)),
        }
    }

    pub fn current() -> Self {
        Self::new(dom::get_hash())
    }

    pub fn push(&self) {
        push_request_path(&self.url_path);
    }

    pub fn replace(&self) {
        replace_request_path(&self.url_path);
    }
}

/// Broad resolution result prior to renderer-specific details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedKind {
    Directory,
    Page,
    Document,
    App,
    Asset,
    Redirect,
}

/// Output of route resolution before content loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteResolution {
    pub request_path: String,
    pub node_path: VirtualPath,
    pub kind: ResolvedKind,
    pub params: BTreeMap<String, String>,
}

/// Full route state consumed by the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteFrame {
    pub request: RouteRequest,
    pub resolution: RouteResolution,
    pub intent: RenderIntent,
}

impl RouteFrame {
    pub fn is_root(&self) -> bool {
        route_cwd(self).is_root()
    }

    pub fn is_home(&self) -> bool {
        route_cwd(self).as_str() == "/site"
    }

    pub fn display_path(&self) -> String {
        let path = if self.is_file() {
            self.resolution.node_path.clone()
        } else {
            route_cwd(self)
        };
        display_path_for(&path)
    }

    pub fn is_file(&self) -> bool {
        !matches!(
            self.resolution.kind,
            ResolvedKind::Directory | ResolvedKind::App
        )
    }
}

pub fn push_request_path(path: &str) {
    dom::set_hash(&format!("#{}", normalize_request_path(path)));
}

pub fn replace_request_path(path: &str) {
    dom::replace_hash(&format!("#{}", normalize_request_path(path)));
}

pub fn request_path_for_canonical_path(path: &VirtualPath) -> String {
    if path.as_str() == "/site" {
        "/shell".to_string()
    } else if path.is_root() {
        "/fs".to_string()
    } else {
        format!("/fs/{}", path.as_str().trim_start_matches('/'))
    }
}

pub fn parent_request_path(path: &str) -> String {
    let normalized = normalize_request_path(path);
    if normalized == "/" {
        return "/".to_string();
    }
    if normalized == "/shell" {
        return "/shell".to_string();
    }
    if let Some(rest) = normalized.strip_prefix("/fs/") {
        let Some(current) = canonical_path_from_fs_request(rest) else {
            return "/fs".to_string();
        };
        return current
            .parent()
            .map(|parent| request_path_for_canonical_path(&parent))
            .unwrap_or_else(|| "/fs".to_string());
    }

    match normalized.rsplit_once('/') {
        Some(("", _)) | None => "/".to_string(),
        Some((parent, _)) => parent.to_string(),
    }
}

pub fn route_cwd(frame: &RouteFrame) -> VirtualPath {
    if let Some(cwd) = frame.resolution.params.get("cwd")
        && let Ok(path) = VirtualPath::from_absolute(cwd.clone())
    {
        return path;
    }

    match frame.resolution.kind {
        ResolvedKind::Directory => frame.resolution.node_path.clone(),
        _ => frame
            .resolution
            .node_path
            .parent()
            .unwrap_or_else(VirtualPath::root),
    }
}

pub fn display_path_for(path: &VirtualPath) -> String {
    if path.is_root() {
        return "/".to_string();
    }
    if path.as_str() == "/site" {
        return "~".to_string();
    }
    if let Some(rest) = path.strip_prefix(&VirtualPath::from_absolute("/site").unwrap()) {
        return format!("~/{}", rest);
    }
    path.as_str().to_string()
}

pub fn canonicalize_user_path(cwd: &VirtualPath, raw: &str) -> Option<VirtualPath> {
    if raw.is_empty() || raw == "." {
        return Some(cwd.clone());
    }

    let input = if raw == "~" {
        "/site".to_string()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("/site/{}", rest)
    } else if raw.starts_with('/') {
        raw.to_string()
    } else if cwd.is_root() {
        format!("/{}", raw)
    } else {
        format!("{}/{}", cwd.as_str().trim_end_matches('/'), raw)
    };

    normalize_absolute_path(&input)
}

/// Resolve routes in priority order:
/// 1. explicit metadata route
/// 2. derived index
/// 3. convention fallback
pub fn resolve_route(fs: &GlobalFs, request: &RouteRequest) -> Option<RouteResolution> {
    let path = normalize_request_path(&request.url_path);

    resolve_metadata_route(fs, &path)
        .or_else(|| resolve_index_route(fs, &path))
        .or_else(|| resolve_convention_route(fs, &path))
}

pub fn normalize_request_path(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }

    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_metadata_route(fs: &GlobalFs, request_path: &str) -> Option<RouteResolution> {
    let mut best: Option<(usize, RouteResolution)> = None;

    for (source_path, meta) in fs.metadata_entries() {
        let Some(route) = meta.route.as_deref() else {
            continue;
        };
        let Some(params) = match_route_pattern(route, request_path) else {
            continue;
        };

        let resolution = if route == "/fs/*path" {
            let suffix = params.get("path").cloned().unwrap_or_default();
            let target = canonical_path_from_fs_request(&suffix)?;
            let kind = classify_candidate(fs, &target)?;
            let mut params = params;
            params.insert(
                "cwd".to_string(),
                route_cwd_for_target(fs, &target).to_string(),
            );
            RouteResolution {
                request_path: request_path.to_string(),
                node_path: target,
                kind,
                params,
            }
        } else {
            let mut params = params;
            let node_path = source_path.clone();
            params.insert("cwd".to_string(), "/site".to_string());
            RouteResolution {
                request_path: request_path.to_string(),
                node_path: node_path.clone(),
                kind: metadata_kind(fs, &node_path, meta),
                params,
            }
        };

        let specificity = route.len();
        if best
            .as_ref()
            .is_none_or(|(best_len, _)| specificity > *best_len)
        {
            best = Some((specificity, resolution));
        }
    }

    best.map(|(_, resolution)| resolution)
}

fn resolve_index_route(fs: &GlobalFs, request_path: &str) -> Option<RouteResolution> {
    let entry = fs.route_entry(request_path)?;
    let node_path = VirtualPath::from_absolute(entry.node_path.clone()).ok()?;
    if !fs.exists(&node_path) {
        return None;
    }

    Some(RouteResolution {
        request_path: request_path.to_string(),
        node_path: node_path.clone(),
        kind: resolved_kind_from_index(
            fs,
            &node_path,
            entry.kind.as_ref(),
            entry.renderer.as_ref(),
        ),
        params: BTreeMap::new(),
    })
}

fn resolve_convention_route(fs: &GlobalFs, request_path: &str) -> Option<RouteResolution> {
    let rel = request_path.trim_start_matches('/');

    for candidate in route_candidates(rel) {
        if let Some(kind) = classify_candidate(fs, &candidate) {
            return Some(RouteResolution {
                request_path: request_path.to_string(),
                node_path: candidate,
                kind,
                params: BTreeMap::new(),
            });
        }
    }

    None
}

fn metadata_kind(
    fs: &GlobalFs,
    node_path: &VirtualPath,
    meta: &LoadedNodeMetadata,
) -> ResolvedKind {
    resolved_kind_from_index(fs, node_path, meta.kind.as_ref(), meta.renderer.as_ref())
}

fn resolved_kind_from_index(
    fs: &GlobalFs,
    node_path: &VirtualPath,
    kind: Option<&NodeKind>,
    renderer: Option<&RendererKind>,
) -> ResolvedKind {
    if let Some(kind) = kind {
        return match kind {
            NodeKind::Page => ResolvedKind::Page,
            NodeKind::Document => ResolvedKind::Document,
            NodeKind::App => ResolvedKind::App,
            NodeKind::Asset => ResolvedKind::Asset,
            NodeKind::Redirect => ResolvedKind::Redirect,
            NodeKind::Data => classify_candidate(fs, node_path).unwrap_or(ResolvedKind::Document),
        };
    }

    if let Some(renderer) = renderer {
        return match renderer {
            RendererKind::HtmlPage | RendererKind::MarkdownPage => ResolvedKind::Page,
            RendererKind::DirectoryListing => ResolvedKind::Directory,
            RendererKind::TerminalApp => ResolvedKind::App,
            RendererKind::Image => ResolvedKind::Asset,
            RendererKind::Pdf | RendererKind::DocumentReader | RendererKind::RawText => {
                ResolvedKind::Document
            }
            RendererKind::Redirect => ResolvedKind::Redirect,
        };
    }

    classify_candidate(fs, node_path).unwrap_or(ResolvedKind::Document)
}

fn route_cwd_for_target(fs: &GlobalFs, target: &VirtualPath) -> VirtualPath {
    if fs.is_directory(target) {
        target.clone()
    } else {
        target.parent().unwrap_or_else(VirtualPath::root)
    }
}

fn match_route_pattern(pattern: &str, request_path: &str) -> Option<BTreeMap<String, String>> {
    if let Some((prefix, name)) = pattern.split_once('*') {
        let prefix = prefix.trim_end_matches('/');
        let request = request_path
            .trim_start_matches(prefix)
            .trim_start_matches('/');
        let mut params = BTreeMap::new();
        params.insert(name.to_string(), request.to_string());
        return request_path.starts_with(prefix).then_some(params);
    }

    (pattern == request_path).then(BTreeMap::new)
}

fn classify_candidate(fs: &GlobalFs, candidate: &VirtualPath) -> Option<ResolvedKind> {
    let entry = fs.get_entry(candidate)?;
    if entry.is_directory() {
        return Some(ResolvedKind::Directory);
    }

    let ext = candidate
        .file_name()
        .and_then(|name| name.rsplit_once('.').map(|(_, ext)| ext))
        .unwrap_or("");

    Some(match ext {
        "app" => ResolvedKind::App,
        "md" | "html" => ResolvedKind::Page,
        "link" => ResolvedKind::Redirect,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" => ResolvedKind::Asset,
        "pdf" => ResolvedKind::Document,
        _ => ResolvedKind::Document,
    })
}

fn route_candidates(relative_request: &str) -> Vec<VirtualPath> {
    let mut out = Vec::new();
    let trimmed = relative_request.trim_matches('/');
    let site_root = VirtualPath::from_absolute("/site").expect("constant absolute path");

    if trimmed.is_empty() {
        for suffix in [
            "index.page.html",
            "index.page.md",
            "index.html",
            "index.md",
            "index.app",
            "index.link",
        ] {
            out.push(site_root.join(suffix));
        }
        return out;
    }

    for suffix in [
        format!("{trimmed}.page.html"),
        format!("{trimmed}.page.md"),
        format!("{trimmed}.html"),
        format!("{trimmed}.md"),
        format!("{trimmed}.app"),
        format!("{trimmed}.link"),
        format!("{trimmed}/index.page.html"),
        format!("{trimmed}/index.page.md"),
        format!("{trimmed}/index.html"),
        format!("{trimmed}/index.md"),
        format!("{trimmed}/index.link"),
    ] {
        out.push(site_root.join(&suffix));
    }

    out.push(site_root.join(trimmed));
    out
}

fn canonical_path_from_fs_request(suffix: &str) -> Option<VirtualPath> {
    if suffix.is_empty() {
        return Some(VirtualPath::root());
    }
    normalize_absolute_path(&format!("/{}", suffix))
}

fn normalize_absolute_path(path: &str) -> Option<VirtualPath> {
    let mut parts = Vec::new();
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        match segment {
            "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(segment),
        }
    }

    let normalized = if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    };
    VirtualPath::from_absolute(normalized).ok()
}

#[cfg(test)]
mod tests {
    use crate::core::VirtualFs;
    use crate::core::storage::{ScannedDirectory, ScannedFile, ScannedSubtree};
    use crate::models::{DirectoryMetadata, FileMetadata, FileSidecarMetadata, RouteIndexEntry};

    use super::*;

    fn site(files: &[&str], directories: &[&str]) -> GlobalFs {
        let snapshot = ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    description: (*path).to_string(),
                    meta: FileMetadata::default(),
                })
                .collect(),
            directories: directories
                .iter()
                .map(|path| ScannedDirectory {
                    path: (*path).to_string(),
                    meta: DirectoryMetadata {
                        title: path.rsplit('/').next().unwrap_or(path).to_string(),
                        ..Default::default()
                    },
                })
                .collect(),
        };

        let mut global = GlobalFs::empty();
        global
            .mount_fs(
                VirtualPath::from_absolute("/site").unwrap(),
                &VirtualFs::from_scanned_subtree(&snapshot),
            )
            .unwrap();
        global.set_node_metadata(
            VirtualPath::from_absolute("/site/shell.app").unwrap(),
            FileSidecarMetadata {
                kind: Some(NodeKind::App),
                renderer: Some(RendererKind::TerminalApp),
                route: Some("/shell".to_string()),
                ..Default::default()
            }
            .into(),
        );
        global.set_node_metadata(
            VirtualPath::from_absolute("/site/fs.app").unwrap(),
            FileSidecarMetadata {
                kind: Some(NodeKind::App),
                renderer: Some(RendererKind::TerminalApp),
                route: Some("/fs/*path".to_string()),
                ..Default::default()
            }
            .into(),
        );
        global
    }

    #[test]
    fn route_request_normalizes_leading_and_trailing_slashes() {
        assert_eq!(RouteRequest::new("").url_path, "/");
        assert_eq!(RouteRequest::new("about").url_path, "/about");
        assert_eq!(RouteRequest::new("/about/").url_path, "/about");
    }

    #[test]
    fn resolves_bootstrap_shell_route_from_metadata() {
        let fs = site(&["shell.app"], &[]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/shell")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::App);
        assert_eq!(resolved.node_path.as_str(), "/site/shell.app");
        assert_eq!(
            resolved.params.get("cwd").map(String::as_str),
            Some("/site")
        );
    }

    #[test]
    fn resolves_fs_namespace_to_canonical_target() {
        let fs = site(&["fs.app", "blog/post.md"], &["blog"]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/fs/site/blog/post.md")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/site/blog/post.md");
        assert_eq!(
            resolved.params.get("cwd").map(String::as_str),
            Some("/site/blog")
        );
    }

    #[test]
    fn resolves_route_from_derived_index() {
        let mut fs = site(&["about.md"], &[]);
        fs.replace_route_index([RouteIndexEntry {
            route: "/company".to_string(),
            node_path: "/site/about.md".to_string(),
            kind: Some(NodeKind::Page),
            renderer: Some(RendererKind::MarkdownPage),
        }]);

        let resolved = resolve_route(&fs, &RouteRequest::new("/company")).unwrap();
        assert_eq!(resolved.node_path.as_str(), "/site/about.md");
        assert_eq!(resolved.kind, ResolvedKind::Page);
    }

    #[test]
    fn resolves_root_to_index_page_via_convention_fallback() {
        let fs = site(&["index.page.md"], &[]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/site/index.page.md");
    }

    #[test]
    fn display_path_uses_site_alias() {
        assert_eq!(
            display_path_for(&VirtualPath::from_absolute("/site/blog").unwrap()),
            "~/blog"
        );
        assert_eq!(
            display_path_for(&VirtualPath::from_absolute("/mnt/db").unwrap()),
            "/mnt/db"
        );
    }

    #[test]
    fn canonicalize_user_path_understands_aliases_and_parent_segments() {
        let cwd = VirtualPath::from_absolute("/site/blog").unwrap();
        assert_eq!(
            canonicalize_user_path(&cwd, "../about.md")
                .unwrap()
                .as_str(),
            "/site/about.md"
        );
        assert_eq!(
            canonicalize_user_path(&cwd, "~/posts").unwrap().as_str(),
            "/site/posts"
        );
        assert_eq!(
            canonicalize_user_path(&cwd, "/mnt/db").unwrap().as_str(),
            "/mnt/db"
        );
    }
}
