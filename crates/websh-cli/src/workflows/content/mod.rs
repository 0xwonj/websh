pub(crate) mod files;
pub(crate) mod frontmatter;
pub(crate) mod ledger;
pub(crate) mod manifest;
pub(crate) mod media;
pub(crate) mod modified_at;
pub(crate) mod sidecar;

pub(crate) use files::{
    artifact_path, build_content_files, collect_files_recursive, discover_bundle_content_units,
    discover_directory_content_units, kind_for_content_path, path_is_inside_bundle,
    path_is_inside_directory_unit, relative_path_from, resolve_path, route_for_content_path,
    should_skip_primary_content_file,
};
pub(crate) use ledger::generate_content_ledger;
pub(crate) use manifest::{DEFAULT_CONTENT_DIR, build_manifest_from_sidecars, sync_content};
pub(crate) use sidecar::matching_file_sidecar;

use crate::CliResult;
use std::path::Path;

pub(crate) fn manifest(root: &Path, content_dir: &Path) -> CliResult {
    let manifest = sync_content(root, content_dir)?;
    println!(
        "manifest: {} entries -> {}/manifest.json (sidecars refreshed)",
        manifest.entries.len(),
        content_dir.display()
    );
    Ok(())
}

pub(crate) fn ledger(root: &Path, content_dir: &Path) -> CliResult {
    sync_content(root, content_dir)?;
    let ledger = generate_content_ledger(root, content_dir)?;
    let manifest = build_manifest_from_sidecars(root, content_dir)?;
    println!(
        "ledger: {} blocks -> {}/.websh/ledger.json; manifest: {} entries",
        ledger.block_count,
        content_dir.display(),
        manifest.entries.len()
    );
    Ok(())
}
