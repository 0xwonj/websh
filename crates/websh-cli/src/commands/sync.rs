use crate::{CliResult, workflows::sync};
use std::path::Path;

pub(crate) fn run(root: &Path) -> CliResult {
    let outcome = sync::sync(root)?;
    println!(
        "synced {} manifest entries, {} ledger blocks, {} subjects",
        outcome.entries, outcome.blocks, outcome.subjects
    );
    Ok(())
}
