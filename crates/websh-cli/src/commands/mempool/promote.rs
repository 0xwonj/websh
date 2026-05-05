use std::path::Path;

use clap::Args;

use crate::CliResult;
use crate::workflows::mempool::promote::{InteractionMode, PromoteOptions, promote_entry};

#[derive(Args)]
pub(super) struct PromoteArgs {
    /// Repo-relative path inside the mempool repo (e.g., `writing/test.md`).
    #[arg(long)]
    path: String,
    /// Skip the post-commit drop from mempool (verify locally first,
    /// run `websh-cli mempool drop` later).
    #[arg(long, default_value_t = false)]
    keep_remote: bool,
    /// Skip attestation regeneration (useful when GPG is not configured).
    #[arg(long, default_value_t = false)]
    no_attest: bool,
    /// Continue even when the current git branch differs from the deploy branch.
    #[arg(long, default_value_t = false)]
    allow_branch_mismatch: bool,
}

pub(super) fn promote(root: &Path, args: PromoteArgs) -> CliResult {
    promote_entry(
        root,
        PromoteOptions {
            path: args.path,
            keep_remote: args.keep_remote,
            no_attest: args.no_attest,
            allow_branch_mismatch: args.allow_branch_mismatch,
            interaction: InteractionMode::detect(),
        },
    )
}
