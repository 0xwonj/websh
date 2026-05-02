use std::path::Path;

use clap::{Args, Subcommand};

use websh_core::crypto::ack::{ACK_ARTIFACT_PATH, AckArtifact, short_hash};
use websh_core::crypto::pgp::IDENTITY_PATH;

use super::CliResult;
use super::ack::{self, AckCommand};
use super::io::read_json;
use super::pgp::{self, PgpCommand};

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

fn verify_all(root: &Path) -> CliResult {
    let artifact = read_json::<AckArtifact>(&root.join(ACK_ARTIFACT_PATH))?;
    artifact.validate()?;
    println!("ack: ok {}", short_hash(&artifact.combined_root));

    let identity_path = root.join(IDENTITY_PATH);
    if identity_path.exists() {
        pgp::verify_identity(root)?;
        println!("pgp: ok");
    } else {
        println!("pgp: skipped ({IDENTITY_PATH} missing)");
    }
    Ok(())
}
