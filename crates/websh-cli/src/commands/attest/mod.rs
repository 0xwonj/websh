use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use websh_site::PUBLIC_KEY_PATH;

use crate::CliResult;
use crate::workflows::attest::{
    AttestAllOptions, DEFAULT_GPG_SIGNER, DEFAULT_SIGNATURE_DIR, attest_all, verify,
};
use crate::workflows::content::DEFAULT_CONTENT_DIR;

mod subject;

#[derive(Args)]
pub(crate) struct AttestCommand {
    #[command(subcommand)]
    command: Option<AttestSubcommand>,
    /// Content root scanned by `websh-cli attest` when no subcommand is given.
    #[arg(long, default_value = DEFAULT_CONTENT_DIR)]
    content_dir: PathBuf,
    /// Public key used to verify automatic PGP signatures.
    #[arg(long, default_value = PUBLIC_KEY_PATH)]
    key: PathBuf,
    /// GPG key id/user id passed to `gpg --local-user`.
    #[arg(long, default_value = DEFAULT_GPG_SIGNER)]
    gpg_key: Option<String>,
    /// Local directory for generated subject messages and detached signatures.
    #[arg(long, default_value = DEFAULT_SIGNATURE_DIR)]
    signature_dir: PathBuf,
    /// Only regenerate subjects; do not call local gpg.
    #[arg(long)]
    no_sign: bool,
    /// Override issued_at for regenerated subjects.
    #[arg(long)]
    issued_at: Option<String>,
}

#[derive(Subcommand)]
enum AttestSubcommand {
    Subject(subject::SubjectCommand),
    Verify {
        #[arg(long)]
        route: Option<String>,
    },
}

pub(crate) fn run(root: &Path, command: AttestCommand) -> CliResult {
    let AttestCommand {
        command,
        content_dir,
        key,
        gpg_key,
        signature_dir,
        no_sign,
        issued_at,
    } = command;

    match command {
        Some(AttestSubcommand::Subject(command)) => subject::subject(root, command),
        Some(AttestSubcommand::Verify { route }) => verify(root, route),
        None => attest_all(
            root,
            AttestAllOptions {
                content_dir,
                key,
                gpg_key,
                signature_dir,
                no_sign,
                issued_at,
            },
        ),
    }
}
