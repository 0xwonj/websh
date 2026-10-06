//! Local draft preparation. Editing and publishing remain ordinary Git operations.

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::CliResult;

#[derive(Args)]
pub(crate) struct MempoolCommand {
    #[command(subcommand)]
    command: MempoolSubcommand,
}

#[derive(Subcommand)]
enum MempoolSubcommand {
    /// Generate manifest.json from an authored mempool checkout; does not publish.
    Sync {
        /// Checkout path, relative to the current working directory.
        checkout: PathBuf,
    },
    /// Generate, commit, and push an independently published draft snapshot.
    Publish { checkout: PathBuf },
    /// Copy a draft into canonical content; leaves the draft and Git index unchanged.
    Import {
        /// Markdown source, relative to the current working directory.
        file: PathBuf,
        /// Destination beneath content/, e.g. writing/example.md.
        /// Defaults to the source file's category directory and filename.
        #[arg(long)]
        to: Option<String>,
    },
}

pub(crate) fn run(root: &Path, command: MempoolCommand) -> CliResult {
    match command.command {
        MempoolSubcommand::Publish { checkout } => crate::commands::publish::run(&checkout, true),
        MempoolSubcommand::Sync { checkout } => {
            let count = crate::workflows::mempool::manifest::sync(&checkout)?;
            println!(
                "mempool sync: {count} entries -> {}",
                checkout.join("manifest.json").display()
            );
            Ok(())
        }
        MempoolSubcommand::Import { file, to } => {
            let path = crate::workflows::mempool::import::import(root, &file, to.as_deref())?;
            println!("mempool import: {} -> {}", file.display(), path.display());
            println!("Run `websh-cli sync` to refresh generated content, then review and commit.");
            Ok(())
        }
    }
}
