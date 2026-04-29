//! Mempool CLI: list pending entries, promote a draft to the canonical
//! chain (atomic local commit on the bundle source), drop a draft from
//! the mempool repo. Replaces Phase 3's browser-side promote per the
//! Phase 5 design (`docs/superpowers/specs/2026-04-28-mempool-phase5-design.md`).

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Stdio};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use clap::{Args, Subcommand};
use serde::Deserialize;

use super::CliResult;
use super::gh::{gh_capture, gh_succeeds, require_gh};
use super::manifest::DEFAULT_CONTENT_DIR;
use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::components::mempool::{
    ComposeError, ComposeForm, ComposePayload, derive_gas, form_to_payload,
    parse_mempool_frontmatter, serialize_mempool_file, slug_from_title, validate_form,
};
use crate::config::BOOTSTRAP_SITE;
use crate::models::manifest::{ContentManifestDocument, ContentManifestFile};
use crate::utils::current_timestamp;
use crate::utils::format::format_date_iso;

#[derive(Deserialize)]
struct ContentsApiResponse {
    content: String,
    sha: String,
}

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
    /// Create a new mempool entry by committing it to the mempool repo.
    /// CRUD-symmetry counterpart to promote/drop; lets terminal-only or
    /// scripted workflows author drafts without opening the browser.
    Add(AddArgs),
    /// Promote a mempool entry to the canonical chain via a single local
    /// git commit on the bundle source. Optionally also drops the entry
    /// from the mempool repo (`--drop-remote`).
    Promote(PromoteArgs),
    /// Delete an entry from the mempool repo.
    Drop(DropArgs),
}

#[derive(Args)]
struct AddArgs {
    /// Category segment (e.g., `writing`, `papers`). Must be one of
    /// LEDGER_CATEGORIES.
    #[arg(long)]
    category: String,
    /// Slug. Defaults to a kebab-case derivation of `--title` if omitted.
    #[arg(long)]
    slug: Option<String>,
    /// Frontmatter title.
    #[arg(long)]
    title: String,
    /// Frontmatter status. `draft` or `review`.
    #[arg(long, default_value = "draft")]
    status: String,
    /// Frontmatter priority (`low`, `med`, `high`). Omitted if absent.
    #[arg(long)]
    priority: Option<String>,
    /// Frontmatter tags, comma-separated. Empty / absent → no tags.
    #[arg(long, default_value = "")]
    tags: String,
    /// Modified date, `YYYY-MM-DD`. Defaults to today.
    #[arg(long)]
    modified: Option<String>,
    /// Path to a file containing the markdown body, or `-` to read from stdin.
    #[arg(long)]
    body: String,
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
        MempoolSubcommand::Add(args) => add(root, args),
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
        println!("  {:6} {:32} {:14} {}", status, file.path, gas, modified,);
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

/// Resolved promote target: where the entry comes from in the mempool repo
/// and where it lands in the bundle source on disk.
#[derive(Clone, Debug)]
struct PromoteTarget {
    /// Path inside the mempool repo, e.g., `writing/foo.md`.
    repo_path: String,
    /// Category segment, e.g., `writing`. Captured for diagnostics; not
    /// directly read because `bundle_disk_path` already encodes it.
    #[allow(dead_code)]
    category: String,
    /// `<category>/<slug>` (no extension), used in commit messages.
    slug_relpath: String,
    /// Filesystem path (relative to repo root) where the body lands:
    /// `content/<category>/<slug>.md`.
    bundle_disk_path: PathBuf,
}

/// Tracks which mutations have happened so the rollback knows what to undo
/// on partial failure.
#[derive(Default)]
struct PromoteCleanup {
    body_written: bool,
    ledger_written: bool,
    manifest_written: bool,
    attest_written: bool,
}

fn add(root: &Path, args: AddArgs) -> CliResult {
    let mount = read_mempool_mount_declaration(root)?;
    require_gh()?;

    let body = read_body_source(&args.body)?;
    let form = build_form(&args, &body)?;

    let errors = validate_form(&form);
    if !errors.is_empty() {
        let messages: Vec<String> = errors.iter().map(humanize_compose_error).collect();
        return Err(format!("invalid input:\n  - {}", messages.join("\n  - ")).into());
    }

    let repo_path = format!("{}/{}.md", form.category, form.slug);
    if gh_path_exists(&mount, &repo_path)? {
        return Err(format!(
            "{} already exists in {}@{} — pass a different --slug or edit via the browser",
            repo_path, mount.repo, mount.branch
        )
        .into());
    }

    let payload = form_to_payload(&form);
    let file_body = serialize_mempool_file(&payload);

    eprintln!("preflight: ok ({}/{})", form.category, form.slug);
    eprintln!("write:     {} ({} bytes)", repo_path, file_body.len());

    add_to_mempool_via_gh(&mount, &repo_path, &file_body, &payload)?;

    println!(
        "mempool add: {} → {}@{}",
        repo_path, mount.repo, mount.branch
    );
    Ok(())
}

/// Read the markdown body from `--body` argument: `-` means stdin, anything
/// else is a filesystem path.
fn read_body_source(spec: &str) -> CliResult<String> {
    if spec == "-" {
        let mut buf = String::new();
        io::Read::read_to_string(&mut io::stdin(), &mut buf)?;
        Ok(buf)
    } else {
        Ok(std::fs::read_to_string(spec)?)
    }
}

/// Build a `ComposeForm` from the parsed CLI args. Auto-derives slug from
/// title when `--slug` is omitted, defaults `modified` to today.
fn build_form(args: &AddArgs, body: &str) -> CliResult<ComposeForm> {
    let slug = args
        .slug
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| slug_from_title(&args.title));

