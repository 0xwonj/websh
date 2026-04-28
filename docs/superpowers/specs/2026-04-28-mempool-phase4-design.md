# Mempool — Phase 4 Design (V1 Hardening)

**Date:** 2026-04-28
**Phase:** 4 (hardening — not part of V1's original 3-phase plan; introduced after live-QA exposed correctness, bootstrap, and liveness gaps)
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Phase 1:** [`2026-04-28-mempool-phase1-design.md`](./2026-04-28-mempool-phase1-design.md) — landed
**Phase 2:** [`2026-04-28-mempool-phase2-design.md`](./2026-04-28-mempool-phase2-design.md) — landed
**Phase 3:** [`2026-04-28-mempool-phase3-design.md`](./2026-04-28-mempool-phase3-design.md) — landed

Phase 4 closes five gaps that surfaced during the first live-QA attempt. The original three-phase plan declared V1 complete after promotion; in practice, the very first compose attempt (after the user provisioned `0xwonj/websh-mempool`) silently committed two drafts to the bundle source repo (`0xwonj/websh`) instead. Investigation surfaced a chain of latent issues that require small but coordinated fixes across the storage layer, runtime, build pipeline, and tooling. After Phase 4, V1 acceptance criteria are honestly met.

## 1. Issues Closed

| # | Issue | Symptom | Phase 4 fix |
|---|---|---|---|
| G1 | Silent fallback in `backend_for_path` | Write to `/mempool/...` lands in `/` mount when `/mempool` not registered | Strict-match lookup for write contexts |
| G2 | Fresh GitHub mount has no `manifest.json` | First scan returns 404 → mount fails to load → first commit impossible | Backend treats 404 as empty tree; CLI provides explicit bootstrap |
| G3 | Compose save doesn't refresh runtime | New entry doesn't appear in mempool list until manual page reload | `save_compose` calls `reload_runtime` post-commit |
| G4 | Bundle source `manifest.json` can be stale | Newly-added mount declaration on disk is invisible to the runtime if manifest wasn't regenerated | Trunk `pre_build` hook auto-regenerates |
| G5 | No CLI surface for new mount setup | Users had to manually push manifest, write declaration JSON, regenerate manifest | `cargo run --bin websh-cli -- mount init` |

## 2. Anchor Decisions

| # | Decision | Rationale |
|---|---|---|
| P4-1 | Manifest layout stays "per-repo, in-repo" | Atomic wasm commits already maintain per-repo manifest; centralizing into bundle source would break master plan A3 (`/site` and mempool as separate trees) and force every compose to be cross-repo |
| P4-2 | `backend_for_path` retains longest-prefix fallback **only for reads**; new `backend_for_mount_root` enforces exact match for writes | Reads tolerating fallback is acceptable (worst case: stale view); writes silently routing to the wrong repo is a correctness bug |
| P4-3 | Empty repo (`manifest.json` missing) = `Ok(empty ScannedSubtree)` | Self-heals fresh mounts; first commit will atomically create the manifest. Other 4xx/5xx still error |
| P4-4 | `save_compose` mirrors `promote_entry` post-commit (apply outcome → reload runtime) | Phase 2/3 inconsistency was the source of the "no UI change after save" report |
| P4-5 | Bundle manifest regenerated on every Trunk build (pre_build hook) | Stale manifest in committed history is no longer a possible state |
| P4-6 | `mount init` subcommand handles bootstrap; repo creation is out of scope | `gh repo create` already covers the latter with full options. `mount init` focuses on websh-specific setup |

## 3. Implementation Surface

### 3.1 Backend: 404-tolerant scan (G2)

`src/core/storage/github/client.rs::load_manifest_snapshot` — if HTTP 404, return `Ok(ScannedSubtree::default())` instead of `Err(NotFound)`. Other non-2xx still maps via `map_http_status`.

### 3.2 Backend: strict mount-root match (G1)

`src/app.rs`:

```rust
pub fn backend_for_path(...)            // unchanged — reads
pub fn backend_for_mount_root(&self, root: &VirtualPath) -> Option<Arc<dyn StorageBackend>> {
    self.backends.with_value(|map| map.get(root).cloned())
}
```

Call-sites migrated to the strict variant: `compose.rs::save_compose`, `promote.rs::promote_entry` (both backends), `promote.rs::retry_mempool_drop`, `terminal.rs::SideEffect::Commit`. Error messages name the specific mount root and point at the registration prerequisites.

### 3.3 Compose: post-commit runtime reload (G3)

`src/components/mempool/compose.rs::save_compose` — after `apply_commit_outcome`, run `reload_runtime + apply_runtime_load`. Best-effort: a reload failure logs a warning but does not poison the already-successful commit. Mirrors `promote_entry`.

### 3.4 Build: pre_build manifest hook (G4)

`Trunk.toml` gains a `pre_build` hook running `cargo run --bin websh-cli -- content manifest`. Stylance hook runs first (CSS) and the manifest hook runs second. Both run on every `trunk serve` and `trunk build`.

### 3.5 CLI: `mount init` (G5)

`src/cli/mount.rs` (new): `MountInit` subcommand. Flow:

1. Verify `gh` CLI is available and authenticated.
2. Verify the target repo exists (`gh api repos/{repo}`).
3. Compute `manifest_repo_path` from the `--root` flag (root → `manifest.json`; subdir → `<dir>/manifest.json`).
4. Check if the manifest already exists; if not, push `{"files":[],"directories":[]}` via `gh api ... -X PUT`.
5. Write `content/.websh/mounts/<name>.mount.json` with the declaration.
6. Call `generate_content_manifest` to refresh the bundle manifest.

Idempotent across all five steps. Three unit tests cover the path helpers; the GitHub-dependent steps are exercised live.

## 4. Test Strategy

### 4.1 Unit / integration

- `cli::mount::tests::*` — three tests for path computation.
- All existing test suites (`cargo test --lib`, `mempool_compose`, `mempool_promote`, `mempool_model`, `commit_integration`) pass without modification: 477 + 14 + 12 + 1 + 1.

### 4.2 Wasm typecheck

`cargo check --target wasm32-unknown-unknown --lib` — clean.

### 4.3 Live end-to-end QA (run by user, not by automation)

1. Repo `0xwonj/websh-mempool` already exists with README; no setup needed.
2. `export GITHUB_TOKEN=github_pat_...`
3. `cargo run --bin websh-cli -- mount init --name mempool --repo 0xwonj/websh-mempool --mount-at /mempool --branch main --writable`
4. Verify `manifest.json` exists in mempool repo; verify `content/manifest.json` includes the mounts directory.
5. `trunk serve` — pre_build hook runs cleanly.
6. `sync auth set <PAT>` in the in-page terminal.
7. Compose new draft → save: GraphQL POSTs (head fetch + commit mutation) succeed, modal closes, mempool list refreshes immediately.
8. Click the new entry: edit modal pre-fills, save updates the file, list refreshes.
9. Promote: confirm modal → confirm → bundle add commit + mempool drop commit → deploy hint banner appears, entry disappears from mempool.
10. Verify the new entry now lives at `0xwonj/websh:content/<category>/<slug>.md`, no longer in `0xwonj/websh-mempool`.

## 5. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| `gh` CLI not installed on the user's machine | `mount init` returns a clear error pointing at https://cli.github.com |
| `gh` authenticated as the wrong account | `gh api repos/...` returns 404 → CLI surfaces the "create the repo first" message |
| `mount init` runs while Trunk is also rebuilding | Both write to `content/manifest.json`; last-write-wins. Acceptable given the manual setup nature |
| User forgets to restart `trunk serve` after `mount init` | Eprintln reminder at the end of `init`; runtime won't pick up the new mount otherwise |
| Pre_build hook adds Trunk build latency | `cargo run --quiet` caches between builds; first run slow, subsequent runs ~tens of ms |
| Strict-match breaks an unforeseen write callsite | All write callsites enumerated and migrated; tests pass; live QA in §4.3 covers the integration |

## 6. Acceptance Criteria

Phase 4 is complete when:

1. All five gaps (G1–G5) are addressed by the change set.
2. `cargo test --lib && cargo test --test mempool_{compose,promote,model} && cargo check --target wasm32-unknown-unknown --lib` — all green.
3. The user runs the live QA flow in §4.3 and reports each step PASS.
4. Master plan §10 Decision Log records the Phase 4 entry.

V1 acceptance (master §8) is genuinely met when both Phase 3 and Phase 4 land and the live QA flow succeeds.

## 7. Out of Scope (V2 candidates)

- A `mount remove` subcommand (manual `rm` + `content manifest` is fine for V1).
- A `mount status` / `mount list` subcommand for inspecting mount state.
- Repo creation inside `mount init` (intentionally deferred — `gh repo create` is the right tool).
- Automatic mount discovery from a centralized registry (no such concept in V1).
- `manifest.json --check` mode for CI gate (Trunk hook makes drift impossible at build time).
