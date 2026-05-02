# Deviation Log

Append-only record of every place execution diverged from `architecture.md`. Newest first.

Each entry: date, phase, what changed, why, ADR reference if material.

For the format and decision criteria, see [workflow.md § Deviation rules](./workflow.md#deviation-rules).

---

## 2026-05-03 · Phase B · scope refinements after Plan-agent recon

Three architecture corrections surfaced when the Plan agent walked the actual codebase against the doc's plan:

1. **`utils/dom.rs` moves to `websh-core` under `cfg(target_arch="wasm32")`**, not to `websh-web` as the architecture doc's "files that stay in legacy" list said. Reason: `runtime/state.rs`, `engine/routing.rs`, `engine/intent.rs` consume `utils::dom` and they must live in core. Cfg-gating dom in core is consistent with the architecture's existing pattern for `storage::github` / `storage::idb`.

2. **`crypto/subject.rs` moves to `websh-core` (with `attestation/`)**, not to `websh-cli`. Reason: `components/home/mod.rs` and `components/shared/signature_footer.rs` consume `Subject` for browser-side rendering. The architecture doc's "subject is build-time only" claim was wrong.

3. **`crypto/attestation.rs` moves wholesale**, not split. Reason: the file is 115 lines with no signing code (signing already lives in `cli/{attest.rs, pgp.rs, ack.rs}`). The architecture's "split artifact loading from signing" prescription was based on the wrong size assumption.

Pre-move refactor (`shell/`-bound prerequisite): `core/commands/execute.rs::execute_clear` takes `&TerminalState` (a Leptos signal). Refactor to emit `SideEffect::ClearHistory`; caller in `components/terminal/` applies the effect. Also `cfg(target_arch="wasm32")`-gate the `web_sys::window()` call in `execute_id`. Required so `shell/` compiles in `websh-core` for both targets.

Tests follow their owners: `tests/idb_roundtrip.rs` and `tests/mempool_model.rs` stay in `websh-web/tests/` (they consume types that themselves stay in `websh-web`); `tests/commit_integration.rs` and `tests/mempool_compose.rs` move to `websh-core/tests/`.

ADR: 0002 covers the dom.rs and subject.rs placement decisions; the shell pre-move refactor and execute.rs split get their own ADR (0003) at Phase B wrap-up.

## 2026-05-03 · Phase B · post-review consolidation

After consensus review (`phases/B-review.md`), the design picks up additional resolutions:

- `dom.rs` is **internally cfg-gated** with native no-ops; the module compiles on both targets. Earlier wording suggested wholesale `cfg(target_arch="wasm32")` on the file, which would break `routing.rs` and `runtime/state.rs` on native.
- `theme.rs` is **split**: pure constants/helpers move to `websh-core::theme`; `apply_theme` and `initial_theme` (web-sys-bound) stay in the legacy crate and migrate to `websh-web::platform` in Phase D.
- `AppError` (in `src/core/error.rs`) is **deleted** in B0.d. Per-domain error types (`WalletError`, `EnvironmentError`, `FetchError`, `StorageError`, `UrlValidationError`) get `thiserror::Error` rewrites folded into their move commits — not deferred.
- `core/changes.rs` lands in `domain/`, not `filesystem/`. Pure data type, used independently of the engine.
- The B5/B6 sequencing splits: B5a moves the storage port (no engine dep), B5b moves the filesystem engine (consumes the port), B6 moves storage adapters. Earlier sequencing was impossible because `filesystem/content.rs` depends on `StorageBackend`.
- The `execute/` family split adds `read.rs` + `write.rs` (separating filesystem-read from filesystem-write) so neither file passes the 800-line ceiling once tests are included. `info.rs` replaces `misc.rs` as the post-rename family for `id`/`whoami`/`theme`/`login`/`logout`/`clear`/`explorer`. `listing.rs` goes away — `ls`/`format_ls_output` live in `read.rs`.
- `node_metadata::test_support` loses its `#[cfg(test)]` gate. `cfg(test)` is per-crate in Cargo; the gate prevents the legacy crate's tests from consuming the symbol via the shim.
- Re-export shim is enumerated explicitly (in `B-tasks.md`'s per-step shim updates) rather than wildcarded blindly. Wildcards are reserved for the type-only re-export at the leaf.
- Pre-existing 800+ line files (`global_fs.rs`, `commands/mod.rs`, `autocomplete.rs`, `ack.rs`, `ledger.rs`) move wholesale without splitting. Pre-existing condition; not introduced by Phase B; logged here for tracking. Splits become follow-up issues post-migration.
- `tests/crypto_homepage.rs` moves to `crates/websh-core/tests/` in B10 alongside `commit_integration.rs` and `mempool_compose.rs`.
- Visibility audit added as B9 (downgrade `pub` to `pub(crate)` where items aren't cross-crate consumed).
- Comment audit is part of B0.e (banner-block separators, restate-the-code, development-history narration scrubbed before move).

## 2026-05-03 · Phase B · B0-B4 landed; B5+ deferred to next session

After 9 commits on `refactor/3-crate-workspace`, the migration paused at the B4/B5 boundary. Landed:

- Phase A (workspace skeleton) — 1 commit.
- B0 pre-move refactoring (4 commits): `SideEffect::ClearHistory` severs `&TerminalState` from shell; `dom::window()` replaces direct `web_sys::window()` in `execute_id`; `AppError` deleted; banner-block comments scrubbed across 16 files. (B0.c and B0.f were satisfied by existing code structure.)
- B1+B2 merged commit: `domain/`, `utils/`, `config`, `content_routes`, `theme` (split — pure helpers to core, apply/init stay in legacy) all moved into `websh-core`. Forced merge because `config::BootstrapSiteSource` depends on `models::VirtualPath` and `models::wallet` depends on `utils::format`.
- B3: `crypto/` (ack, eth, pgp) and `attestation/` (artifact, ledger, subject) moved.
- B4: `mempool/` moved.

B5+ attempted in this session but reverted: bulk-moving `core/{engine,runtime,storage,commands,parser,changes,merge,admin,env,wallet,error,autocomplete}.rs` to their websh-core homes exposes deep Leptos coupling that the recon underestimated. Specifically:

- `core/wallet.rs` uses Leptos `RwSignal` and reaches into `crate::app::AppContext` from inside the engine layer (15+ lines of `ctx.foo.set(...)` calls).
- `core/runtime/state.rs` similarly threads through `AppContext`.
- `core/storage/persist.rs` uses `web_sys::console::error_1` (needs `Console` web-sys feature).
- `core/storage/idb.rs` and `core/storage/persist.rs` reference `crate::core::changes::ChangeSet` which moves to `domain/changes.rs` requiring synchronized internal-import rewrites.
- `core/commands/execute.rs` hardcodes `crate::DirEntry` (referring to a flat re-export in legacy `core/mod.rs:19`) and a now-stale `crate::core::env`/`wallet` path.

These mean B5+ requires more pre-move refactoring than B0 anticipated — separating the pure engine state from the Leptos-bound state in `wallet.rs` and `state.rs` is a multi-commit effort comparable in scope to B0 itself.

**Resumption plan** (next session):

1. Pre-move refactor `core/wallet.rs`: extract Leptos-bound bits into a new `components/wallet.rs` (or similar), leaving `runtime::wallet` with only pure session/connection logic.
2. Pre-move refactor `core/runtime/state.rs` similarly: pure types + state mutations, no `AppContext` reach-through.
3. Add `Console` and `HtmlElement`-related web-sys features to `websh-core`'s Cargo.toml.
4. Then B5a (storage port + error) → B5b (filesystem engine) → B6 (storage adapters) → B7 (runtime + admin + error) → B8 (shell + execute split) → B9 (visibility audit) → B10 (test relocation) per the plan.
5. Wrap-up review (3+ agents) per `workflow.md`.

The 9 committed commits are all green (`cargo test --workspace` 593+ tests pass, clippy clean). The migration's documents (`README.md`, `architecture.md`, `workflow.md`, `conventions.md`, `principles.md`, `phases/B-*.md`, `adrs/`, this log) remain authoritative for the next session to pick up from.

## 2026-05-03 · Phase B · resumption execution: B5-B10 landed

Resumed and completed Phase B's bulk move:

- `refactor(wallet): split appcontext-bound orchestration from pure wallet primitives` — extracted `connect_with_session` and `disconnect` (the only two Leptos-bound functions in `core/wallet.rs`) into a new `components/wallet.rs`. `core::wallet` is now pure web-sys + ENS + session primitives.
- `refactor(core): move filesystem engine, runtime, storage, and shell into websh-core` — single bulk-move commit. `core/{engine, runtime, storage, commands, parser, autocomplete, admin, env, wallet, error, merge, changes}` all migrate. `engine/` renames to `filesystem/`. `commands/` renames to `shell/`. The `#[path = "../env.rs"]` and `#[path = "../wallet.rs"]` aliases unwound. Adapters under `storage/{github, idb, persist}` cfg-gated at the parent `mod` declaration. `web-sys` adds the `console` feature.
- `test(core): relocate engine integration tests into websh-core/tests` — `commit_integration`, `mempool_compose`, `crypto_homepage` move to `crates/websh-core/tests/` with imports rewritten to `websh_core::*` directly. `[[test]] required-features = ["mock"]` moves with them; legacy crate's `mock` feature forwards to `websh-core/mock`. `pgp = "0.19"` joins websh-core's dev-deps for the homepage verification test.

`execute.rs` family split (B8 originally) deferred — moved as one wholesale file under `shell/execute.rs` rather than splitting into `read/write/sync/env/info`. Pre-existing 800+ line file; not introduced by this migration. Tracked as a follow-up after the migration lands.

B9 visibility audit also deferred to a follow-up. The legacy crate's `pub mod core` shim re-exports broad sets via wildcards; tightening to `pub(crate)` for items not actually consumed across crates is a polish step.

Net Phase B state: 13 commits on `refactor/3-crate-workspace`. `cargo test --workspace` 616 tests pass (125 legacy + 468 websh-core + 23 integration). `cargo clippy --workspace --all-targets` clean. `cargo check -p websh-core --target wasm32-unknown-unknown` clean.

Phases C, D, E, F remain. Phase C (CLI engine extraction from clap shims) is the next natural unit; Phase D (move `app.rs`, `components/`, `main.rs` to `crates/websh-web/`) follows; Phase E reconfigures Trunk; Phase F documents the result.
