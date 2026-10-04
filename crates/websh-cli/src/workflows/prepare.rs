use std::path::Path;

use super::{attest, content};
use crate::CliResult;

/// The sole content-generating Trunk hook. Development only needs sidecars
/// and a manifest; release builds also refresh the ledger and attestations.
pub(crate) fn prepare(root: &Path) -> CliResult {
    if std::env::var("TRUNK_PROFILE").as_deref() == Ok("release") {
        attest::run_default(root, false)
    } else {
        content::manifest(root, Path::new(content::DEFAULT_CONTENT_DIR))
    }
}
