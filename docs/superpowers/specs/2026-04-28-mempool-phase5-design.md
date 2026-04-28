# Mempool — Phase 5 Design (CLI Promote)

**Date:** 2026-04-28
**Phase:** 5 (architectural pivot, beyond original 3-phase plan)
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Phase 3 (browser promote, superseded by this phase):** [`2026-04-28-mempool-phase3-design.md`](./2026-04-28-mempool-phase3-design.md)
**Phase 4 (hardening, baseline):** [`2026-04-28-mempool-phase4-design.md`](./2026-04-28-mempool-phase4-design.md)

Phase 3 shipped browser-side promote: a modal in `/ledger` that ran a sequential two-commit transaction (bundle add + mempool drop) against two GitHub repos. Live QA exposed structural fragility:

- **Cross-repo non-atomicity** — GitHub has no cross-repo transaction. Phase 3's `PartialFailure` recovery (~150 lines: retry button, sticky banner, modal state machine) exists only to paper over this.
- **Bundle-source PAT in browser** — promote requires `contents:write` on `0xwonj/websh` (the deploy source-of-truth). Anyone with browser session access can directly commit to the publish queue.
- **Ledger/manifest/attestations stay stale until deploy** — promote only adds the file commit; the CLI regenerates the rest at `just pin` time. So `/ledger` and the chain block list don't reflect the promotion until the next deploy.
- **Truth boundary violation** — bundle source is owned by the local CLI / deploy ritual. Browser promote reaches across that boundary on every promote, against the spirit of master plan A3.

Phase 5 moves promote to the CLI. Compose and edit stay in the browser (the workflow we want preserved: mobile draft → desktop publish). Promote becomes part of the deploy ritual where it naturally belongs.

## 1. Scope

In:

- New CLI subcommand: `websh-cli mempool list` — show pending entries from `0xwonj/websh-mempool`.
- New CLI subcommand: `websh-cli mempool promote --path <repo-relative>` — atomic local promote.
- New CLI subcommand: `websh-cli mempool drop --path <repo-relative>` — clean up without promoting.
- Removal of browser promote: `PromoteConfirmModal`, the per-item Promote button, deploy-hint and partial-failure banners, `promote_state` / `deploy_hint` / `partial_warning` signals from `LedgerPage`.
- Most of `src/components/mempool/promote.rs` becomes dead. Pure helpers (`promote_target_path`, `promote_commit_messages`, change-set builders) move to the CLI; `apply_commit_outcome` and its private `mount_id_fallback` move to `compose.rs` (or to a shared `mempool/post_commit.rs` module — `compose.rs:311` is the sole remaining consumer).
- Master plan §1, §3, §4, §8 deltas land before implementation begins (covered in §6.4).

Out:

- Promote orchestration that touches the deployed site UX in any way (no "promote candidate" badges, no "queued for promote" lists).
- Daemon / automated promote — V2.
- Multi-author conflict detection — V1 single-user assumption preserved.
- Editing canonical chain entries via CLI (`mempool drop` removes from mempool only; once an entry is promoted, the user edits `content/` directly).

## 2. Anchor Decisions (Phase 5 specifics)

