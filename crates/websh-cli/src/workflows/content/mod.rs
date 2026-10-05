mod files;
pub(crate) mod frontmatter;
mod media;
mod metadata;
mod snapshot;

pub(crate) use files::collect_files_recursive;
use files::{kind_for_content_path, route_for_content_path};
pub(crate) use snapshot::{ContentSnapshot, artifact_bytes};

pub(crate) const DEFAULT_CONTENT_DIR: &str = "content";
