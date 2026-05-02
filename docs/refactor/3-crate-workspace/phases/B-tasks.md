# Phase B Tasks

Canonical ordered task list for Phase B execution. Derived from `B-websh-core.md` after consensus review (`B-review.md`). One task per intended commit. Each commit ends with `cargo check --workspace` (both targets where relevant) green and touched tests passing.

Task labels (B0..B10) are doc-internal — they do **not** appear in commit messages. Commits use Conventional Commits per `conventions.md`.

## B0 — Pre-move refactoring

Lands all the prerequisites that block the bulk file moves. One commit per sub-task; each is independent.

### B0.a — Sever `&TerminalState` from shell

- Add `SideEffect::ClearHistory` to `core::commands::result::SideEffect`.
- Replace `Command::Clear` body in `execute.rs` with `SideEffect::ClearHistory`.
- Drop `state: &TerminalState` parameter from `execute_command` and `execute_pipeline`.
- Remove `TerminalState::new()` construction at every test fixture (~25 sites).
- Remove `use crate::app::TerminalState` imports from execute.rs and commands/mod.rs.
- Caller in `components/terminal/terminal.rs:155` applies `SideEffect::ClearHistory` against the signal.
- Verify all tests still pass.

Commit: `refactor(shell): emit clear-history as a side effect instead of mutating terminal state directly`

### B0.b — Cfg-gate `web_sys::window()` in `execute_id`

- Switch `execute.rs::execute_id` from `web_sys::window()` to `dom::window()` (which has the host-stub).
- Use `if let Some(window) = dom::window() { ... }` for user-agent extraction; emit `unknown` on native.
- Verify tests pass.

Commit: `refactor(shell): use dom helper for user-agent lookup so id command runs on native`

### B0.c — Make `dom.rs` portable across both targets

- Audit `src/utils/dom.rs`: every public function must compile on both targets.
- Wasm-touching function bodies guard with `#[cfg(target_arch="wasm32")]` and provide a native no-op or empty-string fallback (extends the existing `dom::window()` shape).
- Verify `cargo check --target wasm32-unknown-unknown` and host `cargo check` both pass on the legacy crate.

Commit: `refactor(utils): make dom helpers compile on native with no-op fallbacks`

### B0.d — Delete `AppError`

- Remove `src/core/error.rs::AppError` and its `From<DomainError> for AppError` impls.
- Re-type each cross-domain caller of `AppError` to either the relevant per-domain error or a `String` per `conventions.md § Error handling`.
- Verify no `AppError` references remain (`grep -rn AppError`).

Commit: `refactor(core)!: delete AppError; route per-domain errors directly to callers`

### B0.e — Comment audit on in-scope files

- Strip banner-block comments (`// ============================================================================` style) from `ring_buffer.rs`, `execute.rs:432, 1322, 2126`, and any other in-scope file.
- Strip restate-the-code comments from `execute.rs::execute_id` (lines 276, 300, 311, 316).
- Strip development-history narration from `runtime/loader.rs:253-258`.
- Run `cargo fmt`; tests must still pass.

Commit: `chore(core): scrub banner-block and history-narration comments before move`

### B0.f — Split theme.rs

- Extract pure helpers (`THEMES`, `ThemeDescriptor`, `theme_ids`, `theme_label`, `normalize_theme_id`, `DEFAULT_THEME`, `STORAGE_KEY`) into a new module — destination is `websh-core::theme` once B1 lands.
- Leave `apply_theme` and `initial_theme` (web-sys-bound) in `src/utils/theme.rs` for Phase D to migrate to `websh-web::platform`.
- Update `core/commands/execute.rs:19` import accordingly.

Commit: `refactor(utils): split theme into pure helpers and web-only application`

## B1 — Populate `websh-core::utils` + `theme` + `content_routes` + `config`

Move clean utility modules first; they're the foundation everything else depends on.

