//! In-memory filesystem engine: globally-mounted tree, render intent,
//! routing, content reads, and change-merge.

#![allow(dead_code, unused_imports)]

mod content;
mod global_fs;
mod intent;
pub mod merge;
mod routing;

pub use crate::domain::{NodeKind, RendererKind, TrustLevel};

pub use content::{BackendRegistry, read_bytes, read_text};
pub use global_fs::{FsEngine, GlobalFs, MountError};
pub use intent::{RenderIntent, build_render_intent};
pub use routing::{
    ResolvedKind, RouteFrame, RouteRequest, RouteResolution, RouteSurface, canonicalize_user_path,
    display_path_for, is_new_request_path, parent_request_path, push_request_path,
    replace_request_path, request_path_for_canonical_path, resolve_route, route_cwd,
};
