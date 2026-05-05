use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};

use crate::CliResult;

mod dotenv;
mod pinata;
mod trunk;

const DEFAULT_DIST_DIR: &str = "dist";
const DEFAULT_GATEWAY: &str = "https://amethyst-decisive-whitefish-145.mypinata.cloud";
const DEFAULT_ENS_RECORDS_URL: &str = "https://app.ens.domains/wonjae.eth?tab=records";

#[derive(Args)]
pub(crate) struct DeployCommand {
    #[command(subcommand)]
    command: DeploySubcommand,
}

#[derive(Subcommand)]
enum DeploySubcommand {
    /// Build the site and upload dist/ to Pinata IPFS.
    Pinata {
        /// Directory to upload after the release build.
        #[arg(long, default_value = DEFAULT_DIST_DIR)]
        dist_dir: PathBuf,

        /// Pinata upload name. Defaults to websh-<unix timestamp>.
        #[arg(long)]
        name: Option<String>,

        /// Skip the release build and upload the existing dist directory.
        /// Note: this also skips the attestation refresh that the trunk
        /// pre-build hook normally performs. Run
        /// `websh-cli attest build --force` first if you need a fresh
        /// attestation artifact without a rebuild.
        #[arg(long)]
        no_build: bool,

        /// Build without signing newly-changed subjects. Propagated to the
        /// trunk pre-build hook via `WEBSH_NO_SIGN=1`; pending subjects
        /// remain unsigned in the uploaded dist.
        #[arg(long)]
        no_sign: bool,

        /// Gateway base URL printed after upload.
        #[arg(long, default_value = DEFAULT_GATEWAY)]
        gateway: String,

        /// ENS records page printed after upload.
        #[arg(long, default_value = DEFAULT_ENS_RECORDS_URL)]
        ens_url: String,
    },
}

pub(crate) fn run(root: &Path, command: DeployCommand) -> CliResult {
    match command.command {
        DeploySubcommand::Pinata {
            dist_dir,
            name,
            no_build,
            no_sign,
            gateway,
            ens_url,
        } => pinata::pinata(root, dist_dir, name, no_build, no_sign, gateway, ens_url),
    }
}