- Move `src/utils/{format,time,ring_buffer,url,asset,dom,fetch,sysinfo}.rs` to `crates/websh-core/src/utils/`.
- Move `src/utils/content_routes.rs` to `crates/websh-core/src/content_routes.rs` (or `utils/content_routes.rs`).
- Move `src/config.rs` to `crates/websh-core/src/config.rs`.
- Move pure `theme.rs` content (B0.f's extraction) to `crates/websh-core/src/theme.rs`.
- Convert `EnvironmentError`, `FetchError`, `WalletError`, `UrlValidationError` to `thiserror::Error`.
- Set up shim re-exports in legacy `src/utils/mod.rs`, `src/lib.rs`.
- Cfg-gate at parent `mod` declaration for `asset`, `fetch`, `sysinfo`. Do NOT cfg-gate `dom`, `time` — they have internal arms.
- Audit `#[allow(dead_code)]`: delete or justify in-place per move.
- Verify both targets compile.

Commit: `refactor(core): move pure utilities and config into websh-core`

## B2 — Populate `websh-core::domain`

Move all of `src/models/` plus `src/core/changes.rs` (per the architecture review's correction).

- Move `src/models/{filesystem,manifest,mempool,virtual_path,explorer,mount,node_metadata,site,terminal,wallet}.rs` to `crates/websh-core/src/domain/`.
- Move `src/core/changes.rs` to `crates/websh-core/src/domain/changes.rs`.
- Drop `#[cfg(test)]` from `node_metadata::test_support`; keep `#[allow(dead_code)]`.
- Set up shim: `pub mod models { pub use websh_core::domain::*; pub mod manifest { pub use websh_core::domain::manifest::*; } }`.
- Verify both targets compile.

Commit: `refactor(core): move domain types into websh-core::domain`

## B3 — Populate `websh-core::crypto` + `attestation`

- Move `src/crypto/{eth,ack,pgp}.rs` to `crates/websh-core/src/crypto/`.
- Move `src/crypto/{attestation,ledger,subject}.rs` to `crates/websh-core/src/attestation/{artifact,ledger,subject}.rs` (no split; wholesale).
- Update `crypto/pgp.rs` to use `pgp = { features = ["wasm"] }` for verification helpers; signing remains absent (signing stays in `websh-cli` Phase C).
- Set up shim: `pub mod crypto { pub use websh_core::crypto::*; pub use websh_core::attestation::*; }`.
- Verify both targets compile.

Commit: `refactor(core): move crypto primitives and attestation into websh-core`

## B4 — Populate `websh-core::mempool`

- Move `src/mempool/` (entire folder — Phase 1's existing structure) to `crates/websh-core/src/mempool/`.
- Set up shim: `pub mod mempool { pub use websh_core::mempool::*; }`.
- Tests in `tests/mempool_compose.rs` follow in B10.
- Verify both targets compile.

Commit: `refactor(core): move mempool helpers into websh-core`

## B5a — Storage port (no engine dep)

- Move `src/core/storage/{backend,error}.rs` to `crates/websh-core/src/storage/{backend,error}.rs`.
- Move `src/core/storage/mod.rs`'s port-relevant items (StorageBackend trait, ScannedSubtree/ScannedFile/ScannedDirectory, CommitRequest/CommitOutcome, error types) — adapter modules stay in legacy until B6.
- Convert `StorageError` to `thiserror::Error`.
- Verify both targets compile.

Commit: `refactor(core): move storage port (trait + records) into websh-core`

## B5b — Filesystem engine

- Move `src/core/engine/{global_fs,intent,routing,content}.rs` to `crates/websh-core/src/filesystem/`.
- Move `src/core/merge.rs` to `crates/websh-core/src/filesystem/merge.rs`.
- Imports point at `websh-core::storage::{StorageBackend, ScannedSubtree, ...}` (now in core after B5a).
- Set up shim: `pub mod core { pub mod engine { pub use websh_core::filesystem::*; } pub mod merge { pub use websh_core::filesystem::merge::*; } pub mod changes { pub use websh_core::domain::changes::*; } }`.
- Verify both targets compile.

Commit: `refactor(core): move filesystem engine into websh-core`

## B6 — Storage adapters

- Move `src/core/storage/{idb,persist,boot}.rs` and `src/core/storage/github/` to `crates/websh-core/src/storage/`.
- Move `src/core/storage/mock.rs` (gated by `mock` feature).
- Cfg-gate at parent `mod` declaration: `#[cfg(target_arch="wasm32")] pub mod github;`, etc.
- `boot.rs` lands here (depends on engine which is in B5b).
- Update shim: `pub mod core { pub mod storage { pub use websh_core::storage::*; } }`.
- Drop `#[allow(dead_code)]` annotations that are no longer dead.
- Verify both targets compile.

Commit: `refactor(core): move storage adapters into websh-core`

## B7 — Runtime + admin

- Move `src/core/runtime/{commit,loader,state,mod}.rs` to `crates/websh-core/src/runtime/`.
- Move `src/core/wallet.rs` and `src/core/env.rs` into `crates/websh-core/src/runtime/{wallet,env}.rs` (unwinding the `#[path = "../..."]` aliases).
- Move `src/core/admin.rs` to `crates/websh-core/src/admin.rs` (top-level of core).
- Move `src/core/error.rs` to `crates/websh-core/src/error.rs` (the file as a whole — only `WalletError`, `EnvironmentError`, `FetchError` remain after B0.d deleted `AppError`).
- Update shim: `pub mod core { pub mod runtime { pub use websh_core::runtime::*; pub use websh_core::runtime::env; pub use websh_core::runtime::wallet; } pub mod error { pub use websh_core::error::*; } pub mod admin { pub use websh_core::admin::*; } }`.
- Verify both targets compile.

Commit: `refactor(core): move runtime, admin, and remaining error types into websh-core`

## B8 — Shell + execute family split

- Move `src/core/commands/{mod,parser,result,filters,autocomplete}.rs` and the `parser/` directory to `crates/websh-core/src/shell/`.
- Split `src/core/commands/execute.rs` into:
  - `shell/execute/mod.rs` — shared helpers (`resolve_path_arg`, `require_write_access`, `mount_for_path`, `is_synthetic_runtime_state_path`, `execute_command` dispatcher) + production-side glue.
  - `shell/execute/read.rs` — `execute_ls`, `execute_cd`, `execute_cat`, `format_ls_output` (read-side filesystem) + their tests + `can_write_path` if used by listing.
  - `shell/execute/write.rs` — `execute_touch`, `execute_mkdir`, `execute_rm`, `execute_rmdir`, `execute_edit`, `execute_echo_redirect` (write-side filesystem) + their tests + `is_pending_create`, `blank_file_meta`, `blank_dir_meta`.
  - `shell/execute/sync.rs` — `execute_sync*`, `mount_for_path`, `sync_mount_root`, `change_tag` + tests.
  - `shell/execute/env.rs` — `execute_export`, `execute_unset` + tests.
  - `shell/execute/info.rs` — `execute_id`, `execute_whoami`, `execute_theme`, `execute_login`, `execute_logout`, `execute_clear`, `execute_explorer` + tests.
- Update shim: `pub mod core { pub mod commands { pub use websh_core::shell::*; pub use websh_core::shell::{Command, SideEffect, AutocompleteResult, parse_input, autocomplete, get_hint}; pub use websh_core::shell::DirEntry; } }` and ensure top-level flat re-exports needed by `src/components/editor/modal.rs` etc. are preserved.
- Verify each split file is < 800 lines including tests; `cargo check --workspace`; `cargo test --workspace`.

Commit: `refactor(shell): move command interpreter into websh-core with execute split per family`

## B9 — Visibility audit

- Audit every `pub` in `crates/websh-core/src/{utils,domain,crypto,attestation,mempool,filesystem,runtime,storage,shell,theme,admin}/mod.rs`.
- Downgrade to `pub(crate)` items not consumed across crates.
- Confirm `pub use` re-exports at `lib.rs` level expose only what `websh-cli` and `websh-web` will need.
- Verify nothing breaks; commit.

Commit: `refactor(core): tighten visibility on items not consumed across crates`

## B10 — Test relocation

- Move `tests/commit_integration.rs`, `tests/mempool_compose.rs`, `tests/crypto_homepage.rs` to `crates/websh-core/tests/`.
- Adjust import paths (`websh::...` → `websh_core::...`).
- `tests/mempool_model.rs` and `tests/idb_roundtrip.rs` stay in legacy `tests/` (consume types in legacy until Phase D moves them).
- `cargo test --workspace` and `cargo test --features mock --test commit_integration` both green.

Commit: `test(core): relocate integration tests for moved subsystems`

## Phase B wrap-up

- Run wrap-up checklist (`workflow.md § Wrap-up checklist`).
- Dispatch ≥3 review agents: goal-achievement, principles + idioms, conventions + code-quality.
- Reconcile findings; fix CRITICAL/HIGH before declaring B done.
- Append wrap-up section to `B-review.md`.
- Update Status table in `README.md`.
- Write ADR-0002 (placement decisions: subject, dom, theme, error type shape) and ADR-0003 (execute/ family split).
- `advisor()` for sanity check before declaring B done.
- Status note: `Phase B done. ~12 commits. All checks pass. <deviations>.`

## Verification at every task boundary

```
cargo fmt --check
cargo check -p websh-core
cargo check -p websh-core --target wasm32-unknown-unknown
cargo check --workspace
cargo check -p websh-web --target wasm32-unknown-unknown   # post-B5b once filesystem moves
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --features mock --test commit_integration       # post-B10
```
