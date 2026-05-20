//! In-memory filesystem engine: globally-mounted tree, render intent,
//! routing, content reads, and change-merge.

mod content;
mod content_routes;
mod global_fs;
mod intent;
pub(crate) mod merge;
mod routing;
mod snapshot;
mod tree;

pub use crate::domain::{NodeKind, RendererKind, TrustLevel};

pub use content::{BackendRegistry, ContentReadError, public_read_url, read_bytes, read_text};
pub use content_routes::{
    attestation_route_for_node_path, bundle_variant_href, content_href_for_path,
    content_route_for_path,
};
pub use global_fs::{FsEngine, FsMutationError, GlobalFs, MountError};
pub use intent::{RenderIntent, build_render_intent};
pub use routing::{
    BundleVariantContext, ResolvedKind, RouteCatalog, RouteCatalogError, RouteCatalogNode,
    RouteFrame, RouteRequest, RouteResolution, RouteRole, RouteSnapshotPathError, RouteSurface,
    canonicalize_user_path, display_path_for, is_new_request_path, parent_request_path,
    request_path_for_canonical_path, resolve_route, resolve_route_with_catalog, route_cwd,
    route_request_targets_runtime_overlay, try_resolve_route,
};