    let modified = args
        .modified
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format_date_iso(current_timestamp() / 1000));

    let tags: Vec<String> = args
        .tags
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    let priority = args.priority.clone().filter(|s| !s.is_empty());

    Ok(ComposeForm {
        title: args.title.trim().to_string(),
        category: args.category.clone(),
        slug,
        status: args.status.clone(),
        modified,
        priority,
        tags,
        body: body.to_string(),
    })
}

/// Two-step add: PUT the file blob, then PUT the rewritten manifest with
/// the new entry inserted. File-first so a step-2 failure leaves the runtime
/// view consistent (manifest is the truth; the orphan file is invisible).
fn add_to_mempool_via_gh(
    mount: &MempoolMountInfo,
    repo_path: &str,
    file_body: &str,
    payload: &ComposePayload,
) -> CliResult {
    let absolute_file_path = file_in_repo(&mount.root_prefix, repo_path);
    let absolute_manifest_path = file_in_repo(&mount.root_prefix, "manifest.json");

    // Step 1: PUT the new file (no sha — it's a create, not an update).
    let file_b64 = BASE64_STANDARD.encode(file_body.as_bytes());
    let put_file_url = format!("repos/{}/contents/{}", mount.repo, absolute_file_path);
    let mut put_file = Process::new("gh");
    put_file.args([
        "api",
        put_file_url.as_str(),
        "-X",
        "PUT",
        "-f",
        &format!("message=mempool: add {repo_path}"),
        "-f",
        &format!("content={file_b64}"),
        "-f",
        &format!("branch={}", mount.branch),
    ]);
    put_file.stdout(Stdio::null());
    let status = put_file.status()?;
    if !status.success() {
        return Err(format!(
            "file PUT failed for {repo_path} on {}@{}; nothing changed",
            mount.repo, mount.branch
        )
        .into());
    }

    // Step 2: read manifest, insert entry, PUT.
    let manifest_url = format!(
        "repos/{}/contents/{}?ref={}",
        mount.repo, absolute_manifest_path, mount.branch,
    );
    let manifest_resp_json = gh_capture(["api", manifest_url.as_str()])?;
    let manifest_resp: ContentsApiResponse = serde_json::from_str(&manifest_resp_json)
        .map_err(|e| format!("failed to parse manifest GET response: {e}"))?;
    let manifest_bytes = BASE64_STANDARD
        .decode(manifest_resp.content.replace('\n', ""))
        .map_err(|e| format!("failed to base64-decode manifest: {e}"))?;
    let mut manifest: ContentManifestDocument = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("failed to parse mempool manifest: {e}"))?;

    let modified_secs = current_timestamp() / 1000;
    let new_entry = ContentManifestFile {
        path: repo_path.to_string(),
        title: payload.title.clone(),
        size: Some(file_body.as_bytes().len() as u64),
        modified: Some(modified_secs),
        date: None,
        tags: payload.tags.clone(),
        access: None,
    };
    manifest.files.retain(|f| f.path != repo_path);
    manifest.files.push(new_entry);
    manifest.files.sort_by(|a, b| a.path.cmp(&b.path));

    let new_body = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("failed to re-serialize manifest: {e}"))?
        + "\n";
    let new_body_b64 = BASE64_STANDARD.encode(new_body.as_bytes());
    let put_manifest_url = format!("repos/{}/contents/{}", mount.repo, absolute_manifest_path);
    let mut put_manifest = Process::new("gh");
    put_manifest.args([
        "api",
        put_manifest_url.as_str(),
        "-X",
        "PUT",
        "-f",
        &format!("message=mempool: add {repo_path} (manifest)"),
        "-f",
        &format!("content={new_body_b64}"),
        "-f",
        &format!("sha={}", manifest_resp.sha),
        "-f",
        &format!("branch={}", mount.branch),
    ]);
    put_manifest.stdout(Stdio::null());
    let status = put_manifest.status()?;
    if !status.success() {
        return Err(format!(
            "manifest PUT failed; the file {} is on {}@{} but the manifest doesn't reference \
             it (runtime won't see it). Re-run `websh-cli mempool add` after deleting the file \
             via the GitHub web UI, or manually edit manifest.json.",
            repo_path, mount.repo, mount.branch
        )
        .into());
    }

    Ok(())
}

