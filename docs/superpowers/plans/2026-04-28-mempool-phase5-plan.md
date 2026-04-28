# Mempool Phase 5 Implementation Plan

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking; flip each to `- [x]` as it ships.

**Goal:** Move promote from a browser two-commit transaction to a CLI-driven single-commit-on-bundle-source operation. Add `websh-cli mempool list / promote / drop`. Remove the browser-side promote modal, button, banners, async orchestration, and partial-failure handling. Keep compose/edit + read-only mempool view in the browser.

**Architecture:** Three new CLI subcommands under a new `Mempool` clap group. They reuse Phase 4's `gh` subprocess pattern for all GitHub Contents API calls and `Command::new("git")` subprocess for the local git commit. Pre-flight checks (clean working tree, on-deploy-branch, mempool entry exists, bundle target absent) gate every disk/network mutation. Failures execute `rollback_partial` / `rollback_full` to leave the working tree byte-equivalent to its pre-promote state.

**Tech stack:** Rust 2024 edition, clap, base64, std::process::Command. No new dependencies. Wasm side: Leptos 0.8 deletions only.

**Master plan:** [`docs/superpowers/specs/2026-04-28-mempool-master.md`](../specs/2026-04-28-mempool-master.md)
**Phase 5 design:** [`docs/superpowers/specs/2026-04-28-mempool-phase5-design.md`](../specs/2026-04-28-mempool-phase5-design.md)

---

## Prerequisites

- Phase 4 merged (`mount init` pattern is the model for Phase 5).
- Master plan §1, §3 A6/A8, §4 row Phase 3, §6 Phase 3 row, §8 #1 + #3 already updated to reflect the pivot (committed in `89d9180`/`e3eda69`).
- `gh auth status` returns logged in (Phase 4 prerequisite).
- The mempool repo `0xwonj/websh-mempool` exists and has `manifest.json` (Phase 4 bootstrap).

---

## Task 1: CLI scaffolding — subcommand enum + dispatch

**Files:**
- Create: `src/cli/mempool.rs` (new module — empty shell with the subcommand enum)
- Modify: `src/cli/mod.rs` (register the new module + dispatch the new variant)

### Steps

- [ ] **1.1: Create `src/cli/mempool.rs` skeleton.** Just the enum + dispatch fn with `todo!()` arms — no logic yet.

```rust
//! Mempool CLI: list pending entries, promote a draft to the canonical
//! chain (atomic local commit on the bundle source), drop a draft from
//! the mempool repo. Replaces Phase 3's browser-side promote.

use std::path::Path;

use clap::{Args, Subcommand};

use super::CliResult;

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
    /// from the mempool repo (`--drop-remote`). See Phase 5 design §3.2.
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
    /// Skip attestation regeneration (e.g., when GPG is not configured).
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

fn list(_root: &Path) -> CliResult {
    todo!("implemented in Task 3")
}

fn promote(_root: &Path, _args: PromoteArgs) -> CliResult {
    todo!("implemented in Task 4")
}

fn drop_entry(_root: &Path, _args: DropArgs) -> CliResult {
    todo!("implemented in Task 5")
}
```

- [ ] **1.2: Register in `src/cli/mod.rs`.**

```rust
mod mempool;   // add alongside other modules

#[derive(Subcommand)]
enum Command {
    // ...existing variants
    Mempool(mempool::MempoolCommand),
}

pub fn run() -> CliResult {
    let cli = Cli::parse();
    let root = cli.root;
    match cli.command {
        // ...existing arms
        Command::Mempool(command) => mempool::run(&root, command),
    }
}
```

- [ ] **1.3: Compile.** `cargo build --bin websh-cli` clean. `cargo run --bin websh-cli -- mempool --help` shows the three subcommands.
- [ ] **1.4: Commit.**
    ```
    feat(cli): mempool subcommand scaffolding (list/promote/drop)
    ```

---

## Task 2: Shared helpers — mount-declaration reader, gh wrappers

**Files:**
- Modify: `src/cli/mount.rs` (extract `require_gh`, `gh_succeeds`, related helpers into a `cli::gh` module so Phase 5 can reuse them)
- Create: `src/cli/gh.rs` (or move the helpers into `cli/mod.rs` if simpler)
- Add to `src/cli/mempool.rs`: `read_mempool_mount_declaration(root)` that reads `content/.websh/mounts/mempool.mount.json` and returns `(repo, branch)`.

