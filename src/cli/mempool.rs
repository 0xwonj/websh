//! Mempool CLI: list pending entries, promote a draft to the canonical
//! chain (atomic local commit on the bundle source), drop a draft from
//! the mempool repo. Replaces Phase 3's browser-side promote per the
//! Phase 5 design (`docs/superpowers/specs/2026-04-28-mempool-phase5-design.md`).

use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Deserialize;

use super::CliResult;
use super::gh::{gh_capture, require_gh};
use super::manifest::DEFAULT_CONTENT_DIR;
use crate::components::mempool::{derive_gas, parse_mempool_frontmatter};
use crate::models::manifest::ContentManifestDocument;

const MEMPOOL_MOUNT_DECL_PATH: &str = ".websh/mounts/mempool.mount.json";

#[derive(Deserialize)]
struct MountDeclarationFile {
    backend: String,
    repo: Option<String>,
    branch: Option<String>,
    root: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct MempoolMountInfo {
    /// `owner/repo`, e.g., `0xwonj/websh-mempool`.
    pub(crate) repo: String,
    /// Branch to read from / write to.
    pub(crate) branch: String,
    /// Sub-path inside the repo whose subtree is exposed at mount root.
    /// Empty string means the repo root itself.
    pub(crate) root_prefix: String,
}

/// Read the mempool mount declaration from
/// `<root>/content/.websh/mounts/mempool.mount.json` and return the resolved
/// repo / branch / root prefix. Errors when the file is missing, malformed,
/// references a non-github backend, or omits the repo field.
pub(crate) fn read_mempool_mount_declaration(root: &Path) -> CliResult<MempoolMountInfo> {
    let path = mempool_mount_decl_path(root);
    if !path.exists() {
        return Err(format!(
            "mempool mount declaration not found at {} — run `websh-cli mount init` first",
            path.display()
        )
        .into());
    }
    let body = std::fs::read_to_string(&path)?;
    let decl: MountDeclarationFile = serde_json::from_str(&body)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;

    if decl.backend != "github" {
        return Err(format!(
            "mempool mount at {} declares backend `{}`; expected `github`",
            path.display(),
            decl.backend
        )
        .into());
    }
    let repo = decl
        .repo
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{} is missing required `repo` field", path.display()))?;
    let branch = decl
        .branch
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "main".to_string());
    let root_prefix = decl.root.unwrap_or_default();

    Ok(MempoolMountInfo {
        repo,
        branch,
        root_prefix,
    })
}

fn mempool_mount_decl_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_CONTENT_DIR).join(MEMPOOL_MOUNT_DECL_PATH)
}

#[derive(Args)]
pub(crate) struct MempoolCommand {
    #[command(subcommand)]
    command: MempoolSubcommand,
}

#[derive(Subcommand)]
enum MempoolSubcommand {
    /// List pending entries in the mempool repo.
    List,
    /// Promote a mempool entry to the canonical chain via a single local
    /// git commit on the bundle source. Optionally also drops the entry
    /// from the mempool repo (`--drop-remote`).
    Promote(PromoteArgs),
    /// Delete an entry from the mempool repo.
    Drop(DropArgs),
}

#[derive(Args)]
struct PromoteArgs {
    /// Repo-relative path inside the mempool repo (e.g., `writing/test.md`).
    #[arg(long)]
    path: String,
    /// After the local commit, also delete the entry from the mempool repo.
    #[arg(long, default_value_t = false)]
    drop_remote: bool,
    /// Skip attestation regeneration (useful when GPG is not configured).
    #[arg(long, default_value_t = false)]
    no_attest: bool,
    /// Allow promote when `content/` has uncommitted changes.
    #[arg(long, default_value_t = false)]
    allow_dirty: bool,
}

#[derive(Args)]
struct DropArgs {
    /// Repo-relative path inside the mempool repo.
    #[arg(long)]
    path: String,
    /// Succeed silently if the entry no longer exists.
    #[arg(long, default_value_t = false)]
    if_exists: bool,
}

pub(crate) fn run(root: &Path, command: MempoolCommand) -> CliResult {
    match command.command {
        MempoolSubcommand::List => list(root),
        MempoolSubcommand::Promote(args) => promote(root, args),
        MempoolSubcommand::Drop(args) => drop_entry(root, args),
    }
}

