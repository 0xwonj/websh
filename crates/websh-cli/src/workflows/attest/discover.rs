use std::path::Path;

use websh_core::domain::NodeKind;

use crate::CliResult;
use crate::workflows::content::matching_file_sidecar;
use crate::workflows::content::{
    collect_files_recursive, kind_for_content_path, relative_path_from, resolve_path,
    route_for_content_path, should_skip_primary_content_file,
};

use super::subject::{SubjectKind, SubjectSpec};

pub(super) fn discover_subject_specs(
    root: &Path,
    content_dir: &Path,
) -> CliResult<Vec<SubjectSpec>> {
    let content_root = resolve_path(root, content_dir);
    let mut files = Vec::new();
    collect_files_recursive(&content_root, &mut files)?;

    let mut specs = Vec::new();
    for file_path in files {
        let rel_path = relative_path_from(&content_root, &file_path)?;
        if should_skip_primary_content_file(&rel_path) {
            continue;
        }
        let mut content_paths = vec![file_path.clone()];
        if let Some(sidecar) = matching_file_sidecar(&content_root, &rel_path) {
            content_paths.push(sidecar);
        }
        let kind = subject_kind_for_node_kind(kind_for_content_path(&rel_path));
        specs.push(SubjectSpec {
            route: route_for_content_path(&rel_path),
            kind,
            content_paths,
        });
    }
    specs.sort_by(|left, right| left.route.cmp(&right.route));
    Ok(specs)
}

fn subject_kind_for_node_kind(kind: NodeKind) -> SubjectKind {
    match kind {
        NodeKind::Page => SubjectKind::Page,
        _ => SubjectKind::Document,
    }
}