### Steps

- [ ] **2.1: Move shared `gh` helpers out of `mount.rs`.** Specifically:
    - `require_gh() -> CliResult`
    - `gh_succeeds(args) -> CliResult<bool>`

  Either into a new `src/cli/gh.rs` or into `src/cli/mod.rs` as `pub(super) fn require_gh()`. Pick `src/cli/gh.rs` for explicit grouping; matches the `cli/io.rs` precedent.

- [ ] **2.2: Add `gh_capture(args) -> CliResult<String>`** helper that captures stdout (used by `mempool list` to read the manifest). Pseudocode:
    ```rust
    pub(crate) fn gh_capture<I, S>(args: I) -> CliResult<String>
    where I: IntoIterator<Item = S>, S: AsRef<OsStr>,
    {
        let out = Process::new("gh").args(args).output()?;
        if !out.status.success() {
            return Err(format!(
                "gh failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ).into());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
    ```

- [ ] **2.3: Add `read_mempool_mount_declaration(root) -> CliResult<MempoolMountInfo>`** in `src/cli/mempool.rs` that reads `content/.websh/mounts/mempool.mount.json`, deserializes as `MountDeclaration`, and returns `MempoolMountInfo { repo: String, branch: String, root_prefix: String }`.

  Use the existing `MountDeclaration` model in `src/models/site.rs:78`. Validate `backend == "github"`; reject otherwise.

- [ ] **2.4: Unit tests for `read_mempool_mount_declaration`.** Tempdir, write a stub mount JSON, parse, assert fields. Reject bad backend, missing required fields.

- [ ] **2.5: Compile + tests.**
- [ ] **2.6: Commit.**
    ```
    refactor(cli): extract shared gh helpers; add mempool mount reader
    ```

---

## Task 3: Implement `mempool list`

**Files:**
- Modify: `src/cli/mempool.rs` (replace `todo!()` arm)

### Steps

- [ ] **3.1: Implement `list`.** Plan:
    1. Read mempool mount info via `read_mempool_mount_declaration(root)`.
    2. Verify `gh` available.
    3. Fetch `manifest.json` from the mempool repo:
       ```
       gh api -H "Accept: application/vnd.github.raw" \
         repos/{repo}/contents/{root_prefix}/manifest.json?ref={branch}
       ```
       The `gh api` command honors the `Accept` header and returns body to stdout. Use `gh_capture`.
    4. Parse JSON as `ScannedSubtree` (already a `Deserialize` type per `src/core/storage/backend.rs`).
    5. For each file path in `manifest.files`:
       - Fetch the file body via `gh api -H "Accept: application/vnd.github.raw" repos/{repo}/contents/{path}?ref={branch}`.
       - Parse frontmatter using `crate::components::mempool::parse_mempool_frontmatter`. (Cross-import OK — `cli` already uses `crate::crypto::*` and `crate::utils::*`.)
       - Extract status, modified date.
       - Compute `gas` (word count for `.md`, file size otherwise) using `crate::components::mempool::derive_gas` if accessible, else inline a small helper.
    6. Print formatted table:
       ```
       0xwonj/websh-mempool @ main:
         draft  writing/test            ~10 words   2026-04-28
         review papers/zkgrep-circuit   ~1200 words 2026-04-25
       2 pending entries
       ```
       Empty mempool: `0 pending entries`.

