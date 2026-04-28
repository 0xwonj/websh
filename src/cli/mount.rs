//! Mount-management CLI: bootstrap a new GitHub mount declaration plus
//! the empty manifest that the runtime expects to find on first scan.
//!
//! The CLI assumes the GitHub repo already exists (use `gh repo create` or
//! the web UI). It only handles the websh-specific setup: pushing the
//! initial `manifest.json`, writing the local mount declaration file, and
//! regenerating the bundle source manifest so the runtime sees the new
//! mount.

use std::path::{Path, PathBuf};
use std::process::Command as Process;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use clap::{Args, Subcommand};
use serde::Serialize;

use super::CliResult;
use super::gh::{gh_succeeds, require_gh};
use super::manifest::{DEFAULT_CONTENT_DIR, generate_content_manifest};

const EMPTY_MANIFEST_BODY: &str = "{\"files\":[],\"directories\":[]}\n";

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

#[derive(Args)]
struct MountInit {
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

pub(crate) fn run(root: &Path, command: MountCommand) -> CliResult {
    match command.command {
        MountSubcommand::Init(init) => init_mount(root, init),
    }
}

#[derive(Serialize)]
struct MountFile {
    backend: &'static str,
    mount_at: String,
    repo: String,
    branch: String,
    root: String,
    name: String,
    writable: bool,
}

fn init_mount(root: &Path, init: MountInit) -> CliResult {
    require_gh()?;

    eprintln!("verify: github repo {}", init.repo);
    if !gh_succeeds(["api", &format!("repos/{}", init.repo), "--silent"])? {
        return Err(format!(
            "github repo {} not found — create it first with `gh repo create {}` \
             (or via the web UI), then re-run this command",
            init.repo, init.repo,
        )
        .into());
    }

    let manifest_path_in_repo = manifest_repo_path(&init.root);

    eprintln!(
        "verify: manifest at {}@{}/{}",
        init.repo, init.branch, manifest_path_in_repo
    );
    let manifest_present = gh_succeeds([
        "api",
        &format!(
            "repos/{}/contents/{}?ref={}",
            init.repo, manifest_path_in_repo, init.branch
        ),
        "--silent",
    ])?;
    if manifest_present {
        eprintln!("manifest: already present, skipping bootstrap");
    } else {
        eprintln!("manifest: pushing empty bootstrap manifest");
        push_empty_manifest(&init.repo, &init.branch, &manifest_path_in_repo)?;
    }

    let mount_file = MountFile {
        backend: "github",
        mount_at: init.mount_at.clone(),
        repo: init.repo.clone(),
        branch: init.branch.clone(),
        root: init.root.clone(),
        name: init.name.clone(),
        writable: init.writable,
    };
    let mount_decl_path = mount_declaration_path(root, &init.name);
    if let Some(parent) = mount_decl_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_string_pretty(&mount_file)?;
    std::fs::write(&mount_decl_path, format!("{body}\n"))?;
    eprintln!("local: wrote {}", mount_decl_path.display());

    let bundle = generate_content_manifest(root, Path::new(DEFAULT_CONTENT_DIR))?;
    eprintln!(
        "bundle manifest regenerated: {} files / {} directories",
        bundle.files.len(),
        bundle.directories.len()
    );

    eprintln!(
        "\nmount '{}' is ready at {}. \
         Restart `trunk serve` (or rebuild) so the runtime picks it up.",
        init.name, init.mount_at,
    );
    Ok(())
}

fn manifest_repo_path(root: &str) -> String {
    let trimmed = root.trim_matches('/');
    if trimmed.is_empty() {
        "manifest.json".to_string()
    } else {
        format!("{trimmed}/manifest.json")
    }
}

fn mount_declaration_path(root: &Path, name: &str) -> PathBuf {
    root.join(DEFAULT_CONTENT_DIR)
        .join(".websh")
        .join("mounts")
        .join(format!("{name}.mount.json"))
}

fn push_empty_manifest(repo: &str, branch: &str, path_in_repo: &str) -> CliResult {
    let encoded = BASE64_STANDARD.encode(EMPTY_MANIFEST_BODY);
    let url = format!("repos/{repo}/contents/{path_in_repo}");
    let status = Process::new("gh")
        .args([
            "api",
            &url,
            "-X",
            "PUT",
            "-f",
            "message=bootstrap: empty manifest",
            "-f",
            &format!("content={encoded}"),
            "-f",
            &format!("branch={branch}"),
        ])
        .status()?;
    if !status.success() {
        return Err(format!(
            "gh api failed pushing bootstrap manifest to {repo}@{branch}/{path_in_repo}; \
             check that `gh auth status` shows an authenticated account with \
             contents:write on this repository"
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_repo_path_at_repo_root() {
        assert_eq!(manifest_repo_path(""), "manifest.json");
        assert_eq!(manifest_repo_path("/"), "manifest.json");
    }

    #[test]
    fn manifest_repo_path_with_subdir() {
        assert_eq!(manifest_repo_path("content"), "content/manifest.json");
        assert_eq!(manifest_repo_path("/content/"), "content/manifest.json");
    }

    #[test]
    fn mount_declaration_path_uses_websh_mounts_dir() {
        let p = mount_declaration_path(Path::new("/tmp/proj"), "mempool");
        assert!(p.ends_with("content/.websh/mounts/mempool.mount.json"));
    }
}
