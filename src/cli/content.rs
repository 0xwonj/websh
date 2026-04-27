use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use super::CliResult;
use super::ledger::generate_content_ledger;
use super::manifest::{DEFAULT_CONTENT_DIR, generate_content_manifest};

#[derive(Args)]
pub(crate) struct ContentCommand {
    #[command(subcommand)]
    command: ContentSubcommand,
}

#[derive(Subcommand)]
enum ContentSubcommand {
    /// Regenerate content/manifest.json from the content directory.
    Manifest {
        #[arg(long, default_value = DEFAULT_CONTENT_DIR)]
        content_dir: PathBuf,
    },
    /// Regenerate content/.websh/ledger.json from primary content files.
    Ledger {
        #[arg(long, default_value = DEFAULT_CONTENT_DIR)]
        content_dir: PathBuf,
    },
    New {
        #[arg(long)]
        kind: String,
        #[arg(long)]
        title: String,
    },
    Render {
        path: PathBuf,
    },
    Validate {
        path: PathBuf,
    },
    Publish {
        path: PathBuf,
    },
}

pub(crate) fn run(root: &Path, command: ContentCommand) -> CliResult {
    match command.command {
        ContentSubcommand::Manifest { content_dir } => {
            let manifest = generate_content_manifest(root, &content_dir)?;
            println!(
                "manifest: {} files, {} directories -> {}/manifest.json",
                manifest.files.len(),
                manifest.directories.len(),
                content_dir.display()
            );
            Ok(())
        }
        ContentSubcommand::Ledger { content_dir } => {
            let ledger = generate_content_ledger(root, &content_dir)?;
            let manifest = generate_content_manifest(root, &content_dir)?;
            println!(
                "ledger: {} entries -> {}/.websh/ledger.json; manifest: {} files",
                ledger.entry_count,
                content_dir.display(),
                manifest.files.len()
            );
            Ok(())
        }
        ContentSubcommand::New { kind, title } => {
            Err(format!("content new is reserved for later: kind={kind}, title={title}").into())
        }
        ContentSubcommand::Render { path } => {
            Err(format!("content render is reserved for later: {}", path.display()).into())
        }
        ContentSubcommand::Validate { path } => {
            Err(format!("content validate is reserved for later: {}", path.display()).into())
        }
        ContentSubcommand::Publish { path } => {
            Err(format!("content publish is reserved for later: {}", path.display()).into())
        }
    }
}