- [ ] **3.2: Edge cases.**
    - 404 on manifest fetch → empty mempool, exit 0 with `0 pending entries` (matches Phase 4's runtime-side 404 handling).
    - Frontmatter parse failure on individual file → skip with a `warn:` line on stderr, continue.

- [ ] **3.3: Smoke test manually** against the real mempool repo (which has `writing/test.md`):
    ```bash
    cargo run --bin websh-cli -- mempool list
    ```
    Expect 1 pending entry shown.

- [ ] **3.4: Commit.**
    ```
    feat(cli): mempool list — fetch manifest + render entries via gh subprocess
    ```

---

## Task 4: Implement `mempool promote`

**Files:**
- Modify: `src/cli/mempool.rs` (replace the `todo!()` for `promote`)
- Possibly: `src/cli/mod.rs` (re-exports if needed)

### Steps

- [ ] **4.1: Define internal types.**
    ```rust
    struct PromoteTarget {
        repo_path: String,        // e.g., "writing/test.md" — path inside mempool repo
        category: String,         // "writing"
        slug_relpath: String,     // "writing/test" — used in commit message
        bundle_disk_path: PathBuf,// content/writing/test.md (relative to root)
    }
    ```

- [ ] **4.2: Implement `parse_promote_path(repo_relative: &str) -> CliResult<PromoteTarget>`.**

    Validate shape: `<category>/<slug>.md` where category ∈ `LEDGER_CATEGORIES` (re-export from `crate::components::ledger_routes`). Reject non-`.md` paths. Return populated target.

- [ ] **4.3: Implement pre-flight Step 0** as a series of single-purpose helpers that each return `CliResult<()>`:

    | Helper | What it checks | Failure message |
    |---|---|---|
    | `ensure_clean_working_tree(root, "content")` | `git status --porcelain content/` empty unless `--allow-dirty` | "uncommitted changes in content/. Stage or stash them, or pass --allow-dirty." |
    | `confirm_on_bundle_branch(root, expected_branch)` | `git rev-parse --abbrev-ref HEAD` == bundle source's branch | Soft warning + `[Y/n]` prompt. (Use `BOOTSTRAP_SITE.branch` — the prompt accepts only Y / N from stdin.) |
    | `gh_verify_path_exists(repo, branch, path)` | `gh api repos/{repo}/contents/{path}?ref={branch}` returns 200 | "{path} not found in {repo}@{branch}" |
    | `ensure_local_target_absent(root, bundle_disk_path)` | `root.join(bundle_disk_path).exists()` is false | "{path} already exists locally — pick a different slug or `git rm` the old" |

- [ ] **4.4: Implement the Step 1 + 2 + 3 algorithm** per design §3.2.

    Use the existing CLI helpers:
    - `crate::cli::ledger::generate_content_ledger(root, content_dir)` — already exists.
    - `crate::cli::manifest::generate_content_manifest(root, content_dir)` — already exists; idempotent thanks to Phase 4's `write_json` skip.
    - `crate::cli::attest::run_default(root, no_sign: bool)` — runs the full attest flow (regenerate subjects, sign with GPG). Phase 5 reuses it; pass `no_sign = false` (false because Phase 5 wants real signatures) unless we add an explicit `--no-attest` skip.

  Wait — `run_default` always runs full attest. For `--no-attest`, we should NOT call it at all. Refactor: in promote, conditionally:
    ```rust
    if !args.no_attest {
        crate::cli::attest::run_default(root, /*no_sign*/ false)?;
    }
    ```

- [ ] **4.5: Implement `rollback_partial`/`rollback_full` helpers.**

    ```rust
    /// Cleanup state captured during promote so rollbacks know what to undo.
    struct PromoteCleanup {
        body_written: bool,      // content/<cat>/<slug>.md written
        ledger_written: bool,    // content/.websh/ledger.json regenerated
        manifest_written: bool,  // content/manifest.json regenerated
        attest_written: bool,    // assets/crypto/attestations.json regenerated
        target: PromoteTarget,
    }

    fn rollback(root: &Path, c: &PromoteCleanup) {
        if c.body_written {
            let _ = fs::remove_file(root.join(&c.target.bundle_disk_path));
        }
        if c.ledger_written {
            let _ = run(root, ["git", "checkout", "--", "content/.websh/ledger.json"]);
        }
        if c.manifest_written {
            let _ = run(root, ["git", "checkout", "--", "content/manifest.json"]);
        }
        if c.attest_written {
            let _ = run(root, ["git", "checkout", "--", "assets/crypto/attestations.json"]);
            // .websh/local/crypto/attestations is gitignored; not restored.
        }
        // After commit failure, also reset the index:
        let _ = run(root, ["git", "reset", "HEAD", "--",
            "content/", "assets/crypto/attestations.json"]);
    }
    ```

    Each step in the main flow updates `cleanup.<flag> = true` immediately after the corresponding mutation succeeds. On any error, call `rollback(&cleanup)` then propagate the error.

- [ ] **4.6: Implement git commit step.**

    ```rust
    let mut paths = vec![
        target.bundle_disk_path.clone(),
        PathBuf::from("content/.websh/ledger.json"),
        PathBuf::from("content/manifest.json"),
    ];
    if did_attest {
        paths.push(PathBuf::from("assets/crypto/attestations.json"));
    }
    let mut add = Process::new("git").current_dir(root).arg("add").arg("--");
    for p in &paths { add.arg(p); }
    if !add.status()?.success() { rollback(...); bail!("git add failed"); }

    let msg = format!("promote: {}", target.slug_relpath);
    let commit = Process::new("git")
        .current_dir(root)
        .args(["commit", "-m", &msg])
        .status()?;
    if !commit.success() { rollback(...); bail!("git commit failed"); }
    ```

- [ ] **4.7: Implement optional `--drop-remote`.**

    ```rust
    if args.drop_remote {
        match drop_via_gh(&mount.repo, &mount.branch, &target.repo_path) {
            Ok(_) => println!("mempool drop: removed {} from {}", target.repo_path, mount.repo),
            Err(e) => println!(
                "mempool drop: {e} — re-run `websh-cli mempool drop --path {}` to retry",
                args.path
            ),
        }
    }
    ```

    `drop_via_gh` will be implemented in Task 5 — for now, factor it out so Task 5 reuses.

- [ ] **4.8: Smoke test against live mempool**:
    ```bash
    cargo run --bin websh-cli -- mempool promote --path writing/test.md
    ```
    Expect:
    - Pre-flight passes
    - `content/writing/test.md` written, ledger/manifest/attestations regenerated
    - 1 git commit on local main
    - Mempool repo unchanged (default `--drop-remote` off)

    Then test with `--drop-remote`:
    ```bash
    # restore: undo the commit + git reset
    git reset --hard HEAD~1
    # re-promote with drop
    cargo run --bin websh-cli -- mempool promote --path writing/test.md --drop-remote
    ```

- [ ] **4.9: Commit.**
    ```
    feat(cli): mempool promote — atomic local commit + optional drop
    ```

---

## Task 5: Implement `mempool drop`

**Files:**
- Modify: `src/cli/mempool.rs` (replace the `todo!()` for `drop_entry`)

### Steps

- [ ] **5.1: Implement `drop_via_gh(repo, branch, path) -> CliResult<()>`.**

    The GitHub Contents API DELETE requires the file's current SHA. Algorithm:
    ```
    GET  /repos/{repo}/contents/{path}?ref={branch}     # get sha
    DELETE same path with body { message, sha, branch }
    ```

    Via `gh api`:
    ```
    sha=$(gh api repos/$repo/contents/$path?ref=$branch --jq .sha)
    gh api repos/$repo/contents/$path -X DELETE \
      -f message="mempool: drop $path" \
      -f sha="$sha" \
      -f branch="$branch"
    ```

    In Rust:
    ```rust
    let sha = gh_capture(["api", &format!("repos/{repo}/contents/{path}?ref={branch}"),
                          "--jq", ".sha"])?.trim().trim_matches('"').to_string();
    let status = Process::new("gh").args([
        "api", &format!("repos/{repo}/contents/{path}"), "-X", "DELETE",
        "-f", &format!("message=mempool: drop {path}"),
        "-f", &format!("sha={sha}"),
        "-f", &format!("branch={branch}"),
    ]).status()?;
    if !status.success() { bail!("delete failed"); }
    ```

- [ ] **5.2: Implement `drop_entry(args)`** with `--if-exists` semantics:

    ```rust
    let mount = read_mempool_mount_declaration(root)?;
    let exists = gh_path_exists(&mount.repo, &mount.branch, &args.path)?;
    if !exists {
        if args.if_exists {
            println!("mempool drop: {} not present, nothing to do", args.path);
            return Ok(());
        }
        bail!("entry not found at {}", args.path);
    }
    drop_via_gh(&mount.repo, &mount.branch, &args.path)?;
    println!("mempool drop: removed {} from {}", args.path, mount.repo);
    Ok(())
    ```

    `gh_path_exists` returns `Ok(true)` on 200, `Ok(false)` on 404, `Err(_)` on auth/network.

- [ ] **5.3: Smoke test.**
    ```bash
    # against an entry that exists
    cargo run --bin websh-cli -- mempool drop --path writing/test.md
    # expect: deletion + exit 0

    # against a missing entry, strict
    cargo run --bin websh-cli -- mempool drop --path writing/nonexistent.md
    # expect: error + non-zero exit

    # against a missing entry, idempotent
    cargo run --bin websh-cli -- mempool drop --path writing/nonexistent.md --if-exists
    # expect: "not present, nothing to do" + exit 0
    ```

- [ ] **5.4: Commit.**
    ```
    feat(cli): mempool drop — strict by default, --if-exists for idempotent retry
    ```

---

## Task 6: Browser-side removal

**Files:**
- Modify: `src/components/mempool/component.rs` (drop `on_promote` prop and the promote button on `MempoolItem`)
- Modify: `src/components/mempool/mempool.module.css` (drop `.mpPromote`, `.mpActions`)
- Modify: `src/components/mempool/promote.rs` (delete the wasm-async + UI parts; keep only `apply_commit_outcome` + `mount_id_fallback` if compose still needs them, OR move them into compose.rs)
- Modify: `src/components/mempool/compose.rs` (move `apply_commit_outcome` here as a private helper, drop the `super::promote::` reference)
- Modify: `src/components/mempool/mod.rs` (drop the now-removed re-exports)
- Modify: `src/components/ledger_page.rs` (remove `promote_state`, `deploy_hint`, `partial_warning`, `on_mempool_promote`, `on_promote_done`, `on_promote_partial`, `PromoteConfirmModal` mount, `PromoteStatusBanner` component, the related CSS class references)
- Modify: `src/components/ledger_page.module.css` (drop `.deployHint`, `.partialBanner`, `.dismiss`)
- Modify or delete: `tests/mempool_promote.rs` (keep only the path-mapping / message-construction unit tests, ideally moved to `cli/mempool.rs` tests — wasm async-flow tests are dead)

### Steps

- [ ] **6.1: Move `apply_commit_outcome` into `compose.rs`** as a private helper. `mount_id_fallback` follows. Update the call site `super::promote::apply_commit_outcome(&ctx, &root, &outcome).await` → `apply_commit_outcome(&ctx, &root, &outcome).await`. Delete the public re-export from `mod.rs`.

- [ ] **6.2: Delete the Promote UI from MempoolItem.** Remove the `on_promote: Callback<MempoolEntry>` prop, the `<Show when=author_mode>` wrapper around the Promote button, and the related `event.stop_propagation()` on the button's click.

- [ ] **6.3: Delete `PromoteConfirmModal` + supporting state in `ledger_page.rs`.** Remove all signals, callbacks, the modal mount, the banner component. The `on_compose_new` callback stays.

- [ ] **6.4: Trim `promote.rs` to empty (or delete the file).** If `apply_commit_outcome` and helpers moved out cleanly, the file is empty; delete it. If they didn't, the file shrinks to just those helpers and `mod.rs` re-exports them privately to compose.

- [ ] **6.5: Update `mod.rs`.** Remove all `pub use promote::*` — every remaining public symbol from promote.rs is gone.

- [ ] **6.6: Trim the test file** `tests/mempool_promote.rs`. Helpers that survive (path-mapping, commit-message construction) should already be CLI-tested in Task 4; consider deleting the file entirely or repurposing as `tests/mempool_cli.rs`. Pick whichever produces the cleaner diff.

- [ ] **6.7: Verify wasm builds clean.**
    ```bash
    cargo check --target wasm32-unknown-unknown --lib
    cargo test --lib
    cargo test --test mempool_compose
    cargo test --test mempool_model
    ```

- [ ] **6.8: Commit.**
    ```
    refactor(mempool): remove browser-side promote (replaced by CLI in Phase 5)
    ```

---

## Task 7: Tests

**Files:**
- Create or modify: `tests/mempool_cli.rs`
- Modify (already in Task 6 if applicable): delete `tests/mempool_promote.rs`

### Steps

- [ ] **7.1: Write integration tests for the CLI helpers.**

    The HTTP-touching paths can't be unit-tested without a stub `gh`. Cover the pure-Rust pieces:
    - `parse_promote_path("writing/test.md")` returns the expected target.
    - Reject `parse_promote_path("invalid")`, `parse_promote_path("foo/bar.txt")`.
    - `read_mempool_mount_declaration` parses a stub mount.json correctly.
    - Rejects bad backend, missing repo, etc.
    - Commit message format: `"promote: writing/test"` for `target.slug_relpath = "writing/test"`.

- [ ] **7.2: Run all test suites.**
    ```bash
    cargo test --lib                              # 480+ tests
    cargo test --test mempool_compose             # Phase 2
    cargo test --test mempool_model               # Phase 1
    cargo test --test mempool_cli                 # Phase 5 (new)
    cargo check --target wasm32-unknown-unknown --lib
    ```

- [ ] **7.3: Commit.**
    ```
    test(cli): mempool helpers integration tests
    ```

---

## Task 8: Live QA

(Manual — not committed.)

- [ ] **8.1: Bootstrap a fresh mempool entry from the browser.** `trunk serve`, `sync auth set <PAT>`, compose a new draft, save. Verify the entry shows in `mempool list`.

- [ ] **8.2: `cargo run --bin websh-cli -- mempool list`** shows the entry.

- [ ] **8.3: `cargo run --bin websh-cli -- mempool promote --path <category>/<slug>.md`** creates a single local commit. Inspect with `git show HEAD` — the diff should be: 1 new content file, modified ledger.json, modified manifest.json, modified attestations.json (if `--no-attest` was not passed). No other files.

- [ ] **8.4: `cargo run --bin websh-cli -- mempool drop --path <category>/<slug>.md`** removes the entry from mempool repo. Verify by re-running `mempool list` — entry gone.

- [ ] **8.5: `git push` + `just pin`** completes the deploy. New entry shows on the deployed `/ledger` as a chain block.

---

## Task 9: Reviewer + master plan finalization

- [ ] **9.1: Dispatch `superpowers:code-reviewer` agent on the Phase 5 diff.**
    Pass: phase5 design + plan + diff between `e3eda69` (last design commit) and HEAD.

- [ ] **9.2: Address all CRITICAL / HIGH findings** in follow-up commits.

- [ ] **9.3: Update master plan §4** — Phase 5 status → **Complete**.

- [ ] **9.4: Update master plan §6** — add `Phase 5 Plan` row pointing at `2026-04-28-mempool-phase5-plan.md` with status Complete.

- [ ] **9.5: Append §10 Decision Log** entry with the rationale + commit count.

- [ ] **9.6: Commit + push.**
    ```
    docs(mempool): mark Phase 5 complete in master plan
    ```

---

## Self-Review

Plan covers Phase 5 design §1 (scope), §2 (anchor decisions implemented in Tasks 1, 4, 5), §3 (CLI surface in Tasks 3, 4, 5), §4 (browser removal in Task 6), §6 (file changes), §7 (test strategy in Task 7 + Live QA in Task 8), §8 (failure modes via Task 4's rollback helpers), §9 (acceptance covered in Task 8 live QA + Task 9 reviewer), §11 (migration — verified during Task 6 and 8).

Type signatures consistent: `MempoolCommand` enum + `PromoteArgs`, `DropArgs` defined in Task 1; consumed in Tasks 3, 4, 5. `PromoteTarget`, `MempoolMountInfo` defined in Tasks 4, 2.

Execution order: Tasks 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 are dependency-ordered. Task 6 (browser removal) is intentionally after CLI is complete and live-validated, so we don't lose the browser flow if the CLI flow has a problem during build-out.

Risks watched while implementing:
- `gh` subprocess interleaving — promote uses `gh api` for fetch + drop AND `git` subprocess for commit. The two are independent OS processes; race-free as long as we serialize calls (the implementation does, via sequential `await`).
- Frontmatter parsing for `mempool list` requires importing wasm-side helpers from `crate::components::mempool::parse`. The `cli` module already cross-imports `crate::crypto::*` and `crate::utils::*`, so the precedent is established. If the import causes wasm-only `cfg` issues, factor `parse_mempool_frontmatter` into `core/` instead.
- `Command::new("git")` working dir — every git invocation MUST `current_dir(root)` to handle the case where the user ran the CLI from outside the repo (clap's `--root` flag).
