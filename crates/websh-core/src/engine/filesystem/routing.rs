use std::collections::{BTreeMap, BTreeSet};

use crate::domain::{
    BundleMetadata, BundleValidationError, BundleVariant, NodeKind, NodeMetadata, VirtualPath,
    validate_bundle_metadata,
};
use crate::ports::ScannedSubtree;

use super::content_routes::content_route_for_path;
use super::global_fs::GlobalFs;
use super::intent::RenderIntent;

const SHELL_ROUTE_PREFIX: &str = "/websh";

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
}

/// User-facing route surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RouteSurface {
    /// Canonical content route, e.g. `#/blog/hello`.
    #[default]
    Content,
    /// Shell route for a canonical cwd, e.g. `#/websh/blog`.
    Shell,
}

/// Broad resolution result prior to renderer-specific details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedKind {
    Directory,
    Bundle,
    Page,
    Document,
    App,
    Asset,
    Redirect,
}

/// How a public content route is owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteRole {
    /// A normal content node owns the route derived from its canonical path.
    ContentNode,
    /// The bundle root route renders the declared default variant target.
    BundleDefaultAlias,
    /// The bundle root route selects a localized explicit variant route.
    BundleLocaleSelector,
    /// A non-default variant owns the route derived from its target path.
    BundleVariantTarget,
}

/// Bundle variant metadata attached to a resolved target route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleVariantContext {
    pub bundle_path: VirtualPath,
    pub variant_id: String,
    pub variant_path: VirtualPath,
    pub role: RouteRole,
}

/// Output of route resolution before content loading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteResolution {
    pub request_path: String,
    pub route_path: String,
    pub surface: RouteSurface,
    /// Filesystem node that owns this public route.
    ///
    /// For normal content this is the same as `node_path`; for the bundle
    /// default alias this is the bundle directory while `node_path` is the
    /// default variant render target.
    pub route_owner_path: VirtualPath,
    pub node_path: VirtualPath,
    pub route_role: RouteRole,
    pub kind: ResolvedKind,
    pub params: BTreeMap<String, String>,
    pub bundle_variant: Option<BundleVariantContext>,
}

/// Filesystem node input used by [`RouteCatalog`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteCatalogNode {
    pub path: VirtualPath,
    pub metadata: NodeMetadata,
    pub is_directory: bool,
}