/// Translate a single `ComposeError` into a CLI-friendly message. Mirrors the
/// browser's compose modal field-error text where relevant.
fn humanize_compose_error(err: &ComposeError) -> String {
    match err {
        ComposeError::TitleEmpty => "title is required".to_string(),
        ComposeError::TitleHasReservedChars => {
            "title cannot contain \" \\ : or newlines".to_string()
        }
        ComposeError::SlugInvalid => {
            "slug must be kebab-case ASCII (a-z, 0-9, hyphens)".to_string()
        }
        ComposeError::StatusUnknown => "status must be `draft` or `review`".to_string(),
        ComposeError::ModifiedNotIso => "modified must be YYYY-MM-DD".to_string(),
        ComposeError::CategoryUnknown => {
            format!("category must be one of {}", LEDGER_CATEGORIES.join(", "))
        }
        ComposeError::PriorityUnknown => "priority must be `low`, `med`, or `high`".to_string(),
        ComposeError::TagHasReservedChars => {
            "tags cannot contain `[ ] \" ,` or newlines".to_string()
        }
    }
}

fn promote(root: &Path, args: PromoteArgs) -> CliResult {
    let target = parse_promote_path(&args.path)?;
    let mount = read_mempool_mount_declaration(root)?;
    require_gh()?;

    // Step 0 — pre-flight (no mutation).
    if !args.allow_dirty {
        ensure_clean_working_tree(root)?;
    }
    confirm_on_bundle_branch(root)?;
    gh_verify_path_exists(&mount, &target)?;
    ensure_local_target_absent(root, &target)?;

    eprintln!("preflight: ok ({})", target.repo_path);

    // Step 1 — fetch + write + regenerate.
    let body = fetch_mempool_body(&mount, &target)?;
    eprintln!("fetch:    {} ({} bytes)", target.repo_path, body.len());

    let mut cleanup = PromoteCleanup::default();
    if let Err(e) = run_promote_steps(root, &target, &body, &args, &mut cleanup) {
        rollback(root, &target, &cleanup);
        return Err(e);
    }

    // Step 2 — git commit.
    if let Err(e) = stage_and_commit(root, &target, cleanup.attest_written) {
        rollback(root, &target, &cleanup);
        return Err(e);
    }

    // Step 3 — optional drop-remote.
    if args.drop_remote {
        match drop_via_gh(&mount, &target.repo_path) {
            Ok(DropOutcome::Removed { manifest, blob }) => println!(
                "mempool drop: removed {} from {} (manifest={}, blob={})",
                target.repo_path, mount.repo, manifest, blob
            ),
            Ok(DropOutcome::Absent) => println!(
                "mempool drop: {} already absent from {}",
                target.repo_path, mount.repo
            ),
            Err(e) => eprintln!(
                "mempool drop: {e} — re-run `websh-cli mempool drop --path {}` to retry",
                args.path
            ),
        }
    }

    println!("\nready. review the commit, then `git push` and `just pin` to deploy.");
    Ok(())
}

