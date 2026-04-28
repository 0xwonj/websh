//! Mempool CLI: list pending entries, promote a draft to the canonical
//! chain (atomic local commit on the bundle source), drop a draft from
//! the mempool repo. Replaces Phase 3's browser-side promote per the
//! Phase 5 design (`docs/superpowers/specs/2026-04-28-mempool-phase5-design.md`).

use std::path::Path;

use clap::{Args, Subcommand};

use super::CliResult;

#[derive(Args)]
pub(crate) struct MempoolCommand {
    #[command(subcommand)]
    command: MempoolSubcommand,
}

#[derive(Subcommand)]
enum MempoolSubcommand {
    /// List pending entries in the mempool repo.
    List,
    /// Promote a mempool entry to the canonical chain via a single local
    /// git commit on the bundle source. Optionally also drops the entry
    /// from the mempool repo (`--drop-remote`).
    Promote(PromoteArgs),
    /// Delete an entry from the mempool repo.
    Drop(DropArgs),
}

#[derive(Args)]
struct PromoteArgs {
    /// Repo-relative path inside the mempool repo (e.g., `writing/test.md`).
    #[arg(long)]
    path: String,
    /// After the local commit, also delete the entry from the mempool repo.
    #[arg(long, default_value_t = false)]
    drop_remote: bool,
    /// Skip attestation regeneration (useful when GPG is not configured).
    #[arg(long, default_value_t = false)]
    no_attest: bool,
    /// Allow promote when `content/` has uncommitted changes.
    #[arg(long, default_value_t = false)]
    allow_dirty: bool,
}

#[derive(Args)]
struct DropArgs {
    /// Repo-relative path inside the mempool repo.
    #[arg(long)]
    path: String,
    /// Succeed silently if the entry no longer exists.
    #[arg(long, default_value_t = false)]
    if_exists: bool,
}

pub(crate) fn run(root: &Path, command: MempoolCommand) -> CliResult {
    match command.command {
        MempoolSubcommand::List => list(root),
        MempoolSubcommand::Promote(args) => promote(root, args),
        MempoolSubcommand::Drop(args) => drop_entry(root, args),
    }
}

fn list(_root: &Path) -> CliResult {
    Err("mempool list: implemented in Task 3".into())
}

fn promote(_root: &Path, _args: PromoteArgs) -> CliResult {
    Err("mempool promote: implemented in Task 4".into())
}

fn drop_entry(_root: &Path, _args: DropArgs) -> CliResult {
    Err("mempool drop: implemented in Task 5".into())
}
