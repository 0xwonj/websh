use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use websh_site::{APP_NAME, PUBLIC_KEY_PATH};

use crate::CliResult;
use crate::workflows::attest::subject::{SubjectAction, subject as run_subject_action};

#[derive(Args)]
pub(super) struct SubjectCommand {
    #[command(subcommand)]
    command: SubjectSubcommand,
}

#[derive(Subcommand)]
enum SubjectSubcommand {
    Set {
        #[arg(long)]
        route: String,
        #[arg(long)]
        kind: String,
        #[arg(long = "content", num_args = 1..)]
        content: Vec<PathBuf>,
        #[arg(long)]
        issued_at: Option<String>,
    },
    Message {
        #[arg(long)]
        route: String,
    },
    PgpImport {
        #[arg(long)]
        route: String,
        #[arg(long)]
        signature: PathBuf,
        #[arg(long, default_value = PUBLIC_KEY_PATH)]
        key: PathBuf,
        #[arg(long)]
        signer: Option<String>,
    },
    EthImport {
        #[arg(long)]
        route: String,
        #[arg(long)]
        address: String,
        #[arg(long)]
        signature: String,
        #[arg(long, default_value = APP_NAME)]
        signer: String,
    },
}

pub(super) fn subject(root: &Path, command: SubjectCommand) -> CliResult {
    let action = match command.command {
        SubjectSubcommand::Set {
            route,
            kind,
            content,
            issued_at,
        } => SubjectAction::Set {
            route,
            kind,
            content_paths: content,
            issued_at,
        },
        SubjectSubcommand::Message { route } => SubjectAction::Message { route },
        SubjectSubcommand::PgpImport {
            route,
            signature,
            key,
            signer,
        } => SubjectAction::PgpImport {
            route,
            signature,
            key,
            signer,
        },
        SubjectSubcommand::EthImport {
            route,
            address,
            signature,
            signer,
        } => SubjectAction::EthImport {
            route,
            address,
            signature,
            signer,
        },
    };
    run_subject_action(root, action)
}