/// Parse `--path` into a structured PromoteTarget. Validates `<category>/<slug>.md`
/// shape with category in `LEDGER_CATEGORIES`.
fn parse_promote_path(repo_relative: &str) -> CliResult<PromoteTarget> {
    let trimmed = repo_relative.trim_start_matches('/');
    if !trimmed.ends_with(".md") {
        return Err(format!("promote path must end in `.md` (got `{repo_relative}`)").into());
    }
    let mut parts = trimmed.splitn(2, '/');
    let category = parts
        .next()
        .filter(|c| !c.is_empty())
        .ok_or_else(|| format!("promote path missing category segment: `{repo_relative}`"))?
        .to_string();
    let rest = parts
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("promote path missing slug segment: `{repo_relative}`"))?;
    if !LEDGER_CATEGORIES.contains(&category.as_str()) {
        return Err(format!(
            "category `{category}` is not in LEDGER_CATEGORIES ({:?})",
            LEDGER_CATEGORIES
        )
        .into());
    }
    let slug = rest
        .strip_suffix(".md")
        .expect("ends_with(.md) checked above")
        .to_string();
    if slug.is_empty() || slug.contains('/') {
        return Err(format!("promote path slug must be a single segment, got `{rest}`").into());
    }
    let slug_relpath = format!("{category}/{slug}");
    let bundle_disk_path = PathBuf::from(DEFAULT_CONTENT_DIR)
        .join(&category)
        .join(format!("{slug}.md"));

    Ok(PromoteTarget {
        repo_path: trimmed.to_string(),
        category,
        slug_relpath,
        bundle_disk_path,
    })
}

fn ensure_clean_working_tree(root: &Path) -> CliResult {
    let mut cmd = Process::new("git");
    cmd.current_dir(root)
        .args(["status", "--porcelain", "--", "content"]);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err(format!(
            "git status failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
        .into());
    }
    if !out.stdout.is_empty() {
        return Err(format!(
            "uncommitted changes in content/. Stage/stash them or pass --allow-dirty:\n{}",
            String::from_utf8_lossy(&out.stdout).trim()
        )
        .into());
    }
    Ok(())
}

fn confirm_on_bundle_branch(root: &Path) -> CliResult {
    let mut cmd = Process::new("git");
    cmd.current_dir(root)
        .args(["rev-parse", "--abbrev-ref", "HEAD"]);
    let out = cmd.output()?;
    if !out.status.success() {
        return Err("git rev-parse failed (is this a git checkout?)".into());
    }
    let current = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let expected = BOOTSTRAP_SITE.branch;
    if current == expected {
        return Ok(());
    }
    eprint!("warn: HEAD is `{current}`, deploy branch is `{expected}`. Continue? [y/N] ");
    io::stderr().flush().ok();
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let trimmed = answer.trim();
    if trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes") {
        Ok(())
    } else {
        Err(format!("aborted: not on `{expected}`").into())
    }
}

fn gh_verify_path_exists(mount: &MempoolMountInfo, target: &PromoteTarget) -> CliResult {
    if !gh_path_exists(mount, &target.repo_path)? {
        return Err(format!(
            "{} not found in {}@{}",
            target.repo_path, mount.repo, mount.branch
        )
        .into());
    }
    Ok(())
}

/// Generic existence check for any path inside the mempool repo. Returns
/// `Ok(true)` when `gh api` reports 200 (file is present), `Ok(false)` on
/// non-zero exit (covers 404 and the like).
fn gh_path_exists(mount: &MempoolMountInfo, repo_path: &str) -> CliResult<bool> {
    let url = format!(
        "repos/{}/contents/{}?ref={}",
        mount.repo,
        file_in_repo(&mount.root_prefix, repo_path),
        mount.branch,
    );
    gh_succeeds(["api", "--silent", url.as_str()])
}

