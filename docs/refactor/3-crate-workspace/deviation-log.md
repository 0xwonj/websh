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
