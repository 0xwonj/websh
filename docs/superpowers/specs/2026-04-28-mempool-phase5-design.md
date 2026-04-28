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
- Most of `src/components/mempool/promote.rs` becomes dead. Pure helpers (`promote_target_path`, `promote_commit_messages`, change-set builders, `apply_commit_outcome`) move to where they're used (CLI for the path/message helpers; `compose.rs` for `apply_commit_outcome` since save_compose is the only remaining wasm consumer).
- Master plan §1 / §3 anchor updates already landed in this commit batch.

Out:

- Promote orchestration that touches the deployed site UX in any way (no "promote candidate" badges, no "queued for promote" lists).
- Daemon / automated promote — V2.
- Multi-author conflict detection — V1 single-user assumption preserved.
- Editing canonical chain entries via CLI (`mempool drop` removes from mempool only; once an entry is promoted, the user edits `content/` directly).

## 2. Anchor Decisions (Phase 5 specifics)

| # | Decision | Rationale |
|---|---|---|
| P5-1 | Promote = a single git commit on the **local** bundle source repo | Atomic at the deploy-relevant level. No cross-repo transaction needed. |
| P5-2 | The committed delta is `content/<category>/<slug>.md` + regenerated `content/.websh/ledger.json` + regenerated `content/manifest.json` (and attestations if `--attest`) | All bundle-source artifacts move to the new state in one commit, so the deploy build sees a consistent tree. |
| P5-3 | Mempool repo cleanup happens after the local commit, as a separate best-effort GitHub Contents API call | If the local commit succeeds and the mempool drop fails, the user has duplicate entry visibility (entry in both repos) until they re-run cleanup. Failure mode is bounded and recoverable; never loses content. |
| P5-4 | The CLI uses an authenticated GitHub Contents API client (matching Phase 4's strategy) for both the read (mempool fetch) and the optional drop | Same authenticated path as the runtime. No raw.githubusercontent CDN race. |
| P5-5 | The bundle-source git commit is created via `git2` or `Command::new("git")` subprocess — to be decided in the implementation plan | Either works; subprocess avoids the `git2` binding cost and mirrors `cli/deploy.rs`'s pinata-subprocess style. |
| P5-6 | `mempool promote` does **not** push automatically. The user reviews the commit, then `git push` + `just pin` on their own cadence. | Explicit publish step preserved (master §1). The commit is local until the user wants it shared. |
| P5-7 | `mempool list` reads via the same authenticated Contents API — read-only, idempotent, requires no token (public repo) but uses one if present for rate limit | Discoverability without leaving the terminal. |
| P5-8 | Browser side keeps compose/edit and the read-only mempool view. Author-mode signal stays. The PAT-with-bundle-write requirement drops; mempool repo write is enough | Smaller browser PAT scope = smaller security surface. |

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

```
$ cargo run --bin websh-cli -- mempool promote --path writing/test [--drop-remote]
verify: writing/test exists in 0xwonj/websh-mempool@main
fetch:  writing/test (250 bytes)
write:  content/writing/test.md
ledger: 8 entries -> content/.websh/ledger.json
manifest: 10 files / 8 directories -> content/manifest.json
git:    staged 3 paths
git:    committed: promote: writing/test [a1b2c3d]

mempool drop: dropped writing/test from 0xwonj/websh-mempool   # only with --drop-remote

ready. review the commit, then `git push` and `just pin` to deploy.
```

Steps (pseudo-code):

```rust
fn promote(repo_relative_path: &str, drop_remote: bool) -> CliResult {
    // 1. Validate path is mempool-shaped
    let target = parse_promote_path(repo_relative_path)?;          // <category>/<slug>.md

    // 2. Verify mempool entry exists; reject if bundle target collides
    let body = github::get_contents_raw(MEMPOOL_REPO, target.repo_path)?;
    if local_disk_exists(target.bundle_disk_path) {
        return Err("bundle target {bundle_disk_path} already exists");
    }

    // 3. Atomic local apply
    fs::write(target.bundle_disk_path, body)?;
    let ledger = generate_content_ledger(...)?;
    let manifest = generate_content_manifest(...)?;
    if attest_all_succeeds() { /* skip if --no-attest passed */ }

    // 4. Single git commit on bundle source
    git_subprocess(["add", "content/<category>/<slug>.md", ".websh/ledger.json",
                    "content/manifest.json", /* attestations.json if regenerated */ ])?;
    git_subprocess(["commit", "-m", &format!("promote: {category}/{slug}")])?;

    // 5. Optional: drop from mempool (best-effort)
    if drop_remote {
        match github::delete_contents(MEMPOOL_REPO, target.repo_path, &mempool_head) {
            Ok(_) => println!("mempool drop: ok"),
            Err(e) => println!("mempool drop: {e} — re-run `websh-cli mempool drop` later"),
        }
    }

    println!("ready. review the commit, then `git push` and `just pin` to deploy.");
    Ok(())
}
```

Failure modes:

| Failure | Effect | Recovery |
|---|---|---|
| Mempool entry not found | Abort before any disk write | None needed |
| Bundle target already exists on disk | Abort before any disk write | User picks different slug or `git rm` the old |
| Local fs write fails (permissions, disk full) | Abort before commit | OS-level fix |
| Ledger / manifest regen errors | Abort before commit | Investigate (likely content-content malformation) |
| `git add` / `git commit` fails | No commit, partial files on disk | `git restore content/<path>` to clean up |
| (with `--drop-remote`) GitHub drop fails | Local commit OK, mempool still has entry | Re-run `websh-cli mempool drop --path <path>` |

Default is **without** `--drop-remote` so the user can review the local commit before touching mempool.

### 3.3 `websh-cli mempool drop`

```
$ cargo run --bin websh-cli -- mempool drop --path writing/test
verify: writing/test exists in 0xwonj/websh-mempool@main
github: deleted writing/test (commit f4d5e6f)
```

Single GitHub Contents API DELETE on the mempool repo. Used to clean up after a successful promote (when `--drop-remote` was not passed) or to discard a draft entirely.

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

- **Should `mempool promote` regenerate attestations?** Probably yes by default (so the deploy commit is consistent), with `--no-attest` opt-out for users without GPG configured. Decided in implementation plan.
- **`git2` crate vs subprocess?** Implementation plan picks one. Subprocess is simpler and matches deploy's pinata pattern; `git2` is more controllable. Default to subprocess.
- **Should `mempool promote` accept multiple paths?** Defer to V2. V1 single-path keeps each commit reviewable.
- **What happens if `mempool drop` fails after a successful local promote (without `--drop-remote`)?** The user re-runs `mempool drop` later. Idempotent because mempool entry won't exist on second attempt, which we handle as success.

## 11. Migration / Compatibility

- **Phase 3's design doc** stays in the repo as historical context. The master plan §6 entry for Phase 3 keeps Status=Complete; Phase 5's design references it and explains the supersession.
- **Existing browser sessions** with cached PAT scoped to bundle-write: still work for compose/edit (which only need mempool-write). The bundle-write PAT scope is dormant. Users can rotate to a narrower PAT at their own pace.
- **`tests/mempool_promote.rs`** stays for the path-mapping and message helpers (those move to CLI), but the wasm-async-flow tests are deleted.

## 12. Scheduling

This is **the next phase**. Plan to follow this design doc within the same workflow as Phases 1–4: design → plan → implementation → reviewer → master update.

If approval is given, the implementation plan should follow within this same docs/superpowers/plans/ batch.