| # | Decision | Rationale |
|---|---|---|
| P5-1 | Promote produces a single git commit on success. Pre-commit failures roll back via the explicit cleanup steps in §8 — true atomicity is not claimed without `git2`-style ODB-only staging, which is out of scope. | "Single-commit on success + documented rollback" is sufficient for V1's single-user, low-cadence publish flow. |
| P5-2 | The committed delta is `content/<category>/<slug>.md` + regenerated `content/.websh/ledger.json` + regenerated `content/manifest.json` + (when `--no-attest` is not passed) regenerated `assets/crypto/attestations.json` and `.websh/local/crypto/attestations/*` | All bundle-source artifacts move to the new state in one commit, so the deploy build sees a consistent tree. |
| P5-3 | Mempool repo cleanup happens after the local commit, as a separate best-effort GitHub Contents API call (only when `--drop-remote` is passed) | If the local commit succeeds and the mempool drop fails, the user has duplicate entry visibility (entry in both repos) until they re-run `mempool drop`. Failure mode is bounded and recoverable; never loses content. |
| P5-4 | GitHub API access uses the `gh` subprocess pattern established in Phase 4 (`src/cli/mount.rs:173-208`) | Same model as `mount init`; one host-side GitHub-access mechanism. Long-term consolidation deferred to the parked `2026-04-28-github-client-refactor.md` design. |
| P5-5 | The bundle-source git commit is created via `Command::new("git")` subprocess | Matches `cli/deploy.rs`'s pinata-subprocess style; avoids the `git2` binding cost; rollback semantics are well-understood (`git restore` for tracked files, `git clean -f` for untracked). |
| P5-6 | `mempool promote` does **not** push automatically. The user reviews the commit, then `git push` + `just pin` on their own cadence. | Explicit publish step preserved (master §1). The commit is local until the user wants it shared. |
| P5-7 | `mempool list` reads via the same `gh api` path — read-only, idempotent | Discoverability without leaving the terminal; uses gh's stored auth, same as Phase 4. |
| P5-8 | The promote PAT-scope removal is **promote-specific**: browser no longer writes to bundle source via the promote flow. The terminal `sync commit` flow at `cwd=/` is **unchanged in this phase** and continues to need bundle-write. The full PAT-narrowing only happens when (or if) terminal authoring at `/` is later retired. | Honest scope statement. The C1 risk surfaced in design review was that the original wording oversold the security delta. |

### 2.1 Explicit non-decisions

- We do **not** integrate promote into `just pin` (e.g., interactive prompt). Promote is a discrete decision that produces a reviewable git commit; bundling it into deploy hides the decision and removes the review step.
- We do **not** preserve Phase 3's promote-related UI in any form. No "promote candidate" badges, no "promotable" markers on mempool items.
- We do **not** add a `mempool publish` (promote + push + deploy in one) command. Three separate commands keeps each step reviewable.

## 3. CLI Surface

### 3.1 `websh-cli mempool list`

```
$ cargo run --bin websh-cli -- mempool list
0xwonj/websh-mempool @ main:
  draft  writing/test            ~10 words   2026-04-28
  review papers/zkgrep-circuit   ~1200 words  2026-04-25
2 pending entries
```

Reads `<mempool-repo>/manifest.json` via authenticated Contents API. Lists each entry with status (draft/review), path, gas (word count or byte size), modified date.

Empty mempool: `0 pending entries`. No exit-code distinction; informational only.

### 3.2 `websh-cli mempool promote`

CLI surface (full):

```
$ cargo run --bin websh-cli -- mempool promote --path <repo-relative-path>
                                               [--drop-remote]
                                               [--no-attest]
                                               [--allow-dirty]
```

| Flag | Default | Meaning |
|---|---|---|
| `--path` | required | The repo-relative path inside the mempool repo, e.g., `writing/test.md`. |
| `--drop-remote` | off | After the local commit lands, also delete the entry from the mempool repo via `gh api -X DELETE`. Off by default so the user can review the commit before touching mempool. |
| `--no-attest` | off | Skip the attestation regeneration step. Useful when the user lacks a configured GPG keychain. The local commit then doesn't include `assets/crypto/attestations.json` / `.websh/local/crypto/attestations/*`; the next deploy's `attest_all` fills them in. |
| `--allow-dirty` | off | Skip the pre-flight `git status --porcelain content/` check. By default, promote refuses to run when the working tree under `content/` has uncommitted changes, so the resulting commit is a clean diff. |

Pre-flight steps (Step 0 — must all pass before any disk or remote mutation):

1. Resolve `<category>/<slug>.md` from `--path`.
2. (Unless `--allow-dirty`) confirm `git status --porcelain content/` is empty.
3. Confirm `git rev-parse --abbrev-ref HEAD` matches the bundle source's configured branch (per `BOOTSTRAP_SITE.branch`); soft warning otherwise (continue with `[Y/n]` prompt).
4. Verify the mempool entry exists: `gh api repos/<mempool-repo>/contents/<path>?ref=<branch>` returns 200.
5. Confirm the bundle target path does not exist on local disk: `content/<category>/<slug>.md`.

Sample successful output:

