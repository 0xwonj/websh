//! Native content, attestation, and publishing commands.
use crate::{CliResult, commands, project::Project};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "websh-cli", about = "Build and verify the websh archive")]
struct Cli {
    /// Project directory; explicit external file arguments use the current directory.
    #[arg(long, global = true, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate current content and attestation subjects without signing.
    Sync,
    /// Verify current artifacts without changing any files.
    Check {
        /// Require a valid site signature on every current subject.
        #[arg(long)]
        require_signatures: bool,
    },
    /// Sign current subjects or exchange offline signing requests.
    Attest(commands::attest::AttestCommand),
    /// Generate a local draft manifest or import a draft into this project.
    Mempool(commands::mempool::MempoolCommand),
    /// Manage acknowledgement commitments and export proofs.
    Ack(commands::ack::AckCommand),
    /// Validate and publish the prebuilt dist directory.
    Deploy,
}

pub fn run() -> CliResult {
    let cli = Cli::parse();
    let project = Project::open(&cli.root)?;
    let root = project.root();
    match cli.command {
        Command::Sync => commands::sync::run(root),
        Command::Check { require_signatures } => commands::check::run(root, require_signatures),
        Command::Attest(command) => commands::attest::run(root, command),
        Command::Mempool(command) => commands::mempool::run(root, command),
        Command::Ack(command) => commands::ack::run(root, command),
        Command::Deploy => commands::deploy::run(root),
    }
}
