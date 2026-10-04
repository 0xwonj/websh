//! Native CLI for local websh project maintenance.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::CliResult;
use crate::commands::{attest, content, crypto, deploy, mempool, mount};

#[derive(Parser)]
#[command(name = "websh-cli")]
#[command(about = "Native maintenance CLI for websh")]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Attest(attest::AttestCommand),
    Crypto(crypto::CryptoCommand),
    Content(content::ContentCommand),
    Deploy(deploy::DeployCommand),
    Mempool(mempool::MempoolCommand),
    Mount(mount::MountCommand),
}

pub fn run() -> CliResult {
    let cli = Cli::parse();
    let root = cli.root;
    match cli.command {
        Command::Attest(command) => attest::run(&root, command),
        Command::Crypto(command) => crypto::run(&root, command),
        Command::Content(command) => content::run(&root, command),
        Command::Deploy(command) => deploy::run(&root, command),
        Command::Mempool(command) => mempool::run(&root, command),
        Command::Mount(command) => mount::run(&root, command),
    }
}
