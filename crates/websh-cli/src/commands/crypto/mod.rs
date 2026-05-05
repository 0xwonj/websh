use std::path::Path;

use clap::{Args, Subcommand};

use crate::CliResult;

mod ack;
mod pgp;
mod verify_all;

use ack::AckCommand;
use pgp::PgpCommand;
use verify_all::verify_all;

#[derive(Args)]
pub(crate) struct CryptoCommand {
    #[command(subcommand)]
    command: CryptoSubcommand,
}

#[derive(Subcommand)]
enum CryptoSubcommand {
    Ack(AckCommand),
    Pgp(PgpCommand),
    Verify,
}

pub(crate) fn run(root: &Path, command: CryptoCommand) -> CliResult {
    match command.command {
        CryptoSubcommand::Ack(command) => ack::run(root, command),
        CryptoSubcommand::Pgp(command) => pgp::run(root, command),
        CryptoSubcommand::Verify => verify_all(root),
    }
}