fn ensure_local_target_absent(root: &Path, target: &PromoteTarget) -> CliResult {
    let p = root.join(&target.bundle_disk_path);
    if p.exists() {
        return Err(format!(
            "{} already exists locally — pick a different slug or `git rm` the existing file",
            target.bundle_disk_path.display()
        )
        .into());
    }
    Ok(())
}

fn fetch_mempool_body(mount: &MempoolMountInfo, target: &PromoteTarget) -> CliResult<String> {
    let url = format!(
        "repos/{}/contents/{}?ref={}",
        mount.repo,
        file_in_repo(&mount.root_prefix, &target.repo_path),
        mount.branch,
    );
    gh_capture([
        "api",
        "-H",
        "Accept: application/vnd.github.raw",
        url.as_str(),
    ])
}

fn run_promote_steps(
    root: &Path,
    target: &PromoteTarget,
    body: &str,
    args: &PromoteArgs,
    cleanup: &mut PromoteCleanup,
) -> CliResult {
    // Ensure the parent directory exists, then write the body.
    let abs_path = root.join(&target.bundle_disk_path);
    if let Some(parent) = abs_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    cleanup.body_written = true; // Set before write — partial-write on disk-full counts.
    std::fs::write(&abs_path, body)?;
    eprintln!("write:    {}", target.bundle_disk_path.display());

    if args.no_attest {
        // Direct ledger + manifest regeneration. Set flags BEFORE the calls
        // so a mid-write failure still lets rollback restore the prior state.
        cleanup.ledger_written = true;
        let ledger = super::ledger::generate_content_ledger(root, Path::new(DEFAULT_CONTENT_DIR))?;
        cleanup.manifest_written = true;
        let manifest =
            super::manifest::generate_content_manifest(root, Path::new(DEFAULT_CONTENT_DIR))?;
        eprintln!(
            "ledger:   {} entries -> content/.websh/ledger.json",
            ledger.entry_count
        );
        eprintln!(
            "manifest: {} files / {} directories -> content/manifest.json",
            manifest.files.len(),
            manifest.directories.len()
        );
    } else {
        // attest::run_default writes ledger.json + manifest.json + attestations.json
        // sequentially; flag each as potentially-written before invocation so a
        // mid-flow signing failure rolls back all three.
        cleanup.ledger_written = true;
        cleanup.manifest_written = true;
        cleanup.attest_written = true;
        super::attest::run_default(root, /*no_sign*/ false)?;
    }
    Ok(())
}

fn stage_and_commit(root: &Path, target: &PromoteTarget, did_attest: bool) -> CliResult {
    let mut paths: Vec<PathBuf> = vec![
        target.bundle_disk_path.clone(),
        PathBuf::from("content/.websh/ledger.json"),
        PathBuf::from("content/manifest.json"),
    ];
    if did_attest {
        paths.push(PathBuf::from("assets/crypto/attestations.json"));
    }

    let mut add = Process::new("git");
    add.current_dir(root).arg("add").arg("--");
    for p in &paths {
        add.arg(p);
    }
    let add_status = add.status()?;
    if !add_status.success() {
        return Err("git add failed".into());
    }

    let msg = format!("promote: {}", target.slug_relpath);
    let mut commit = Process::new("git");
    commit.current_dir(root).args(["commit", "-m", &msg]);
    let commit_status = commit.status()?;
    if !commit_status.success() {
        return Err("git commit failed".into());
    }
    Ok(())
}

fn rollback(root: &Path, target: &PromoteTarget, c: &PromoteCleanup) {
    // Reset the index FIRST so subsequent `git checkout HEAD --` actually
    // restores from HEAD instead of from the (potentially-staged-with-new-
    // content) index. This ordering is correct whether or not `git add`
    // has run before the failure point.
    let _ = git_quiet(
        root,
        [
            "reset",
            "HEAD",
            "--",
            "content/",
            "assets/crypto/attestations.json",
        ],
    );
    if c.body_written {
        let _ = std::fs::remove_file(root.join(&target.bundle_disk_path));
    }
    // `git checkout HEAD -- <path>` (vs the bare `git checkout -- <path>`)
    // explicitly sources from HEAD, ignoring whatever's in the index. Safe
    // even if the index already matches HEAD (no-op).
    if c.ledger_written {
        let _ = git_quiet(
            root,
            ["checkout", "HEAD", "--", "content/.websh/ledger.json"],
        );
    }
    if c.manifest_written {
        let _ = git_quiet(root, ["checkout", "HEAD", "--", "content/manifest.json"]);
    }
    if c.attest_written {
        let _ = git_quiet(
            root,
            ["checkout", "HEAD", "--", "assets/crypto/attestations.json"],
        );
        // .websh/local/crypto/attestations is gitignored; not restored.
    }
}