```
$ cargo run --bin websh-cli -- mempool promote --path writing/test.md
preflight: working tree clean
preflight: on branch main
preflight: writing/test.md exists in 0xwonj/websh-mempool@main
preflight: content/writing/test.md does not exist
fetch:    writing/test.md (250 bytes)
write:    content/writing/test.md
ledger:   8 entries -> content/.websh/ledger.json
manifest: 10 files / 8 directories -> content/manifest.json
attest:   9 subjects, 9 manifest files, 8 ledger entries, 1 new pgp signature
git:      staged 4 paths
git:      committed: promote: writing/test [a1b2c3d]

ready. review the commit, then `git push` and `just pin` to deploy.
```

Algorithm:

```rust
fn promote(opts: PromoteOpts) -> CliResult {
    // Step 0 — pre-flight (no mutation yet).
    let target = parse_promote_path(&opts.path)?;            // <category>/<slug>.md
    if !opts.allow_dirty {
        ensure_clean_working_tree("content")?;               // git status --porcelain content/
    }
    confirm_on_bundle_branch()?;                             // soft warning, prompts on mismatch
    gh_verify_path_exists(MEMPOOL_REPO, &target.repo_path)?; // 200 from contents API
    ensure_local_target_absent(&target.bundle_disk_path)?;   // content/<category>/<slug>.md

    // Step 1 — fetch, regenerate, write to disk.
    let body = gh_get_raw(MEMPOOL_REPO, &target.repo_path)?;
    fs::write(&target.bundle_disk_path, &body)
        .map_err(|e| { rollback_partial(&target, false, false, false); e })?;
    let ledger = generate_content_ledger(/*paths*/)
        .map_err(|e| { rollback_partial(&target, true, false, false); e })?;
    let manifest = generate_content_manifest(/*paths*/)
        .map_err(|e| { rollback_partial(&target, true, true, false); e })?;
    let did_attest = if !opts.no_attest {
        attest_all(/*paths*/)
            .map_err(|e| { rollback_partial(&target, true, true, true); e })?;
        true
    } else { false };

    // Step 2 — single git commit.
    let mut paths = vec![
        target.bundle_disk_path.clone(),
        PathBuf::from("content/.websh/ledger.json"),
        PathBuf::from("content/manifest.json"),
    ];
    if did_attest {
        paths.push(PathBuf::from("assets/crypto/attestations.json"));
        // .websh/local/crypto/attestations/* is gitignored — not staged
    }
    git_subprocess(["add", "--", &paths])
        .map_err(|e| { rollback_partial(&target, true, true, did_attest); e })?;
    git_subprocess(["commit", "-m", &format!("promote: {}", target.slug_relpath)])
        .map_err(|e| { rollback_full(&target, did_attest); e })?;

    // Step 3 — optional mempool drop (best-effort).
    if opts.drop_remote {
        match gh_delete_contents(MEMPOOL_REPO, &target.repo_path) {
            Ok(_) => println!("mempool drop: removed {} from {MEMPOOL_REPO}", target.repo_path),
            Err(e) => println!(
                "mempool drop: {e} — re-run `websh-cli mempool drop --path {}` to retry",
                opts.path
            ),
        }
    }

    println!("\nready. review the commit, then `git push` and `just pin` to deploy.");
    Ok(())
}
```

`rollback_partial` is the explicit cleanup helper. Concrete commands per failure point:

| Failure point | Rollback executed | What it does |
|---|---|---|
| `fs::write` fails | nothing — disk untouched | OS-level retry / fix |
| `generate_content_ledger` fails | `rm content/<category>/<slug>.md` | Remove the just-written body |
| `generate_content_manifest` fails | + `git checkout -- content/.websh/ledger.json` | Restore the prior ledger from git |
| `attest_all` fails | + `git checkout -- content/manifest.json` | Restore the prior manifest |
| `git add` fails | + `git checkout -- assets/crypto/attestations.json` (if attestation regen happened) | Restore prior attestations |
| `git commit` fails | All of the above + `git restore --staged content/...` | Reset the index |

`rollback_full` runs every step of the partial cleanup — used after the commit step itself errors. The intent is: any failure leaves the working tree byte-for-byte identical to its pre-promote state.

