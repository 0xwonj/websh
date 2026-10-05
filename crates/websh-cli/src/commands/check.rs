use crate::{CliResult, workflows::check};
use std::path::Path;

pub(crate) fn run(root: &Path, require_signatures: bool) -> CliResult {
    let outcome = check::check(root, require_signatures)?;
    println!(
        "checked {} subjects: {} signed, {} pending",
        outcome.subjects,
        outcome.signed,
        outcome.subjects - outcome.signed
    );
    Ok(())
}