fn git_quiet<I, S>(root: &Path, args: I) -> CliResult
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut cmd = Process::new("git");
    cmd.current_dir(root).args(args);
    let _ = cmd.output();
    Ok(())
}

/// Drop a mempool entry via two sequential GitHub Contents API calls:
///
/// 1. Fetch + parse the mempool repo's `manifest.json`, remove the entry,
///    PUT the rewritten manifest (atomically replaces it on the branch).
/// 2. DELETE the file blob.
///
/// Manifest-first order means a step-2 failure leaves the repo in a
/// "dangling blob" state — the manifest no longer references the file but
/// the file still lives in the git tree. The runtime scan reads the
/// manifest, so the user-facing mempool view is correct. The orphan blob
/// is harmless and will be cleaned up the next time the file is committed
/// to (or by `git gc`).
fn drop_via_gh(mount: &MempoolMountInfo, path_in_repo: &str) -> CliResult<DropOutcome> {
    let absolute_file_path = file_in_repo(&mount.root_prefix, path_in_repo);
    let absolute_manifest_path = file_in_repo(&mount.root_prefix, "manifest.json");

    // Step 1: rewrite manifest (skip if entry isn't present).
    let manifest_url = format!(
        "repos/{}/contents/{}?ref={}",
        mount.repo, absolute_manifest_path, mount.branch,
    );
    let manifest_resp_json = gh_capture(["api", manifest_url.as_str()])?;
    let manifest_resp: ContentsApiResponse = serde_json::from_str(&manifest_resp_json)
        .map_err(|e| format!("failed to parse manifest GET response: {e}"))?;
    let manifest_bytes = BASE64_STANDARD
        .decode(manifest_resp.content.replace('\n', ""))
        .map_err(|e| format!("failed to base64-decode manifest: {e}"))?;
    let mut manifest: ContentManifestDocument = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("failed to parse mempool manifest: {e}"))?;

    let before = manifest.files.len();
    manifest.files.retain(|f| f.path != path_in_repo);
    let manifest_changed = manifest.files.len() != before;

    if manifest_changed {
        let new_body = serde_json::to_string_pretty(&manifest)
            .map_err(|e| format!("failed to re-serialize manifest: {e}"))?
            + "\n";
        let new_body_b64 = BASE64_STANDARD.encode(new_body.as_bytes());
        let put_url = format!("repos/{}/contents/{}", mount.repo, absolute_manifest_path);
        let mut put_cmd = Process::new("gh");
        put_cmd.args([
            "api",
            put_url.as_str(),
            "-X",
            "PUT",
            "-f",
            &format!("message=mempool: drop {path_in_repo}"),
            "-f",
            &format!("content={new_body_b64}"),
            "-f",
            &format!("sha={}", manifest_resp.sha),
            "-f",
            &format!("branch={}", mount.branch),
        ]);
        // Suppress the JSON response gh prints by default; we only want
        // pass/fail.
        put_cmd.stdout(Stdio::null());
        let status = put_cmd.status()?;
        if !status.success() {
            return Err(format!(
                "manifest update failed when dropping {path_in_repo}; nothing else changed"
            )
            .into());
        }
    }

    // Step 2: delete the file blob (skip cleanly if already absent).
    let file_url = format!(
        "repos/{}/contents/{}?ref={}",
        mount.repo, absolute_file_path, mount.branch,
    );
    let blob_deleted = match gh_capture(["api", "--jq", ".sha", file_url.as_str()]) {
        Ok(file_sha_raw) => {
            let file_sha = file_sha_raw.trim().trim_matches('"').to_string();
            if file_sha.is_empty() {
                return Err(format!("could not extract sha for {path_in_repo}").into());
            }
            let delete_url = format!("repos/{}/contents/{}", mount.repo, absolute_file_path);
            let mut del_cmd = Process::new("gh");
            del_cmd.args([
                "api",
                delete_url.as_str(),
                "-X",
                "DELETE",
                "-f",
                &format!("message=mempool: drop {path_in_repo} (blob)"),
                "-f",
                &format!("sha={file_sha}"),
                "-f",
                &format!("branch={}", mount.branch),
            ]);
            del_cmd.stdout(Stdio::null());
            let status = del_cmd.status()?;
            if !status.success() {
                return Err(format!(
                    "blob delete failed for {} (manifest already updated; orphan blob remains — \
                     re-run `websh-cli mempool drop --path {}` later to retry the blob delete)",
                    path_in_repo, path_in_repo
                )
                .into());
            }
            true
        }
        Err(_) => false, // File already absent; manifest cleanup may still have happened.
    };

    if !manifest_changed && !blob_deleted {
        Ok(DropOutcome::Absent)
    } else {
        Ok(DropOutcome::Removed {
            manifest: manifest_changed,
            blob: blob_deleted,
        })
    }
}

