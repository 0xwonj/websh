use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use websh_site::{APP_NAME, PUBLIC_KEY_PATH};

use crate::CliResult;
use crate::workflows::crypto::pgp;

#[derive(Args)]
pub(crate) struct PgpCommand {
    #[command(subcommand)]
    command: PgpSubcommand,
}

#[derive(Subcommand)]
enum PgpSubcommand {
    Import {
        #[arg(long, default_value = PUBLIC_KEY_PATH)]
        key: PathBuf,
        #[arg(long, default_value = APP_NAME)]
        ens: String,
        #[arg(long, default_value = "")]
        address: String,
    },
    Verify,
}

pub(crate) fn run(root: &Path, command: PgpCommand) -> CliResult {
    match command.command {
        PgpSubcommand::Import { key, ens, address } => pgp::import(root, key, ens, address),
        PgpSubcommand::Verify => {
            pgp::verify_identity(root)?;
            println!("pgp: ok");
            Ok(())
        }
    }
}