fn list(root: &Path) -> CliResult {
    let mount = read_mempool_mount_declaration(root)?;
    require_gh()?;

    let manifest_repo_path = if mount.root_prefix.trim_matches('/').is_empty() {
        "manifest.json".to_string()
    } else {
        format!("{}/manifest.json", mount.root_prefix.trim_matches('/'))
    };
    let manifest_url = format!(
        "repos/{}/contents/{}?ref={}",
        mount.repo, manifest_repo_path, mount.branch,
    );

    // Fetch the raw manifest. 404 → empty mempool (matches Phase 4 backend
    // semantics), so the user sees `0 pending entries` instead of an error.
    let manifest_body = match gh_capture([
        "api",
        "-H",
        "Accept: application/vnd.github.raw",
        manifest_url.as_str(),
    ]) {
        Ok(body) => body,
        Err(e) if e.to_string().contains("HTTP 404") || e.to_string().contains("Not Found") => {
            println!("{} @ {}:", mount.repo, mount.branch);
            println!("0 pending entries");
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    let manifest: ContentManifestDocument = serde_json::from_str(&manifest_body)
        .map_err(|e| format!("failed to parse mempool manifest: {e}"))?;

    println!("{} @ {}:", mount.repo, mount.branch);
    if manifest.files.is_empty() {
        println!("0 pending entries");
        return Ok(());
    }

    for file in &manifest.files {
        let body_url = format!(
            "repos/{}/contents/{}?ref={}",
            mount.repo,
            file_in_repo(&mount.root_prefix, &file.path),
            mount.branch,
        );
        let body = match gh_capture([
            "api",
            "-H",
            "Accept: application/vnd.github.raw",
            body_url.as_str(),
        ]) {
            Ok(body) => body,
            Err(e) => {
                eprintln!("warn: failed to fetch {}: {e}", file.path);
                continue;
            }
        };
        let meta = parse_mempool_frontmatter(&body);
        let status = meta
            .as_ref()
            .and_then(|m| m.status.clone())
            .unwrap_or_else(|| "?".to_string());
        let modified = meta
            .as_ref()
            .and_then(|m| m.modified.clone())
            .unwrap_or_else(|| "—".to_string());
        let is_markdown = file.path.ends_with(".md");
        let gas = derive_gas(&body, body.len(), is_markdown);
        println!(
            "  {:6} {:32} {:14} {}",
            status, file.path, gas, modified,
        );
    }
    println!("{} pending entries", manifest.files.len());

    Ok(())
}

/// Compose `<prefix>/<path>` for the GitHub Contents API URL, handling the
/// empty-prefix case so we don't emit a leading slash.
fn file_in_repo(root_prefix: &str, file_path: &str) -> String {
    let prefix = root_prefix.trim_matches('/');
    if prefix.is_empty() {
        file_path.to_string()
    } else {
        format!("{prefix}/{file_path}")
    }
}

fn promote(_root: &Path, _args: PromoteArgs) -> CliResult {
    Err("mempool promote: implemented in Task 4".into())
}

fn drop_entry(_root: &Path, _args: DropArgs) -> CliResult {
    Err("mempool drop: implemented in Task 5".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_mount(root: &Path, body: &str) {
        let p = mempool_mount_decl_path(root);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
    }

    #[test]
    fn reads_minimal_declaration() {
        let root = tempdir();
        write_mount(
            &root,
            r#"{"backend":"github","mount_at":"/mempool","repo":"0xwonj/m","branch":"main","root":"","writable":true,"name":"mempool"}"#,
        );
        let info = read_mempool_mount_declaration(&root).unwrap();
        assert_eq!(info.repo, "0xwonj/m");
        assert_eq!(info.branch, "main");
        assert_eq!(info.root_prefix, "");
    }

    #[test]
    fn defaults_branch_to_main_when_missing() {
        let root = tempdir();
        write_mount(
            &root,
            r#"{"backend":"github","mount_at":"/mempool","repo":"0xwonj/m"}"#,
        );
        let info = read_mempool_mount_declaration(&root).unwrap();
        assert_eq!(info.branch, "main");
    }

    #[test]
    fn rejects_non_github_backend() {
        let root = tempdir();
        write_mount(&root, r#"{"backend":"ipfs","mount_at":"/x","repo":"x/y"}"#);
        let err = read_mempool_mount_declaration(&root).unwrap_err();
        assert!(err.to_string().contains("backend `ipfs`"));
    }

    #[test]
    fn rejects_missing_repo() {
        let root = tempdir();
        write_mount(&root, r#"{"backend":"github","mount_at":"/mempool"}"#);
        let err = read_mempool_mount_declaration(&root).unwrap_err();
        assert!(err.to_string().contains("missing required `repo`"));
    }

    #[test]
    fn rejects_empty_repo_string() {
        let root = tempdir();
        write_mount(
            &root,
            r#"{"backend":"github","mount_at":"/mempool","repo":""}"#,
        );
        let err = read_mempool_mount_declaration(&root).unwrap_err();
        assert!(err.to_string().contains("missing required `repo`"));
    }

    #[test]
    fn errors_when_file_missing() {
        let root = tempdir();
        let err = read_mempool_mount_declaration(&root).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut d = std::env::temp_dir();
        d.push(format!(
            "websh-mempool-test-{}-{}",
            std::process::id(),
            id
        ));
        if d.exists() {
            fs::remove_dir_all(&d).unwrap();
        }
        fs::create_dir_all(&d).unwrap();
        d
    }
}
