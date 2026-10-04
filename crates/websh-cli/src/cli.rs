//! Native CLI for local websh project maintenance.

use std::path::PathBuf;

use anyhow::Context;
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
    /// Prepare content for the current Trunk profile.
    Prepare,
    Attest(attest::AttestCommand),
    Crypto(crypto::CryptoCommand),
    Content(content::ContentCommand),
    Deploy(deploy::DeployCommand),
    Mempool(mempool::MempoolCommand),
    Mount(mount::MountCommand),
}

pub fn run() -> CliResult {
    let cli = Cli::parse();
    let root = std::path::absolute(cli.root).context("resolve project root")?;
    match cli.command {
        Command::Prepare => crate::workflows::prepare::prepare(&root),
        Command::Attest(command) => attest::run(&root, command),
        Command::Crypto(command) => crypto::run(&root, command),
        Command::Content(command) => content::run(&root, command),
        Command::Deploy(command) => deploy::run(&root, command),
        Command::Mempool(command) => mempool::run(&root, command),
        Command::Mount(command) => mount::run(&root, command),
    }
}
