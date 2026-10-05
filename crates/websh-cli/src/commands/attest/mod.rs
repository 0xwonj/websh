use crate::{CliResult, workflows::attest};
use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Args)]
pub(crate) struct AttestCommand {
    #[command(subcommand)]
    command: AttestSubcommand,
}

#[derive(Subcommand)]
enum AttestSubcommand {
    /// Sign missing current subjects with the site PGP key.
    Sign { route: Option<String> },
    /// Print exact signing bytes; redirect stdout to a request file.
    Message { route: String },
    /// Verify and import a signature of an exported request.
    Import {
        #[command(subcommand)]
        signature: Import,
    },
}

#[derive(Subcommand)]
enum Import {
    /// Import an armored detached signature from the site PGP key.
    Pgp {
        route: String,
        #[arg(long)]
        message: PathBuf,
        #[arg(long)]
        signature: PathBuf,
    },
    /// Import an additional EIP-191 signature (not a replacement for site PGP).
    Ethereum {
        route: String,
        #[arg(long)]
        message: PathBuf,
        #[arg(long)]
        address: String,
        #[arg(long)]
        signature: String,
    },
}

pub(crate) fn run(root: &Path, command: AttestCommand) -> CliResult {
    match command.command {
        AttestSubcommand::Sign { route } => {
            println!("signed {} subjects", attest::sign(root, route.as_deref())?)
        }
        AttestSubcommand::Message { route } => print!("{}", attest::message(root, &route)?),
        AttestSubcommand::Import {
            signature:
                Import::Pgp {
                    route,
                    message,
                    signature,
                },
        } => {
            attest::import_pgp(root, &route, &message, &signature)?;
            println!("imported PGP signature for {route}");
        }
        AttestSubcommand::Import {
            signature:
                Import::Ethereum {
                    route,
                    message,
                    address,
                    signature,
                },
        } => {
            attest::import_ethereum(root, &route, &message, &address, &signature)?;
            println!("imported Ethereum signature for {route}");
        }
    }
    Ok(())
}
