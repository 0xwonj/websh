use std::path::Path;

use crate::CliResult;
use crate::workflows::mempool::list::list_entries;

pub(super) fn list(root: &Path) -> CliResult {
    let outcome = list_entries(root)?;

    println!("{} @ {}:", outcome.repo, outcome.branch);
    if outcome.entries.is_empty() {
        println!("0 pending entries");
    } else {
        for entry in &outcome.entries {
            println!(
                "  {:6} {:32} {:14} {}",
                entry.status, entry.path, entry.size_hint, entry.modified
            );
        }
        println!("{} pending entries", outcome.entries.len());
    }

    for warning in outcome.warnings {
        eprintln!("warning: {warning}");
    }

    Ok(())
}
