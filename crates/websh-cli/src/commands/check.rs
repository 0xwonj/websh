use crate::{CliResult, workflows::check};
use std::path::Path;

pub(crate) fn run(root: &Path, require_signatures: bool) -> CliResult {
    let outcome = check::check(root, require_signatures)?;
    println!(
        "checked root manifest ({}), {} page subjects ({} signed)",
        if outcome.root_signed {
            "signed"
        } else {
            "unissued"
        },
        outcome.subjects,
        outcome.signed,
    );
    Ok(())
}
