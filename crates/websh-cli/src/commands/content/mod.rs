use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::CliResult;
use crate::workflows::content::{self, DEFAULT_CONTENT_DIR};

#[derive(Args)]
pub(crate) struct ContentCommand {
    #[command(subcommand)]
    command: ContentSubcommand,
}

#[derive(Subcommand)]
enum ContentSubcommand {
    /// Refresh every node's sidecar (read YAML frontmatter, recompute
    /// derived fields like PDF dimensions / image size / content hashes),
    /// then regenerate `content/manifest.json` from the updated sidecars.
    /// Idempotent: re-running on unchanged content produces a byte-equal
    /// manifest.
    Manifest {
        #[arg(long, default_value = DEFAULT_CONTENT_DIR)]
        content_dir: PathBuf,
    },
    /// Regenerate `content/.websh/ledger.json` from primary content files.
    /// Implicitly refreshes sidecars and manifest first so the ledger
    /// reflects the current frontmatter.
    Ledger {
        #[arg(long, default_value = DEFAULT_CONTENT_DIR)]
        content_dir: PathBuf,
    },
}

pub(crate) fn run(root: &Path, command: ContentCommand) -> CliResult {
    match command.command {
        ContentSubcommand::Manifest { content_dir } => content::manifest(root, &content_dir),
        ContentSubcommand::Ledger { content_dir } => content::ledger(root, &content_dir),
    }
}
