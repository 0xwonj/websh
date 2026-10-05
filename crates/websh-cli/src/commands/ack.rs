use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use websh_core::crypto::ack::{AckArtifact, AckEntryMode, short_hash};

use crate::CliResult;
use crate::workflows::ack;

#[derive(Args)]
pub(crate) struct AckCommand {
    #[command(subcommand)]
    command: AckSubcommand,
}

#[derive(Subcommand)]
enum AckSubcommand {
    /// Add an acknowledgement and update its public commitment.
    Add {
        name: String,
        #[arg(long, value_enum)]
        visibility: Visibility,
    },
    /// Remove an acknowledgement and update its public commitment.
    Remove { name: String },
    /// List names from the local private source.
    List,
    /// Export a private entry's proof against the current published commitment.
    Receipt {
        name: String,
        /// Output file, relative to the invocation directory.
        #[arg(long)]
        out: PathBuf,
    },
    /// Verify an exported receipt against the current published commitment.
    Verify { file: PathBuf },
}

#[derive(Clone, Copy, ValueEnum)]
enum Visibility {
    Public,
    Private,
}

pub(crate) fn run(root: &Path, command: AckCommand) -> CliResult {
    match command.command {
        AckSubcommand::Add { name, visibility } => {
            let mode = match visibility {
                Visibility::Public => AckEntryMode::Public,
                Visibility::Private => AckEntryMode::Private,
            };
            print_update(&ack::add(root, name, mode)?);
        }
        AckSubcommand::Remove { name } => print_update(&ack::remove(root, &name)?),
        AckSubcommand::List => {
            for entry in ack::list(root)? {
                let mode = match entry.mode {
                    AckEntryMode::Public => "public",
                    AckEntryMode::Private => "private",
                };
                println!("{mode}\t{}", entry.name);
            }
        }
        AckSubcommand::Receipt { name, out } => {
            let receipt = ack::receipt(root, &name, &out)?;
            println!(
                "receipt: {} ({})",
                out.display(),
                short_hash(&receipt.combined_root)
            );
        }
        AckSubcommand::Verify { file } => {
            let verification = ack::verify(root, &file)?;
            println!("receipt: ok {}", short_hash(&verification.combined_root));
        }
    }
    Ok(())
}

fn print_update(artifact: &AckArtifact) {
    println!(
        "ack: {} entries, {}",
        artifact.count(),
        short_hash(&artifact.combined_root)
    );
}