Default-off `--drop-remote`: the user inspects the local commit (`git show HEAD`) before deciding to remove the entry from mempool. This preserves the "review before publish" posture the rest of the V1 deploy ritual encodes.

`--allow-dirty`: power-user escape hatch. Without it, promote refuses to run on a dirty `content/` working tree, preventing the user from accidentally bundling unrelated edits into the promote commit.

### 3.3 `websh-cli mempool drop`

```
$ cargo run --bin websh-cli -- mempool drop --path writing/test.md [--if-exists]
verify: writing/test.md exists in 0xwonj/websh-mempool@main
github: deleted writing/test.md (commit f4d5e6f)
```

Single GitHub Contents API DELETE on the mempool repo. Used to clean up after a successful promote (when `--drop-remote` was not passed) or to discard a draft entirely.

Semantics:

- **Strict by default**: a missing entry exits with a non-zero error code and the message `entry not found at <path>`. This catches typo'd slugs and clearly signals "nothing was deleted".
- `--if-exists`: succeed-on-missing. The CLI prints `mempool drop: <path> not present, nothing to do` and exits 0. Use this when retrying after a previous successful drop, or in scripts where the drop is idempotent.

The 4 behavioural combinations:

| State | Without `--if-exists` | With `--if-exists` |
|---|---|---|
| Entry exists | Delete + exit 0 | Delete + exit 0 |
| Entry missing | Exit non-zero | Skip + exit 0 |

### 3.4 (Future) `websh-cli mempool promote-all` — not in Phase 5

Defer until usage shows it's needed.

## 4. Browser-Side Removal

After Phase 5 lands, the browser keeps:

| Feature | Status |
|---|---|
| Read-only mempool section on `/ledger` (Phase 1) | ✅ kept |
| Compose modal + per-mount auth detection (Phase 2) | ✅ kept |
| Edit existing mempool entry (Phase 2) | ✅ kept |
| `/ledger` filter integration | ✅ kept |
| Per-item Promote button | ❌ removed |
| `PromoteConfirmModal` | ❌ removed |
| Deploy-hint banner / partial-failure banner | ❌ removed |
| `promote_state` / `deploy_hint` / `partial_warning` signals | ❌ removed |
| `promote_entry` / `retry_mempool_drop` async handlers | ❌ removed |

Code deletion estimate: `src/components/mempool/promote.rs` shrinks from ~600 lines to ~30 (just `apply_commit_outcome`, which moves to `compose.rs` as a private helper). The `tests/mempool_promote.rs` test file is mostly retained but tests target CLI helpers instead of the wasm path.

PAT scope reduction: browser PAT no longer needs `0xwonj/websh:contents:write`. Only `0xwonj/websh-mempool:contents:write` is needed for compose/edit. This is a meaningful security surface reduction.

## 5. Component Tree After Phase 5

```
LedgerPage
├── LedgerIdentifier
├── LedgerHeader
├── LedgerFilterBar
├── Mempool                       ← still here
│   └── MempoolItem (×N)           ← Phase 1 click-to-preview / Phase 2 click-to-edit
│       └── (no Promote button)
├── LedgerChain
├── MempoolPreviewModal           ← Phase 1
└── ComposeModal                  ← Phase 2
    (no PromoteConfirmModal)
```

## 6. Files

### 6.1 New files

| Path | Purpose |
|---|---|
| `src/cli/mempool.rs` | New `MempoolCommand` enum: `List`, `Promote { path, drop_remote }`, `Drop { path }`. Dispatches to subcommand handlers. |
| `tests/mempool_cli.rs` | Integration tests for the CLI surface — pure helpers (path parsing, message construction) plus mocked GitHub responses for the API-touching paths. |

### 6.2 Modified files