fn drop_entry(root: &Path, args: DropArgs) -> CliResult {
    let mount = read_mempool_mount_declaration(root)?;
    require_gh()?;

    let outcome = drop_via_gh(&mount, &args.path)?;
    match outcome {
        DropOutcome::Removed { manifest, blob } => {
            println!(
                "mempool drop: removed {} from {} (manifest={}, blob={})",
                args.path, mount.repo, manifest, blob,
            );
            Ok(())
        }
        DropOutcome::Absent => {
            if args.if_exists {
                println!("mempool drop: {} not present, nothing to do", args.path);
                Ok(())
            } else {
                Err(format!("entry not found at {}", args.path).into())
            }
        }
    }
}

enum DropOutcome {
    /// At least one of (manifest entry, file blob) was removed.
    Removed { manifest: bool, blob: bool },
    /// Neither manifest nor blob existed.
    Absent,
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

    #[test]
    fn parse_promote_path_extracts_category_slug_and_disk_path() {
        let t = parse_promote_path("writing/foo.md").unwrap();
        assert_eq!(t.repo_path, "writing/foo.md");
        assert_eq!(t.category, "writing");
        assert_eq!(t.slug_relpath, "writing/foo");
        assert_eq!(t.bundle_disk_path, PathBuf::from("content/writing/foo.md"));
    }

    #[test]
    fn parse_promote_path_strips_leading_slash() {
        let t = parse_promote_path("/papers/bar.md").unwrap();
        assert_eq!(t.repo_path, "papers/bar.md");
        assert_eq!(t.bundle_disk_path, PathBuf::from("content/papers/bar.md"));
    }

    #[test]
    fn parse_promote_path_rejects_non_md_extension() {
        let err = parse_promote_path("writing/foo.txt").unwrap_err();
        assert!(err.to_string().contains("must end in `.md`"));
    }

    #[test]
    fn parse_promote_path_rejects_unknown_category() {
        let err = parse_promote_path("fiction/foo.md").unwrap_err();
        assert!(err.to_string().contains("not in LEDGER_CATEGORIES"));
    }

    #[test]
    fn parse_promote_path_rejects_nested_slug() {
        let err = parse_promote_path("writing/series/foo.md").unwrap_err();
        assert!(err.to_string().contains("single segment"));
    }

    #[test]
    fn parse_promote_path_rejects_missing_slug() {
        let err = parse_promote_path("writing/.md").unwrap_err();
        assert!(err.to_string().contains("single segment") || err.to_string().contains("slug"));
    }

    #[test]
    fn file_in_repo_handles_empty_prefix() {
        assert_eq!(file_in_repo("", "writing/foo.md"), "writing/foo.md");
        assert_eq!(file_in_repo("/", "writing/foo.md"), "writing/foo.md");
    }

    #[test]
    fn file_in_repo_prepends_prefix() {
        assert_eq!(
            file_in_repo("content", "writing/foo.md"),
            "content/writing/foo.md"
        );
        assert_eq!(
            file_in_repo("/content/", "writing/foo.md"),
            "content/writing/foo.md"
        );
    }

