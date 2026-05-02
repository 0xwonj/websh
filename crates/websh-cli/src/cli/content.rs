use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use super::CliResult;
use super::ledger::generate_content_ledger;
use super::manifest::{DEFAULT_CONTENT_DIR, build_manifest_from_sidecars, sync_content};

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
            let manifest = sync_content(root, &content_dir)?;
            println!(
                "manifest: {} entries -> {}/manifest.json (sidecars refreshed)",
                manifest.entries.len(),
                content_dir.display()
            );
            Ok(())
        }
        ContentSubcommand::Ledger { content_dir } => {
            // Refresh sidecars first so the ledger sees current frontmatter,
            // then re-fold the manifest after the ledger write so it picks
            // up the new ledger.json hash.
            sync_content(root, &content_dir)?;
            let ledger = generate_content_ledger(root, &content_dir)?;
            let manifest = build_manifest_from_sidecars(root, &content_dir)?;
            println!(
                "ledger: {} blocks -> {}/.websh/ledger.json; manifest: {} entries",
                ledger.block_count,
                content_dir.display(),
                manifest.entries.len()
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