impl RouteCatalogNode {
    pub fn new(path: VirtualPath, metadata: NodeMetadata, is_directory: bool) -> Self {
        Self {
            path,
            metadata,
            is_directory,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RouteCatalogError {
    #[error("duplicate route catalog node path: {path}")]
    DuplicateNode { path: VirtualPath },
    #[error("invalid snapshot path `{path}`: {reason}")]
    InvalidSnapshotPath {
        path: String,
        reason: RouteSnapshotPathError,
    },
    #[error("file node {path} has directory-like kind `{kind:?}`")]
    FileNodeHasDirectoryKind { path: VirtualPath, kind: NodeKind },
    #[error("directory node {path} has file-like kind `{kind:?}`")]
    DirectoryNodeHasFileKind { path: VirtualPath, kind: NodeKind },
    #[error("path {path} has bundle metadata but kind is not `bundle`")]
    BundleMetadataOnNonBundleKind { path: VirtualPath },
    #[error("bundle {path} requires a bundle metadata block")]
    MissingBundleMetadata { path: VirtualPath },
    #[error("bundle {bundle_path} variant `{variant_id}` points to missing node `{path}`")]
    MissingBundleVariantTarget {
        bundle_path: VirtualPath,
        variant_id: String,
        path: VirtualPath,
    },
    #[error("bundle {bundle_path} variant `{variant_id}` points to nested bundle `{path}`")]
    BundleVariantTargetIsBundle {
        bundle_path: VirtualPath,
        variant_id: String,
        path: VirtualPath,
    },
    #[error("content route `{route}` is claimed by both `{first_path}` and `{second_path}`")]
    RouteCollision {
        route: String,
        first_path: VirtualPath,
        second_path: VirtualPath,
    },
    #[error(transparent)]
    Bundle(#[from] BundleValidationError),
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RouteSnapshotPathError {
    #[error("file path must not be empty")]
    EmptyFilePath,
    #[error("path must be repo-relative")]
    Absolute,
    #[error("path must use forward slashes only")]
    Backslash,
    #[error("path contains an empty segment")]
    EmptySegment,
    #[error("path contains a dot segment")]
    DotSegment,
    #[error("path contains a parent segment")]
    ParentSegment,
    #[error("path contains a control character")]
    ControlCharacter,
    #[error("path failed virtual path validation: {reason}")]
    InvalidVirtualPath { reason: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteCatalog {
    routes: BTreeMap<String, RouteResolution>,
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
        !self.is_file() && route_cwd(self).is_root()
    }

    pub fn is_home(&self) -> bool {
        route_cwd(self).is_root()
    }

    pub fn surface(&self) -> RouteSurface {
        self.resolution.surface
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

pub fn route_request_targets_runtime_overlay(request: &RouteRequest) -> bool {
    let trimmed = request.url_path.trim_matches('/');
    if is_runtime_state_request(trimmed) {
        return true;
    }
    trimmed
        .strip_prefix("websh/")
        .is_some_and(is_runtime_state_request)
}

pub fn request_path_for_canonical_path(path: &VirtualPath, surface: RouteSurface) -> String {
    match surface {
        RouteSurface::Content => {
            if path.is_root() {
                "/".to_string()
            } else {
                content_route_for_path(path.as_str())
            }
        }
        RouteSurface::Shell => surface_request_path(SHELL_ROUTE_PREFIX, path),
    }
}

fn is_runtime_state_request(path: &str) -> bool {
    path == ".websh/state" || path.starts_with(".websh/state/")
}

pub fn parent_request_path(path: &str) -> String {
    let normalized = normalize_request_path(path);
    if normalized == "/" {
        return "/".to_string();
    }

    if let Some((surface, current)) = surface_target_from_request(&normalized) {
        return current
            .parent()
            .map(|parent| request_path_for_canonical_path(&parent, surface))
            .unwrap_or_else(|| request_path_for_canonical_path(&VirtualPath::root(), surface));
    }

    if let Ok(current) = VirtualPath::from_absolute(normalized.clone()) {
        return current
            .parent()
            .map(|parent| request_path_for_canonical_path(&parent, RouteSurface::Content))
            .unwrap_or_else(|| "/".to_string());
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
        return "~".to_string();
    }
    path.as_str().to_string()
}

pub fn canonicalize_user_path(cwd: &VirtualPath, raw: &str) -> Option<VirtualPath> {
    if raw.is_empty() || raw == "." {
        return Some(cwd.clone());
    }

    let input = if raw == "~" {
        "/".to_string()
    } else if let Some(rest) = raw.strip_prefix("~/") {
        format!("/{}", rest)
    } else if raw.starts_with('/') {
        raw.to_string()
    } else if cwd.is_root() {
        format!("/{}", raw)
    } else {
        format!("{}/{}", cwd.as_str().trim_end_matches('/'), raw)
    };

    normalize_absolute_path(&input)
}

/// Resolve routes by exact lookup against the content route catalog, after
/// reserved shell routes have had their chance to claim the request.
pub fn resolve_route(fs: &GlobalFs, request: &RouteRequest) -> Option<RouteResolution> {
    try_resolve_route(fs, request).ok().flatten()
}

pub fn try_resolve_route(
    fs: &GlobalFs,
    request: &RouteRequest,
) -> Result<Option<RouteResolution>, RouteCatalogError> {
    let path = normalize_request_path(&request.url_path);
    if is_reserved_request_path(&path) {
        return Ok(resolve_reserved_route(fs, &path));
    }
    let catalog = RouteCatalog::from_global_fs(fs)?;
    Ok(resolve_route_with_catalog(fs, &catalog, request))
}

pub fn resolve_route_with_catalog(
    fs: &GlobalFs,
    catalog: &RouteCatalog,
    request: &RouteRequest,
) -> Option<RouteResolution> {
    let path = normalize_request_path(&request.url_path);

    if is_reserved_request_path(&path) {
        return resolve_reserved_route(fs, &path);
    }

    catalog.resolve(&path)
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

fn resolve_reserved_route(fs: &GlobalFs, request_path: &str) -> Option<RouteResolution> {
    let (surface, cwd) = surface_target_from_request(request_path)?;
    if !fs.is_directory(&cwd) {
        return None;
    }

    let mut params = BTreeMap::new();
    params.insert("cwd".to_string(), cwd.to_string());

    Some(RouteResolution {
        request_path: request_path.to_string(),
        route_path: request_path.to_string(),
        surface,
        route_owner_path: cwd.clone(),
        node_path: cwd,
        route_role: RouteRole::ContentNode,
        kind: match surface {
            RouteSurface::Shell => ResolvedKind::App,
            RouteSurface::Content => return None,
        },
        params,
        bundle_variant: None,
    })
}

fn is_reserved_request_path(request_path: &str) -> bool {
    request_path == SHELL_ROUTE_PREFIX
        || request_path.starts_with(&format!("{SHELL_ROUTE_PREFIX}/"))
}

fn bundle_child_path(bundle_path: &VirtualPath, rel_path: &str) -> Option<VirtualPath> {
    if rel_path.is_empty()
        || rel_path.starts_with('/')
        || rel_path.contains('\\')
        || rel_path.chars().any(char::is_control)
    {
        return None;
    }
    if rel_path
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return None;
    }
    let path = bundle_path.join(rel_path);
    path.starts_with(bundle_path).then_some(path)
}

impl RouteCatalog {
    pub fn from_nodes(
        nodes: impl IntoIterator<Item = RouteCatalogNode>,
    ) -> Result<Self, RouteCatalogError> {
        let nodes = catalog_node_map(nodes)?;
        Self::from_node_map(&nodes)
    }

    pub fn from_global_fs(fs: &GlobalFs) -> Result<Self, RouteCatalogError> {
        let nodes = fs
            .metadata_entries()
            .into_iter()
            .filter_map(|(path, metadata)| {
                fs.get_entry(&path).map(|entry| {
                    RouteCatalogNode::new(path, metadata.clone(), entry.is_directory())
                })
            })
            .collect::<Vec<_>>();
        Self::from_nodes(nodes)
    }

    pub fn from_snapshot(snapshot: &ScannedSubtree) -> Result<Self, RouteCatalogError> {
        Self::from_nodes(route_catalog_nodes_from_snapshot(snapshot)?)
    }

    pub fn resolve(&self, normalized_request_path: &str) -> Option<RouteResolution> {
        let path = normalize_request_path(normalized_request_path);
        self.routes.get(&path).cloned().map(|mut resolution| {
            resolution.request_path = path;
            resolution
        })
    }

    pub fn validate_nodes(
        nodes: impl IntoIterator<Item = RouteCatalogNode>,
    ) -> Result<(), RouteCatalogError> {
        Self::from_nodes(nodes).map(|_| ())
    }

    pub fn validate_snapshot(snapshot: &ScannedSubtree) -> Result<(), RouteCatalogError> {
        Self::from_snapshot(snapshot).map(|_| ())
    }

    fn from_node_map(
        nodes: &BTreeMap<VirtualPath, RouteCatalogNode>,
    ) -> Result<Self, RouteCatalogError> {
        let mut builder = RouteCatalogBuilder::default();

        for node in nodes.values() {
            classify_catalog_node(node)?;
        }

        for (bundle_path, node) in nodes {
            if node.metadata.bundle.is_some() && node.metadata.kind != NodeKind::Bundle {
                return Err(RouteCatalogError::BundleMetadataOnNonBundleKind {
                    path: bundle_path.clone(),
                });
            }
            if !node.metadata.is_bundle() {
                continue;
            }
            let bundle = node.metadata.bundle.as_ref().ok_or_else(|| {
                RouteCatalogError::MissingBundleMetadata {
                    path: bundle_path.clone(),
                }
            })?;
            validate_bundle_metadata(bundle_path.as_str().trim_start_matches('/'), bundle)?;
            builder.add_bundle(nodes, bundle_path, bundle)?;
        }

        for (path, node) in nodes {
            if node.metadata.is_bundle() || builder.variant_targets.contains(path) {
                continue;
            }
            let kind = classify_catalog_node(node)?;
            if matches!(kind, ResolvedKind::Bundle) {
                continue;
            }
            let route = content_route_for_path(path.as_str());
            builder.insert_route(
                route,
                RouteResolution {
                    request_path: String::new(),
                    route_path: String::new(),
                    surface: RouteSurface::Content,
                    route_owner_path: path.clone(),
                    node_path: path.clone(),
                    route_role: RouteRole::ContentNode,
                    kind,
                    params: BTreeMap::new(),
                    bundle_variant: None,
                },
            )?;
        }

        Ok(Self {
            routes: builder.routes,
        })
    }
}

#[derive(Default)]
struct RouteCatalogBuilder {
    routes: BTreeMap<String, RouteResolution>,
    variant_targets: BTreeSet<VirtualPath>,
    suppressed_default_routes: BTreeMap<String, VirtualPath>,
}

impl RouteCatalogBuilder {
    fn add_bundle(
        &mut self,
        nodes: &BTreeMap<VirtualPath, RouteCatalogNode>,
        bundle_path: &VirtualPath,
        bundle: &BundleMetadata,
    ) -> Result<(), RouteCatalogError> {
        let bundle_route = content_route_for_path(bundle_path.as_str());

        if let Some(default_id) = bundle.static_default_variant_id() {
            let default_variant = bundle
                .variant_by_id(default_id)
                .expect("bundle metadata validation ensures default variant exists");
            let default_path = variant_target_path(bundle_path, default_variant);
            let default_node = bundle_variant_target_node(nodes, bundle_path, default_variant)?;
            let default_kind =
                classify_bundle_variant_target(default_node, bundle_path, default_variant)?;

            self.variant_targets.insert(default_path.clone());
            let default_route = content_route_for_path(default_path.as_str());
            if default_route != bundle_route {
                self.suppressed_default_routes
                    .insert(default_route, default_path.clone());
            }
            self.insert_route(
                bundle_route,
                route_resolution_for_variant(
                    bundle_path,
                    default_variant.id.as_str(),
                    &default_path,
                    default_kind,
                    RouteRole::BundleDefaultAlias,
                ),
            )?;
        } else {
            self.insert_route(
                bundle_route,
                route_resolution_for_bundle_selector(bundle_path.clone()),
            )?;
        }

        for variant in &bundle.variants {
            let variant_path = variant_target_path(bundle_path, variant);
            self.variant_targets.insert(variant_path.clone());
            if bundle.is_static_default_variant(&variant.id) {
                continue;
            }
            let variant_node = bundle_variant_target_node(nodes, bundle_path, variant)?;
            let kind = classify_bundle_variant_target(variant_node, bundle_path, variant)?;
            self.insert_route(
                content_route_for_path(variant_path.as_str()),
                route_resolution_for_variant(
                    bundle_path,
                    variant.id.as_str(),
                    &variant_path,
                    kind,
                    RouteRole::BundleVariantTarget,
                ),
            )?;
        }

        Ok(())
    }

    fn insert_route(
        &mut self,
        route: String,
        mut resolution: RouteResolution,
    ) -> Result<(), RouteCatalogError> {
        if let Some(first_path) = self.suppressed_default_routes.get(&route) {
            return Err(RouteCatalogError::RouteCollision {
                route,
                first_path: first_path.clone(),
                second_path: resolution.route_owner_path,
            });
        }
        if let Some(existing) = self.routes.get(&route) {
            return Err(RouteCatalogError::RouteCollision {
                route,
                first_path: existing.route_owner_path.clone(),
                second_path: resolution.route_owner_path,
            });
        }
        resolution.request_path = route.clone();
        resolution.route_path = route.clone();
        self.routes.insert(route, resolution);
        Ok(())
    }
}

fn catalog_node_map(
    nodes: impl IntoIterator<Item = RouteCatalogNode>,
) -> Result<BTreeMap<VirtualPath, RouteCatalogNode>, RouteCatalogError> {
    let mut out = BTreeMap::new();
    for node in nodes {
        let path = node.path.clone();
        if out.insert(path.clone(), node).is_some() {
            return Err(RouteCatalogError::DuplicateNode { path });
        }
    }
    Ok(out)
}

fn route_catalog_nodes_from_snapshot(
    snapshot: &ScannedSubtree,
) -> Result<Vec<RouteCatalogNode>, RouteCatalogError> {
    let mut nodes = Vec::with_capacity(snapshot.files.len() + snapshot.directories.len());
    for dir in &snapshot.directories {
        let path = snapshot_path_to_virtual_path(&dir.path, true)?;
        nodes.push(RouteCatalogNode::new(path, dir.meta.clone(), true));
    }
    for file in &snapshot.files {
        let path = snapshot_path_to_virtual_path(&file.path, false)?;
        nodes.push(RouteCatalogNode::new(path, file.meta.clone(), false));
    }
    Ok(nodes)
}

fn snapshot_path_to_virtual_path(
    path: &str,
    is_directory: bool,
) -> Result<VirtualPath, RouteCatalogError> {
    if path.is_empty() {
        return if is_directory {
            Ok(VirtualPath::root())
        } else {
            Err(invalid_snapshot_path(
                path,
                RouteSnapshotPathError::EmptyFilePath,
            ))
        };
    }
    if path.starts_with('/') {
        return Err(invalid_snapshot_path(
            path,
            RouteSnapshotPathError::Absolute,
        ));
    }
    if path.contains('\\') {
        return Err(invalid_snapshot_path(
            path,
            RouteSnapshotPathError::Backslash,
        ));
    }
    if path.chars().any(char::is_control) {
        return Err(invalid_snapshot_path(
            path,
            RouteSnapshotPathError::ControlCharacter,
        ));
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(invalid_snapshot_path(
                path,
                RouteSnapshotPathError::EmptySegment,
            ));
        }
        if segment == "." {
            return Err(invalid_snapshot_path(
                path,
                RouteSnapshotPathError::DotSegment,
            ));
        }
        if segment == ".." {
            return Err(invalid_snapshot_path(
                path,
                RouteSnapshotPathError::ParentSegment,
            ));
        }
    }

    VirtualPath::from_absolute(format!("/{path}")).map_err(|err| {
        invalid_snapshot_path(
            path,
            RouteSnapshotPathError::InvalidVirtualPath {
                reason: err.to_string(),
            },
        )
    })
}

fn invalid_snapshot_path(path: &str, reason: RouteSnapshotPathError) -> RouteCatalogError {
    RouteCatalogError::InvalidSnapshotPath {
        path: path.to_string(),
        reason,
    }
}

fn variant_target_path(bundle_path: &VirtualPath, variant: &BundleVariant) -> VirtualPath {
    bundle_child_path(bundle_path, &variant.path)
        .expect("bundle metadata validation ensures variant paths are portable")
}

fn bundle_variant_target_node<'a>(
    nodes: &'a BTreeMap<VirtualPath, RouteCatalogNode>,
    bundle_path: &VirtualPath,
    variant: &BundleVariant,
) -> Result<&'a RouteCatalogNode, RouteCatalogError> {
    let target_path = variant_target_path(bundle_path, variant);
    nodes
        .get(&target_path)
        .ok_or_else(|| RouteCatalogError::MissingBundleVariantTarget {
            bundle_path: bundle_path.clone(),
            variant_id: variant.id.clone(),
            path: target_path,
        })
}

fn classify_bundle_variant_target(
    node: &RouteCatalogNode,
    bundle_path: &VirtualPath,
    variant: &BundleVariant,
) -> Result<ResolvedKind, RouteCatalogError> {
    let kind = classify_catalog_node(node)?;
    if matches!(kind, ResolvedKind::Bundle) {
        return Err(RouteCatalogError::BundleVariantTargetIsBundle {
            bundle_path: bundle_path.clone(),
            variant_id: variant.id.clone(),
            path: node.path.clone(),
        });
    }
    Ok(kind)
}

fn route_resolution_for_variant(
    bundle_path: &VirtualPath,
    variant_id: &str,
    variant_path: &VirtualPath,
    kind: ResolvedKind,
    role: RouteRole,
) -> RouteResolution {
    let route_owner_path = match role {
        RouteRole::BundleDefaultAlias => bundle_path.clone(),
        RouteRole::BundleLocaleSelector => bundle_path.clone(),
        RouteRole::BundleVariantTarget => variant_path.clone(),
        RouteRole::ContentNode => variant_path.clone(),
    };
    RouteResolution {
        request_path: String::new(),
        route_path: String::new(),
        surface: RouteSurface::Content,
        route_owner_path,
        node_path: variant_path.clone(),
        route_role: role,
        kind,
        params: BTreeMap::new(),
        bundle_variant: Some(BundleVariantContext {
            bundle_path: bundle_path.clone(),
            variant_id: variant_id.to_string(),
            variant_path: variant_path.clone(),
            role,
        }),
    }
}

fn route_resolution_for_bundle_selector(bundle_path: VirtualPath) -> RouteResolution {
    RouteResolution {
        request_path: String::new(),
        route_path: String::new(),
        surface: RouteSurface::Content,
        route_owner_path: bundle_path.clone(),
        node_path: bundle_path,
        route_role: RouteRole::BundleLocaleSelector,
        kind: ResolvedKind::Bundle,
        params: BTreeMap::new(),
        bundle_variant: None,
    }
}

fn classify_catalog_node(node: &RouteCatalogNode) -> Result<ResolvedKind, RouteCatalogError> {
    let metadata = &node.metadata;
    if node.is_directory {
        return match metadata.kind {
            NodeKind::Directory => Ok(ResolvedKind::Directory),
            NodeKind::Bundle => Ok(ResolvedKind::Bundle),
            kind => Err(RouteCatalogError::DirectoryNodeHasFileKind {
                path: node.path.clone(),
                kind,
            }),
        };
    }

    if metadata.kind.is_directory_like() {
        return Err(RouteCatalogError::FileNodeHasDirectoryKind {
            path: node.path.clone(),
            kind: metadata.kind,
        });
    }

    match metadata.kind {
        NodeKind::Page => Ok(ResolvedKind::Page),
        NodeKind::Document | NodeKind::Data => Ok(ResolvedKind::Document),
        NodeKind::Asset => Ok(ResolvedKind::Asset),
        NodeKind::App => Ok(ResolvedKind::App),
        NodeKind::Redirect => Ok(ResolvedKind::Redirect),
        NodeKind::Directory | NodeKind::Bundle => {
            unreachable!("directory-like file kind is rejected before classification")
        }
    }
}

fn surface_request_path(prefix: &str, path: &VirtualPath) -> String {
    if path.is_root() {
        prefix.to_string()
    } else {
        format!("{}/{}", prefix, path.as_str().trim_start_matches('/'))
    }
}

fn surface_target_from_request(request_path: &str) -> Option<(RouteSurface, VirtualPath)> {
    if request_path == SHELL_ROUTE_PREFIX {
        return Some((RouteSurface::Shell, VirtualPath::root()));
    }
    if let Some(rest) = request_path.strip_prefix(&format!("{SHELL_ROUTE_PREFIX}/")) {
        return normalize_absolute_path(&format!("/{rest}"))
            .map(|path| (RouteSurface::Shell, path));
    }
    None
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
    use crate::domain::{
        BundleDefaultVariant, BundleMetadata, BundleVariant, EntryExtensions, Fields, NodeKind,
        NodeMetadata,
    };
    use crate::ports::{ScannedDirectory, ScannedFile, ScannedSubtree};

    use super::*;

    fn make_meta(kind: NodeKind) -> NodeMetadata {
        NodeMetadata {
            kind,
            bundle: None,
            authored: Fields::default(),
            derived: Fields::default(),
        }
    }

    fn make_dir_meta(name: &str) -> NodeMetadata {
        NodeMetadata {
            kind: NodeKind::Directory,
            bundle: None,
            authored: Fields {
                title: Some(name.to_string()),
                ..Fields::default()
            },
            derived: Fields::default(),
        }
    }

    fn static_default(id: &str) -> BundleDefaultVariant {
        BundleDefaultVariant::Static { id: id.to_string() }
    }

    fn locale_default(fallback: &str) -> BundleDefaultVariant {
        BundleDefaultVariant::Locale {
            fallback: fallback.to_string(),
        }
    }

    fn site(files: &[&str], directories: &[&str]) -> GlobalFs {
        let snapshot = ScannedSubtree {
            files: files
                .iter()
                .map(|path| ScannedFile {
                    path: (*path).to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                })
                .collect(),
            directories: directories
                .iter()
                .map(|path| ScannedDirectory {
                    path: (*path).to_string(),
                    meta: make_dir_meta(path.rsplit('/').next().unwrap_or(path)),
                })
                .collect(),
        };

        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    fn bundle_site() -> GlobalFs {
        let mut snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/fr.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/ko.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: "writing".to_string(),
                    meta: make_dir_meta("writing"),
                },
                ScannedDirectory {
                    path: "writing/foo".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Bundle,
                        bundle: Some(BundleMetadata {
                            default_variant: static_default("en"),
                            variants: vec![
                                BundleVariant {
                                    id: "en".to_string(),
                                    path: "en.md".to_string(),
                                    label: "English".to_string(),
                                    locale: Some("en".to_string()),
                                    media_type: None,
                                },
                                BundleVariant {
                                    id: "ko".to_string(),
                                    path: "ko.md".to_string(),
                                    label: "Korean".to_string(),
                                    locale: Some("ko".to_string()),
                                    media_type: None,
                                },
                            ],
                        }),
                        authored: Fields {
                            title: Some("Foo".to_string()),
                            ..Fields::default()
                        },
                        derived: Fields {
                            kind: Some(NodeKind::Bundle),
                            ..Fields::default()
                        },
                    },
                },
            ],
        };
        snapshot.files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    fn locale_bundle_site() -> GlobalFs {
        let mut snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/ko.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: "writing".to_string(),
                    meta: make_dir_meta("writing"),
                },
                ScannedDirectory {
                    path: "writing/foo".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Bundle,
                        bundle: Some(BundleMetadata {
                            default_variant: locale_default("en"),
                            variants: vec![
                                BundleVariant {
                                    id: "en".to_string(),
                                    path: "en.md".to_string(),
                                    label: "English".to_string(),
                                    locale: Some("en".to_string()),
                                    media_type: None,
                                },
                                BundleVariant {
                                    id: "ko".to_string(),
                                    path: "ko.md".to_string(),
                                    label: "Korean".to_string(),
                                    locale: Some("ko".to_string()),
                                    media_type: None,
                                },
                            ],
                        }),
                        authored: Fields {
                            title: Some("Foo".to_string()),
                            ..Fields::default()
                        },
                        derived: Fields {
                            kind: Some(NodeKind::Bundle),
                            ..Fields::default()
                        },
                    },
                },
            ],
        };
        snapshot.files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut global = GlobalFs::empty();
        global
            .mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();
        global
    }

    #[test]
    fn route_request_normalizes_leading_and_trailing_slashes() {
        assert_eq!(RouteRequest::new("").url_path, "/");
        assert_eq!(RouteRequest::new("about").url_path, "/about");
        assert_eq!(RouteRequest::new("/about/").url_path, "/about");
    }

    #[test]
    fn resolves_shell_route_from_reserved_surface() {
        let fs = site(&["blog/post.md"], &["blog"]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/websh")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::App);
        assert_eq!(resolved.surface, RouteSurface::Shell);
        assert_eq!(resolved.node_path.as_str(), "/");
        assert_eq!(resolved.params.get("cwd").map(String::as_str), Some("/"));
    }

    #[test]
    fn resolves_nested_shell_route_to_canonical_cwd() {
        let fs = site(&["blog/post.md"], &["blog"]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/websh/blog")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::App);
        assert_eq!(resolved.surface, RouteSurface::Shell);
        assert_eq!(resolved.node_path.as_str(), "/blog");
        assert_eq!(
            resolved.params.get("cwd").map(String::as_str),
            Some("/blog")
        );
    }

    #[test]
    fn reserved_shell_route_wins_over_content_node() {
        let fs = site(&["shell/index.md"], &["shell"]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/websh")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::App);
        assert_eq!(resolved.surface, RouteSurface::Shell);
        assert_eq!(resolved.node_path.as_str(), "/");
    }

    #[test]
    fn resolves_root_to_root_directory_by_exact_route() {
        let fs = site(&[], &[]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Directory);
        assert_eq!(resolved.node_path.as_str(), "/");
    }

    #[test]
    fn resolves_canonical_content_route_to_render_target() {
        let fs = site(&["about.md", "db/fresh.md"], &["db"]);
        let resolved = resolve_route(&fs, &RouteRequest::new("/db/fresh")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.surface, RouteSurface::Content);
        assert_eq!(resolved.node_path.as_str(), "/db/fresh.md");
    }

    #[test]
    fn reader_routes_use_canonical_extensionless_paths() {
        let fs = site(&["db/fresh.md"], &["db"]);
        assert!(resolve_route(&fs, &RouteRequest::new("/db/fresh.md")).is_none());
    }

    #[test]
    fn resolves_bundle_root_to_bundle_directory() {
        let fs = bundle_site();
        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/en.md");
        let context = resolved.bundle_variant.as_ref().unwrap();
        assert_eq!(context.bundle_path.as_str(), "/writing/foo");
        assert_eq!(context.variant_id, "en");
        assert_eq!(context.variant_path.as_str(), "/writing/foo/en.md");
        assert_eq!(resolved.route_role, RouteRole::BundleDefaultAlias);
        assert_eq!(context.role, RouteRole::BundleDefaultAlias);
    }

    #[test]
    fn default_bundle_route_ignores_locale_preferences() {
        let fs = bundle_site();
        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/en.md");
        let context = resolved.bundle_variant.as_ref().unwrap();
        assert_eq!(context.variant_id, "en");
        assert_eq!(resolved.route_role, RouteRole::BundleDefaultAlias);
        assert_eq!(context.role, RouteRole::BundleDefaultAlias);
    }

    #[test]
    fn non_default_variant_resolves_at_target_derived_route() {
        let fs = bundle_site();
        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo/ko")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/ko.md");
        let context = resolved.bundle_variant.as_ref().unwrap();
        assert_eq!(context.bundle_path.as_str(), "/writing/foo");
        assert_eq!(context.variant_id, "ko");
        assert_eq!(resolved.route_role, RouteRole::BundleVariantTarget);
        assert_eq!(context.role, RouteRole::BundleVariantTarget);
    }

    #[test]
    fn extensionful_bundle_variant_target_route_is_unresolved() {
        let fs = bundle_site();
        assert!(resolve_route(&fs, &RouteRequest::new("/writing/foo/ko.md")).is_none());
    }

    #[test]
    fn default_variant_target_route_is_suppressed() {
        let fs = bundle_site();
        assert!(resolve_route(&fs, &RouteRequest::new("/writing/foo/en")).is_none());
    }

    #[test]
    fn locale_bundle_root_resolves_to_selector_endpoint() {
        let fs = locale_bundle_site();
        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Bundle);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo");
        assert_eq!(resolved.route_owner_path.as_str(), "/writing/foo");
        assert_eq!(resolved.route_role, RouteRole::BundleLocaleSelector);
        assert_eq!(resolved.bundle_variant, None);
    }

    #[test]
    fn locale_fallback_variant_resolves_at_explicit_route() {
        let fs = locale_bundle_site();
        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo/en")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/en.md");
        assert_eq!(resolved.route_role, RouteRole::BundleVariantTarget);
        let context = resolved.bundle_variant.as_ref().unwrap();
        assert_eq!(context.variant_id, "en");
        assert_eq!(context.role, RouteRole::BundleVariantTarget);
    }

    #[test]
    fn locale_explicit_variant_route_resolves_without_selector_alias() {
        let fs = locale_bundle_site();
        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo/ko")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Page);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/ko.md");
        assert_eq!(resolved.route_role, RouteRole::BundleVariantTarget);
        let context = resolved.bundle_variant.as_ref().unwrap();
        assert_eq!(context.variant_id, "ko");
        assert_eq!(context.role, RouteRole::BundleVariantTarget);
    }

    #[test]
    fn variant_id_only_route_is_not_an_alias() {
        let fs = bundle_site();
        assert!(resolve_route(&fs, &RouteRequest::new("/writing/foo/print_pdf")).is_none());
    }

    #[test]
    fn resolves_directory_bundle_variant_to_directory_target() {
        let mut snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/fr.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/notes/readme.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![
                ScannedDirectory {
                    path: "writing/foo".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Bundle,
                        bundle: Some(BundleMetadata {
                            default_variant: static_default("en"),
                            variants: vec![
                                BundleVariant {
                                    id: "en".to_string(),
                                    path: "en.md".to_string(),
                                    label: "English".to_string(),
                                    locale: None,
                                    media_type: None,
                                },
                                BundleVariant {
                                    id: "notes".to_string(),
                                    path: "notes".to_string(),
                                    label: "Notes".to_string(),
                                    locale: None,
                                    media_type: None,
                                },
                            ],
                        }),
                        authored: Fields::default(),
                        derived: Fields {
                            kind: Some(NodeKind::Bundle),
                            ..Fields::default()
                        },
                    },
                },
                ScannedDirectory {
                    path: "writing/foo/notes".to_string(),
                    meta: make_dir_meta("Notes"),
                },
            ],
        };
        snapshot.files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();

        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo/notes")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Directory);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/notes");
        assert_eq!(
            resolved
                .bundle_variant
                .as_ref()
                .map(|context| context.variant_id.as_str()),
            Some("notes")
        );
        assert_eq!(
            resolved.bundle_variant.as_ref().map(|context| context.role),
            Some(RouteRole::BundleVariantTarget)
        );
    }

    #[test]
    fn resolves_pdf_bundle_variant_at_target_derived_route() {
        let mut snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/print.pdf".to_string(),
                    meta: make_meta(NodeKind::Document),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![ScannedDirectory {
                path: "writing/foo".to_string(),
                meta: NodeMetadata {
                    kind: NodeKind::Bundle,
                    bundle: Some(BundleMetadata {
                        default_variant: static_default("en"),
                        variants: vec![
                            BundleVariant {
                                id: "en".to_string(),
                                path: "en.md".to_string(),
                                label: "English".to_string(),
                                locale: None,
                                media_type: None,
                            },
                            BundleVariant {
                                id: "print_pdf".to_string(),
                                path: "print.pdf".to_string(),
                                label: "Print".to_string(),
                                locale: None,
                                media_type: None,
                            },
                        ],
                    }),
                    authored: Fields::default(),
                    derived: Fields {
                        kind: Some(NodeKind::Bundle),
                        ..Fields::default()
                    },
                },
            }],
        };
        snapshot.files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();

        let resolved = resolve_route(&fs, &RouteRequest::new("/writing/foo/print.pdf")).unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Document);
        assert_eq!(resolved.node_path.as_str(), "/writing/foo/print.pdf");
        assert_eq!(resolved.route_role, RouteRole::BundleVariantTarget);
        assert_eq!(
            resolved
                .bundle_variant
                .as_ref()
                .map(|context| context.variant_id.as_str()),
            Some("print_pdf")
        );
    }

    #[test]
    fn route_catalog_rejects_markdown_html_route_collision() {
        let fs = site(
            &["writing/foo/ko.md", "writing/foo/ko.html"],
            &["writing/foo"],
        );

        let err = RouteCatalog::from_global_fs(&fs).unwrap_err();

        assert!(matches!(
            err,
            RouteCatalogError::RouteCollision { route, .. } if route == "/writing/foo/ko"
        ));
    }

    #[test]
    fn route_catalog_rejects_pdf_markdown_route_collision() {
        let fs = site(
            &["writing/foo/print.pdf", "writing/foo/print.pdf.md"],
            &["writing/foo"],
        );

        let err = RouteCatalog::from_global_fs(&fs).unwrap_err();

        assert!(matches!(
            err,
            RouteCatalogError::RouteCollision { route, .. } if route == "/writing/foo/print.pdf"
        ));
    }

    #[test]
    fn route_catalog_rejects_default_target_route_claimed_by_another_node() {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/en.html".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![ScannedDirectory {
                path: "writing/foo".to_string(),
                meta: NodeMetadata {
                    kind: NodeKind::Bundle,
                    bundle: Some(BundleMetadata {
                        default_variant: static_default("en"),
                        variants: vec![BundleVariant {
                            id: "en".to_string(),
                            path: "en.md".to_string(),
                            label: "English".to_string(),
                            locale: None,
                            media_type: None,
                        }],
                    }),
                    authored: Fields::default(),
                    derived: Fields::default(),
                },
            }],
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();

        let err = RouteCatalog::from_global_fs(&fs).unwrap_err();

        assert!(
            matches!(
                err,
                RouteCatalogError::RouteCollision { ref route, .. } if route == "/writing/foo/en"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn route_catalog_rejects_duplicate_variant_public_routes() {
        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/foo/fr.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/en.md".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/foo/en.html".to_string(),
                    meta: make_meta(NodeKind::Page),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: vec![ScannedDirectory {
                path: "writing/foo".to_string(),
                meta: NodeMetadata {
                    kind: NodeKind::Bundle,
                    bundle: Some(BundleMetadata {
                        default_variant: static_default("fr"),
                        variants: vec![
                            BundleVariant {
                                id: "fr".to_string(),
                                path: "fr.md".to_string(),
                                label: "French".to_string(),
                                locale: None,
                                media_type: None,
                            },
                            BundleVariant {
                                id: "en_md".to_string(),
                                path: "en.md".to_string(),
                                label: "English".to_string(),
                                locale: None,
                                media_type: None,
                            },
                            BundleVariant {
                                id: "en_html".to_string(),
                                path: "en.html".to_string(),
                                label: "English HTML".to_string(),
                                locale: None,
                                media_type: None,
                            },
                        ],
                    }),
                    authored: Fields::default(),
                    derived: Fields::default(),
                },
            }],
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();

        let err = RouteCatalog::from_global_fs(&fs).unwrap_err();

        assert!(
            matches!(
                err,
                RouteCatalogError::RouteCollision { ref route, .. } if route == "/writing/foo/en"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn route_catalog_rejects_nested_bundle_variant_target() {
        let snapshot = ScannedSubtree {
            files: vec![ScannedFile {
                path: "writing/foo/nested/en.md".to_string(),
                meta: make_meta(NodeKind::Page),
                extensions: EntryExtensions::default(),
            }],
            directories: vec![
                ScannedDirectory {
                    path: "writing/foo".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Bundle,
                        bundle: Some(BundleMetadata {
                            default_variant: static_default("nested"),
                            variants: vec![BundleVariant {
                                id: "nested".to_string(),
                                path: "nested".to_string(),
                                label: "Nested".to_string(),
                                locale: None,
                                media_type: None,
                            }],
                        }),
                        authored: Fields::default(),
                        derived: Fields::default(),
                    },
                },
                ScannedDirectory {
                    path: "writing/foo/nested".to_string(),
                    meta: NodeMetadata {
                        kind: NodeKind::Bundle,
                        bundle: Some(BundleMetadata {
                            default_variant: static_default("en"),
                            variants: vec![BundleVariant {
                                id: "en".to_string(),
                                path: "en.md".to_string(),
                                label: "English".to_string(),
                                locale: None,
                                media_type: None,
                            }],
                        }),
                        authored: Fields::default(),
                        derived: Fields::default(),
                    },
                },
            ],
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .unwrap();

        let err = RouteCatalog::from_global_fs(&fs).unwrap_err();

        assert!(matches!(
            err,
            RouteCatalogError::BundleVariantTargetIsBundle { variant_id, .. }
                if variant_id == "nested"
        ));
    }

    #[test]
    fn route_catalog_uses_top_level_kind_for_structural_classification() {
        let mut meta = make_meta(NodeKind::Asset);
        meta.authored.kind = Some(NodeKind::Page);
        assert_eq!(meta.effective_kind(), NodeKind::Page);

        let catalog = RouteCatalog::from_nodes([RouteCatalogNode::new(
            VirtualPath::from_absolute("/docs/readme.md").unwrap(),
            meta,
            false,
        )])
        .unwrap();
        let resolved = catalog.resolve("/docs/readme").unwrap();

        assert_eq!(resolved.kind, ResolvedKind::Asset);
        assert_eq!(resolved.node_path.as_str(), "/docs/readme.md");
    }

    #[test]
    fn route_catalog_rejects_file_node_with_directory_like_kind() {
        let err = RouteCatalog::from_nodes([RouteCatalogNode::new(
            VirtualPath::from_absolute("/docs/readme.md").unwrap(),
            make_meta(NodeKind::Directory),
            false,
        )])
        .unwrap_err();

        assert!(matches!(
            err,
            RouteCatalogError::FileNodeHasDirectoryKind { path, kind }
                if path.as_str() == "/docs/readme.md" && kind == NodeKind::Directory
        ));
    }

    #[test]
    fn route_catalog_rejects_directory_node_with_file_like_kind() {
        let err = RouteCatalog::from_nodes([RouteCatalogNode::new(
            VirtualPath::from_absolute("/docs").unwrap(),
            make_meta(NodeKind::Page),
            true,
        )])
        .unwrap_err();

        assert!(matches!(
            err,
            RouteCatalogError::DirectoryNodeHasFileKind { path, kind }
                if path.as_str() == "/docs" && kind == NodeKind::Page
        ));
    }

    #[test]
    fn route_catalog_rejects_invalid_snapshot_paths() {
        let cases = [
            (String::new(), false, RouteSnapshotPathError::EmptyFilePath),
            (
                "/absolute.md".to_string(),
                false,
                RouteSnapshotPathError::Absolute,
            ),
            (
                "/absolute".to_string(),
                true,
                RouteSnapshotPathError::Absolute,
            ),
            (
                "bad\\path.md".to_string(),
                false,
                RouteSnapshotPathError::Backslash,
            ),
            (
                "bad//path.md".to_string(),
                false,
                RouteSnapshotPathError::EmptySegment,
            ),
            (
                "bad/./path.md".to_string(),
                false,
                RouteSnapshotPathError::DotSegment,
            ),
            (
                "bad/../path.md".to_string(),
                false,
                RouteSnapshotPathError::ParentSegment,
            ),
            (
                "bad/\u{7}.md".to_string(),
                false,
                RouteSnapshotPathError::ControlCharacter,
            ),
        ];

        for (path, is_directory, reason) in cases {
            let snapshot = if is_directory {
                ScannedSubtree {
                    files: vec![],
                    directories: vec![ScannedDirectory {
                        path: path.clone(),
                        meta: make_dir_meta("bad"),
                    }],
                }
            } else {
                ScannedSubtree {
                    files: vec![ScannedFile {
                        path: path.clone(),
                        meta: make_meta(NodeKind::Page),
                        extensions: EntryExtensions::default(),
                    }],
                    directories: vec![],
                }
            };

            let err = RouteCatalog::validate_snapshot(&snapshot).unwrap_err();
            assert_eq!(err, RouteCatalogError::InvalidSnapshotPath { path, reason });
        }
    }

    #[test]
    fn route_catalog_allows_empty_root_directory_snapshot_path() {
        let snapshot = ScannedSubtree {
            files: vec![],
            directories: vec![ScannedDirectory {
                path: String::new(),
                meta: make_dir_meta("root"),
            }],
        };

        RouteCatalog::validate_snapshot(&snapshot).unwrap();
    }

    #[test]
    fn display_path_uses_home_alias_for_root() {
        assert_eq!(
            display_path_for(&VirtualPath::from_absolute("/blog").unwrap()),
            "/blog"
        );
        assert_eq!(display_path_for(&VirtualPath::root()), "~");
    }

    #[test]
    fn canonicalize_user_path_understands_aliases_and_parent_segments() {
        let cwd = VirtualPath::from_absolute("/blog").unwrap();
        assert_eq!(
            canonicalize_user_path(&cwd, "../about.md")
                .unwrap()
                .as_str(),
            "/about.md"
        );
        assert_eq!(
            canonicalize_user_path(&cwd, "~/posts").unwrap().as_str(),
            "/posts"
        );
        assert_eq!(canonicalize_user_path(&cwd, "/db").unwrap().as_str(), "/db");
    }

    #[test]
    fn request_paths_are_surface_aware() {
        let path = VirtualPath::from_absolute("/blog/hello.md").unwrap();
        assert_eq!(
            request_path_for_canonical_path(&path, RouteSurface::Content),
            "/blog/hello"
        );
        assert_eq!(
            request_path_for_canonical_path(&path, RouteSurface::Shell),
            "/websh/blog/hello.md"
        );
    }

    #[test]
    fn route_request_runtime_overlay_matches_content_and_shell_surfaces() {
        for path in [
            "/.websh/state",
            "/.websh/state/session",
            "/websh/.websh/state/session",
        ] {
            assert!(
                route_request_targets_runtime_overlay(&RouteRequest::new(path)),
                "expected runtime overlay for {path}"
            );
        }

        for path in ["/", "/ledger", "/websh", "/.websh/mounts"] {
            assert!(
                !route_request_targets_runtime_overlay(&RouteRequest::new(path)),
                "expected content fs for {path}"
            );
        }
    }
}
