//! Mount-management CLI: bootstrap a new GitHub mount declaration plus
//! the empty manifest that the runtime expects to find on first scan.
//!
//! The CLI assumes the GitHub repo already exists (use `gh repo create` or
//! the web UI). It only handles the websh-specific setup: pushing the
//! initial `manifest.json`, writing the local mount declaration file, and
//! regenerating the bundle source manifest so the runtime sees the new
//! mount.

use std::path::Path;

use clap::{Args, Subcommand};

use crate::CliResult;

mod init;

use init::MountInit;

#[derive(Args)]
pub(crate) struct MountCommand {
    #[command(subcommand)]
    command: MountSubcommand,
}

#[derive(Subcommand)]
enum MountSubcommand {
    /// Bootstrap a GitHub mount: push an empty manifest to the remote repo
    /// (if missing), write the local mount declaration, regenerate the
    /// bundle source manifest. Idempotent — re-running is safe.
    Init(MountInit),
}

pub(crate) fn run(root: &Path, command: MountCommand) -> CliResult {
    match command.command {
        MountSubcommand::Init(init) => init::init_mount(root, init),
    }
}
