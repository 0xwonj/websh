//! Mempool CLI: list pending entries, promote a draft to the canonical
//! chain (atomic local commit on the bundle source), drop a draft from
//! the mempool repo.

use std::path::Path;

use clap::{Args, Subcommand};

use crate::CliResult;

mod add;
mod drop;
mod list;
mod promote;

use add::AddArgs;
use drop::DropArgs;
use promote::PromoteArgs;

#[derive(Args)]
pub(crate) struct MempoolCommand {
    #[command(subcommand)]
    command: MempoolSubcommand,
}

#[derive(Subcommand)]
enum MempoolSubcommand {
    /// List pending entries in the mempool repo.
    List,
    /// Create a new mempool entry by committing it to the mempool repo.
    /// CRUD-symmetry counterpart to promote/drop; lets terminal-only or
    /// scripted workflows author drafts without opening the browser.
    Add(AddArgs),
    /// Promote a mempool entry to the canonical chain via a single local
    /// git commit on the bundle source. Optionally also drops the entry
    /// from the mempool repo (`--drop-remote`).
    Promote(PromoteArgs),
    /// Delete an entry from the mempool repo.
    Drop(DropArgs),
}

pub(crate) fn run(root: &Path, command: MempoolCommand) -> CliResult {
    match command.command {
        MempoolSubcommand::List => list::list(root),
        MempoolSubcommand::Add(args) => add::add(root, args),
        MempoolSubcommand::Promote(args) => promote::promote(root, args),
        MempoolSubcommand::Drop(args) => drop::drop_entry(root, args),
    }
}
