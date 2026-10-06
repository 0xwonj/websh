use crate::{
    CliResult,
    workflows::{publish, release},
};
use std::path::Path;

pub(crate) fn sign(root: &Path) -> CliResult {
    let manifest = release::sign(root)?;
    let metadata = manifest.release.expect("signed root manifest");
    println!(
        "signed content sequence {} (issued {})",
        metadata.sequence, metadata.issued_at
    );
    Ok(())
}

pub(crate) fn run(root: &Path, drafts: bool) -> CliResult {
    let publication = publish::publish(root, drafts)?;
    println!(
        "{} content snapshot {}",
        if publication.changed {
            "published"
        } else {
            "current"
        },
        publication.snapshot
    );
    if let Some(warning) = publication.visibility_warning {
        eprintln!("warning: {warning}");
    }
    Ok(())
}