| Path | Change |
|---|---|
| `src/cli/mod.rs` | Register `Mempool(MempoolCommand)` subcommand |
| `src/components/mempool/promote.rs` | Delete most of file. Move `apply_commit_outcome` to `compose.rs`. Possibly delete entirely. |
| `src/components/mempool/mod.rs` | Drop re-exports of removed promote symbols |
| `src/components/mempool/component.rs` | Remove `on_promote: Callback<MempoolEntry>` prop and the promote button. Remove `author_mode` thread for promote (still kept for compose button visibility) |
| `src/components/mempool/mempool.module.css` | Remove `.mpPromote`, `.mpActions` |
| `src/components/ledger_page.rs` | Remove `promote_state`, `deploy_hint`, `partial_warning`, `on_mempool_promote`, `on_promote_done`, `on_promote_partial`, `PromoteConfirmModal` mount, `PromoteStatusBanner` |
| `src/components/ledger_page.module.css` | Remove `.deployHint`, `.partialBanner`, `.dismiss` |
| `tests/mempool_promote.rs` | Either retire or refocus on CLI helpers (decided in implementation plan) |
| Master plan §1, §3, §4, §6, §10 | Already updated (anchor, phase plan, decision log) |

### 6.3 Out of scope

- `src/cli/deploy.rs` — promote does not integrate with `just pin` in this phase.
- Phase 4 design doc — references stay as-is; Phase 5 is the explicit revision.
- New wasm code beyond removal — no new browser features.

### 6.4 Master plan deltas (must land before implementation begins)

| File / section | Current text | Replacement |
|---|---|---|
| Master §4 line 66 | "After Phase 3, V1 is complete. V2 items (§7) are queued separately." | "After Phase 5, V1 is complete (Phases 4 and 5 are post-Phase-3 hardening + architectural pivot — see Decision Log). V2 items (§7) are queued separately." |
| Master §6 phase-plan table — Phase 3 row | Status: **Complete** | Status: **Complete (superseded by Phase 5)** |
| Master §8 #1 | "I can compose a draft entirely in the deployed site, see it in the mempool, edit it, and promote it — without any terminal interaction beyond `just pin` for the final IPFS deploy." | "I can compose a draft entirely in the deployed site, see it in the mempool, and edit it without leaving the browser. Promotion happens at the local terminal via `websh-cli mempool promote` followed by `git push` + `just pin` — the same publish ritual the deploy step already requires." |
| Master §8 #3 | "The two-commit promotion is documented with a partial-failure recovery path." | "Promotion is a single git commit on the bundle source plus an optional best-effort mempool drop. Both surfaces have explicit failure recovery documented in Phase 5 §3.2 / §3.3." |

These edits land as part of the same commit batch that ships the Phase 5 design doc, so V1 acceptance is checkable against an internally-consistent master plan throughout the implementation.

## 7. Test Strategy

### 7.1 Unit / CLI integration

- `cli::mempool::tests` — path parsing (`writing/foo.md` → category `writing`, slug `foo`, bundle_disk_path `content/writing/foo.md`), commit message construction, idempotent `git add` glob expansion.
- `tests/mempool_cli.rs` — integration tests with stub HTTP responses for GitHub Contents API. Verifies `mempool list` parses manifest correctly, `promote` writes to disk + stages git correctly (using a temp git repo).

### 7.2 Wasm-side regression

- `cargo test --lib` — all 480 tests still pass after the promote module shrinkage.
- `cargo test --test mempool_compose` — Phase 2 still green.
- `cargo test --test mempool_model` — Phase 1 still green.
- `cargo test --test mempool_promote` — repurposed for CLI helpers, or deleted.
- `cargo check --target wasm32-unknown-unknown --lib` — wasm clean after wasm-side deletions.

### 7.3 Live QA (manual, post-implementation)

1. `0xwonj/websh-mempool` has at least one entry (use the browser to create one).
2. `cargo run --bin websh-cli -- mempool list` shows it.
3. `cargo run --bin websh-cli -- mempool promote --path writing/<slug>` — succeeds:
   - `content/writing/<slug>.md` exists locally with the entry's body
   - `content/.websh/ledger.json` includes the new route
   - `content/manifest.json` includes the new file
   - `git log -1` shows the promote commit
4. (Optional) `cargo run --bin websh-cli -- mempool drop --path writing/<slug>` — removes from mempool repo.
5. `git push` + `just pin` — deploy proceeds normally; new entry shows on the deployed `/ledger` as a chain block, no longer in mempool.