    fn sample_add_args() -> AddArgs {
        AddArgs {
            category: "writing".into(),
            slug: None,
            title: "On writing slow".into(),
            status: "draft".into(),
            priority: None,
            tags: String::new(),
            modified: Some("2026-04-28".into()),
            body: "/dev/null".into(),
        }
    }

    #[test]
    fn build_form_auto_derives_slug_from_title() {
        let args = sample_add_args();
        let form = build_form(&args, "body").unwrap();
        assert_eq!(form.slug, "on-writing-slow");
        assert_eq!(form.category, "writing");
        assert_eq!(form.title, "On writing slow");
        assert_eq!(form.modified, "2026-04-28");
        assert_eq!(form.status, "draft");
        assert!(form.priority.is_none());
        assert!(form.tags.is_empty());
        assert_eq!(form.body, "body");
    }

    #[test]
    fn build_form_uses_explicit_slug_when_set() {
        let mut args = sample_add_args();
        args.slug = Some("custom-slug".into());
        let form = build_form(&args, "").unwrap();
        assert_eq!(form.slug, "custom-slug");
    }

    #[test]
    fn build_form_parses_comma_separated_tags() {
        let mut args = sample_add_args();
        args.tags = "essay, slow ,zk".into();
        let form = build_form(&args, "").unwrap();
        assert_eq!(form.tags, vec!["essay", "slow", "zk"]);
    }

    #[test]
    fn build_form_drops_empty_tags() {
        let mut args = sample_add_args();
        args.tags = ", , ".into();
        let form = build_form(&args, "").unwrap();
        assert!(form.tags.is_empty());
    }

    #[test]
    fn build_form_normalizes_priority() {
        let mut args = sample_add_args();
        args.priority = Some("med".into());
        let form = build_form(&args, "").unwrap();
        assert_eq!(form.priority.as_deref(), Some("med"));

        args.priority = Some(String::new());
        let form = build_form(&args, "").unwrap();
        assert!(form.priority.is_none());
    }

    #[test]
    fn validate_form_rejects_form_built_from_bad_args() {
        // Empty title → form has empty title → validate_form flags it.
        let mut args = sample_add_args();
        args.title = String::new();
        let form = build_form(&args, "").unwrap();
        let errs = validate_form(&form);
        assert!(errs.iter().any(|e| matches!(e, ComposeError::TitleEmpty)));
    }

    #[test]
    fn validate_form_rejects_unknown_category() {
        let mut args = sample_add_args();
        args.category = "fiction".into();
        // slug is auto-derived (form will pass slug validation), so the
        // only failure should be category.
        let form = build_form(&args, "").unwrap();
        let errs = validate_form(&form);
        assert!(
            errs.iter()
                .any(|e| matches!(e, ComposeError::CategoryUnknown))
        );
    }

    #[test]
    fn validate_form_rejects_invalid_modified() {
        let mut args = sample_add_args();
        args.modified = Some("April 28".into());
        let form = build_form(&args, "").unwrap();
        let errs = validate_form(&form);
        assert!(
            errs.iter()
                .any(|e| matches!(e, ComposeError::ModifiedNotIso))
        );
    }

    #[test]
    fn humanize_compose_error_covers_every_variant() {
        let variants = [
            ComposeError::TitleEmpty,
            ComposeError::TitleHasReservedChars,
            ComposeError::SlugInvalid,
            ComposeError::StatusUnknown,
            ComposeError::ModifiedNotIso,
            ComposeError::CategoryUnknown,
            ComposeError::PriorityUnknown,
            ComposeError::TagHasReservedChars,
        ];
        for v in variants {
            let msg = humanize_compose_error(&v);
            assert!(!msg.is_empty(), "variant {:?} produced empty message", v);
        }
    }

    fn tempdir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let mut d = std::env::temp_dir();
        d.push(format!("websh-mempool-test-{}-{}", std::process::id(), id));
        if d.exists() {
            fs::remove_dir_all(&d).unwrap();
        }
        fs::create_dir_all(&d).unwrap();
        d
    }
}
