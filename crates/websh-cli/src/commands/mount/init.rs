use std::path::Path;

use clap::Args;

use crate::CliResult;
use crate::workflows::mount::init::{
    ManifestBootstrapStatus, MountInitOptions, init_mount as run_init_mount,
};

#[derive(Args)]
pub(super) struct MountInit {
    /// Logical mount name. Used for the declaration filename
    /// (`content/.websh/mounts/<name>.mount.json`).
    #[arg(long)]
    name: String,
    /// `owner/name` of the GitHub repo to mount.
    #[arg(long)]
    repo: String,
    /// Canonical mount path (e.g., `/mempool`).
    #[arg(long = "mount-at")]
    mount_at: String,
    /// Branch to mount.
    #[arg(long, default_value = "main")]
    branch: String,
    /// Sub-path within the repo whose subtree is exposed at mount root.
    /// Empty string means the repo root itself.
    #[arg(long, default_value = "")]
    root: String,
    /// Mark the mount as writable (allow wasm-driven commits).
    #[arg(long, default_value_t = false)]
    writable: bool,
}

pub(super) fn init_mount(root: &Path, init: MountInit) -> CliResult {
    let options = MountInitOptions {
        name: init.name,
        repo: init.repo,
        mount_at: init.mount_at,
        branch: init.branch,
        root: init.root,
        writable: init.writable,
    };
    let outcome = run_init_mount(root, options)?;

    eprintln!("verify: github repo {}", outcome.repo);
    eprintln!(
        "verify: manifest at {}@{}/{}",
        outcome.repo, outcome.branch, outcome.manifest_path_in_repo
    );
    match outcome.manifest_status {
        ManifestBootstrapStatus::AlreadyPresent => {
            eprintln!("manifest: already present, skipping bootstrap");
        }
        ManifestBootstrapStatus::Created => {
            eprintln!("manifest: pushed empty bootstrap manifest");
        }
    }
    eprintln!("local: wrote {}", outcome.mount_decl_path.display());
    eprintln!(
        "bundle manifest regenerated: {} entries",
        outcome.bundle_entries
    );
    eprintln!(
        "\nmount '{}' is ready at {}. \
         Restart `trunk serve` (or rebuild) so the runtime picks it up.",
        outcome.mount_name, outcome.mount_at,
    );

    Ok(())
}