## 8. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| User runs `mempool promote` while uncommitted local changes exist in `content/` | Pre-flight: `git status --porcelain content/` returns dirty → abort with clear message. Or unstage their changes first. |
| Path collision with existing bundle entry | Pre-flight check against local disk; abort if exists. |
| Mempool entry has malformed frontmatter | Read passes through (we don't parse it for the CLI; just copy bytes). Ledger regen may surface the issue. |
| User forgets to push after `promote` | Eprintln reminder at end. Optional: `--push` flag in V2. |
| `--drop-remote` race with someone else editing the mempool entry | Best-effort. If GitHub returns 409, surface clear error and recommend `mempool drop` later. |
| Browser still has stale mempool list showing the promoted entry | Browser refetches on next `/ledger` render or sync refresh; not the CLI's problem. |
| Existing `.mempool/promote.rs` removal breaks something subtle | Reviewer-agent + full test-suite run before declaring complete. |

## 9. Acceptance Criteria

Phase 5 is complete when:

1. `websh-cli mempool list / promote / drop` are implemented with the surface above.
2. Browser-side promote modal + button + banners are removed.
3. `apply_commit_outcome` either lives in `compose.rs` (private to compose) or in a sensible shared location.
4. `cargo test --lib && cargo test --test mempool_compose && cargo test --test mempool_model && cargo test --test mempool_cli && cargo check --target wasm32-unknown-unknown --lib` — all green.
5. Live QA flow in §7.3 passes end-to-end.
6. Reviewer agent has cleared the diff with no outstanding CRITICAL or HIGH findings.
7. Master plan §4 marks Phase 5 Complete.

V1 acceptance is then: Phase 1 + 2 + 4 + 5 (Phase 3's browser promote is superseded but the design doc stays as the historical record of why we pivoted).

## 10. Open Questions

Resolved by this design (no longer open):

- **Attestation regeneration default**: ON by default. `--no-attest` opt-out for users without configured GPG (P5-2 + §3.2 flag table).
- **`git2` crate vs subprocess**: subprocess (P5-5).
- **GitHub API client**: `gh` subprocess (P5-4), matching Phase 4.
- **`mempool drop` idempotency**: strict by default + `--if-exists` opt-in (§3.3).
- **Recovery semantics**: explicit `rollback_partial` / `rollback_full` helpers in §3.2.

Deferred to V2 / future:

- **Should `mempool promote` accept multiple paths?** V1 single-path keeps each commit reviewable. If batch publish becomes a real workflow, `mempool promote-all` or `mempool promote --path a --path b` is a natural extension.
- **Should the CLI offer dry-run / preview mode?** A `--dry-run` flag that prints the would-be commit message + diff statistics without mutating anything is useful but not essential for V1. If user feedback shows a need, add it as a small follow-up.
- **Should `mempool list` filter by status (`--status draft`)?** V1's `mempool list` shows everything; per-status filtering matches a workflow we don't yet have evidence for.

## 11. Migration / Compatibility

- **Phase 3's design doc** stays in the repo as historical context. The master plan §4 row for Phase 3 reads "Complete (superseded by Phase 5)" and Phase 5's design references it and explains the supersession.
- **Existing browser sessions** with cached PAT scoped to bundle-write: continue to work for compose/edit (the terminal `sync` flow at `/` still uses bundle-write per P5-8). No forced PAT rotation.
- **`tests/mempool_promote.rs`** stays for the path-mapping and message helpers (those move to CLI), but the wasm-async-flow tests are deleted. The file may be renamed to `tests/mempool_cli.rs` if its scope ends up entirely CLI.
- **Persistent browser state**: Phase 3's `PromoteState`, `deploy_hint`, and `partial_warning` are all transient `signal()` instances scoped to `LedgerPage`. They are not persisted to IDB or `localStorage`/`sessionStorage`, so removing them is byte-equivalent to their never having existed for any persistent layer. The IDB key `remote_head.<storage_id>` written by `apply_commit_outcome` is shared with compose and stays valid; no cleanup is needed.
- **Trunk + dev workflow**: no impact. The pre_build hook still regenerates the bundle manifest.

## 12. Scheduling

This is **the next phase**. Plan to follow this design doc within the same workflow as Phases 1–4: design → plan → implementation → reviewer → master update.

If approval is given, the implementation plan should follow within this same docs/superpowers/plans/ batch.
