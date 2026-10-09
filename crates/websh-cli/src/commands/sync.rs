use crate::{CliResult, workflows::sync};
use std::path::Path;

pub(crate) fn run(root: &Path) -> CliResult {
    let outcome = sync::sync(root)?;
    println!(
        "synced {} manifest entries and {} page subjects",
        outcome.entries, outcome.subjects
    );
    Ok(())
}
