use std::path::Path;

use clap::Args;

use crate::CliResult;
use crate::workflows::mempool::drop::drop_entry as run_drop;

#[derive(Args)]
pub(super) struct DropArgs {
    /// Repo-relative path inside the mempool repo.
    #[arg(long)]
    path: String,
    /// Succeed silently if the entry no longer exists.
    #[arg(long, default_value_t = false)]
    if_exists: bool,
}

pub(super) fn drop_entry(root: &Path, args: DropArgs) -> CliResult {
    run_drop(root, &args.path, args.if_exists)
}
