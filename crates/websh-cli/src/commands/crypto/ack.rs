use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::CliResult;
use crate::workflows::crypto::ack;

#[derive(Args)]
pub(crate) struct AckCommand {
    #[command(subcommand)]
    command: AckSubcommand,
}

#[derive(Subcommand)]
enum AckSubcommand {
    Init {
        #[arg(long)]
        force: bool,
    },
    Add {
        #[arg(long, conflicts_with = "private")]
        public: bool,
        #[arg(long)]
        private: bool,
        name: String,
    },
    Remove {
        #[arg(long)]
        keep_receipt: bool,
        name: String,
    },
    List,
    Build,
    Receipt {
        #[arg(long)]
        name: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Verify {
        #[arg(long, conflicts_with = "receipt")]
        name: Option<String>,
        #[arg(long)]
        receipt: Option<PathBuf>,
    },
}

pub(crate) fn run(root: &Path, command: AckCommand) -> CliResult {
    match command.command {
        AckSubcommand::Init { force } => ack::init(root, force),
        AckSubcommand::Add {
            public,
            private,
            name,
        } => ack::add(root, public, private, name),
        AckSubcommand::Remove { keep_receipt, name } => ack::remove_entry(root, name, keep_receipt),
        AckSubcommand::List => ack::list(root),
        AckSubcommand::Build => ack::build(root),
        AckSubcommand::Receipt { name, out } => ack::receipt(root, name, out),
        AckSubcommand::Verify { name, receipt } => ack::verify(root, name, receipt),
    }
}
