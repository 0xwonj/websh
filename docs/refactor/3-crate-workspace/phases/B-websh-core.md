# Phase B Design — Populate `websh-core`

## Status

Design — agreed after consensus review. See [B-review.md](./B-review.md) for the review consolidation. Canonical task list at [B-tasks.md](./B-tasks.md).

## Source

- Architecture spec: `architecture.md` §3.1, §4.
- Recon: Plan agent's punch-list (see [B-review.md § Recon findings](./B-review.md) once review lands).

## Goal

Move pure-Rust shared logic from `src/{models,core,crypto,mempool,utils}` into `crates/websh-core/src/` so that:

- `websh-core` compiles standalone for `wasm32-unknown-unknown` and the host triple.
- The legacy `src/` crate continues to build during Phase B (so each commit is green) by depending on `websh-core` and re-exporting moved items via shim modules in `src/lib.rs`.
- The end of Phase B leaves `websh-core` populated and the legacy crate hollowed-out enough for Phase C and D to consume cleanly.

## Target layout (`crates/websh-core/src/`)

```
lib.rs                       only declares submodules + re-exports
domain/                      pure data types — no I/O, no async
  mod.rs
  filesystem.rs              FsEntry, FileMetadata, NodeKind, DirEntry, paths
  manifest.rs                ContentManifestEntry, ContentManifestDocument, EntryExtensions
  mempool.rs                 MempoolStatus, Priority, MempoolFields
  changes.rs                 ChangeType, ChangeSet, Summary
  node_metadata.rs           NodeMetadata, Fields, accessors
  virtual_path.rs            VirtualPath
  explorer.rs                ExplorerNode + supporting types
  mount.rs                   RuntimeMount, MountFailure
  site.rs                    AppContext-adjacent shapes that don't touch leptos
  terminal.rs                terminal model (RingBuffer<OutputLine>, OutputLine)
  wallet.rs                  WalletState (depends on utils::format::format_eth_address)
filesystem/                  in-memory fs engine
  mod.rs
  global_fs.rs               (was core/engine/global_fs.rs)
  intent.rs                  (was core/engine/intent.rs) — wasm dep on utils::media_type_for_path
  routing.rs                 (was core/engine/routing.rs) — wasm dep on utils::dom
  content.rs                 (was core/engine/content.rs) — read_text/read_bytes via backend
  merge.rs                   (was core/merge.rs)
runtime/
  mod.rs                     populate_runtime_state etc.
  commit.rs                  (was core/runtime/commit.rs)
  loader.rs                  (was core/runtime/loader.rs)
  state.rs                   (was core/runtime/state.rs) — wasm dep on utils::dom
storage/                     hexagonal port + adapters (cfg-gated)
  mod.rs                     re-exports
  backend.rs                 StorageBackend trait, ScannedSubtree, CommitRequest, errors
  github/                    cfg(target_arch="wasm32") — gloo-net adapter
  idb.rs                     cfg(target_arch="wasm32") — IDB adapter
  persist.rs                 cfg(target_arch="wasm32") — was core/storage/persist.rs
  mock.rs                    feature-gated test adapter (mock feature)
mempool/                     pure helpers (Phase 1's home)
  categories.rs
  parse.rs
  serialize.rs
  form.rs
  path.rs
  manifest_entry.rs
  mod.rs
attestation/                 wholesale move; no split
  mod.rs                     pub use of artifact + ledger + subject
  artifact.rs                (was crypto/attestation.rs — wholesale, 115 lines)
  ledger.rs                  (was crypto/ledger.rs)
  subject.rs                 (was crypto/subject.rs — used by browser, not CLI-only)
crypto/                      narrow primitives
  eth.rs                     (was crypto/eth.rs)
  ack.rs                     (was crypto/ack.rs)
  pgp.rs                     (was crypto/pgp.rs — fingerprint + verification helpers)
shell/                       terminal interpreter (was core/commands/)
  mod.rs                     dispatcher + ExecuteCtx + SideEffect
  parser/                    (was core/parser/)
  result.rs
  autocomplete.rs
  execute/                   (was core/commands/execute.rs split per family)
    mod.rs                   shared helpers + execute_command dispatcher
    filesystem.rs            ls, cd, cat, touch, mkdir, rm, rmdir, edit, echo_redirect
    sync.rs                  sync*, mount_for_path, sync_mount_root
    env.rs                   export, unset
    listing.rs               (folded into misc — see below)
    misc.rs                  id, login/logout, clear, theme, explorer
content_routes.rs            (was utils/content_routes.rs)
utils/                       cross-platform leaf utilities
  mod.rs
  format.rs                  (was utils/format.rs)
  time.rs                    (was utils/time.rs) — cfg-gated
  ring_buffer.rs             (was utils/ring_buffer.rs)
  url.rs                     (was utils/url.rs) — needs config dep
  asset.rs                   cfg(target_arch="wasm32") (was utils/asset.rs)
  dom.rs                     cfg(target_arch="wasm32") (was utils/dom.rs)
  fetch.rs                   cfg(target_arch="wasm32") (was utils/fetch.rs)
  sysinfo.rs                 cfg(target_arch="wasm32") (was utils/sysinfo.rs)
config.rs                    (was config.rs — needed by utils/url.rs and storage adapters)
```

## What stays in legacy `src/` during Phase B

```
src/
├── lib.rs                  rewritten to declare re-export shims (`pub mod models { pub use websh_core::domain::*; }`, etc.)
├── main.rs                 unchanged (browser entry; moves to web in Phase D)
├── app.rs                  unchanged (uses websh_core::* via shim)
├── components/             entire tree — moves in Phase D
├── cli/                    entire tree — moves in Phase C
└── utils/
    ├── markdown.rs         (comrak/ammonia) — moves to web in Phase D
    ├── theme.rs            uses web_sys::window — moves to web in Phase D
    ├── breakpoints.rs      uses leptos-use — moves to web in Phase D
    └── wasm_cleanup.rs     UI-lifecycle bound — moves to web in Phase D
```

The legacy `Cargo.toml` adds `websh-core = { path = "crates/websh-core" }` as a dependency.

## Pre-move refactoring (must land before bulk moves)

### B0.a — Sever `&TerminalState` from `shell/execute_clear`

`core/commands/execute.rs::execute_clear` takes `&TerminalState` (Leptos `RwSignal<RingBuffer<OutputLine>>`) and calls `state.clear_history()`. This is the sole reason `core/commands/` cannot move into `websh-core` as-is.

**Refactor**: introduce `SideEffect::ClearHistory`. `execute_clear` returns the side effect; the caller in `components/terminal/` applies it against the signal. The signature change ripples through `execute_command` dispatcher and any other site that takes `&TerminalState` purely for clear semantics.

### B0.b — Cfg-gate `web_sys::window()` in `execute_id`

`execute_id` (line ~317 in current execute.rs) calls `web_sys::window()` to read user-agent. Wrap in `#[cfg(target_arch = "wasm32")]` with a no-op `unknown` fallback for native.

### B0.c — Break the `core::env` ↔ `runtime::state` ↔ `utils::dom` chain

Plan agent flagged a hidden wasm chain through `core/parser/lexer.rs` → `core::env` → `runtime::state` → `utils::dom`. `runtime::state` legitimately needs `utils::dom` to access browser localStorage; that's wasm-only by nature. Resolution: move `utils::dom` into `websh-core/src/utils/dom.rs` under `cfg(target_arch="wasm32")` (deviation noted in `deviation-log.md`).

## Plan-agent corrections folded in

- `crypto/subject.rs` moves to `websh-core::attestation::subject` (used by browser).
- `crypto/attestation.rs` moves wholesale (no split — only 115 lines, no signing code).
- `utils/dom.rs` moves to `websh-core::utils::dom` cfg-gated (wasm-only callers in core need it).
- `tests/mempool_model.rs` and `tests/idb_roundtrip.rs` stay in legacy (consume types that themselves stay in legacy; in Phase D they migrate to `websh-web/tests/`).

## Sequencing — task plan

12+ sub-commits inside Phase B. Each ends green. See [B-tasks.md](./B-tasks.md) for the authoritative task list; summary:

- **B0** — pre-move refactors. Six sub-commits: B0.a (sever `&TerminalState`), B0.b (cfg-gate `web_sys::window()` in execute_id), B0.c (make `dom.rs` portable), B0.d (delete `AppError`), B0.e (comment audit), B0.f (split `theme.rs`).
- **B1** — `websh-core::utils::*` + `theme` + `content_routes` + `config`. Convert `EnvironmentError`/`FetchError`/`WalletError`/`UrlValidationError` to `thiserror`.
- **B2** — `websh-core::domain::*` (all of `src/models/` plus `src/core/changes.rs`).
- **B3** — `websh-core::crypto::*` + `websh-core::attestation::*` (wholesale; subject under attestation per Plan-agent correction).
- **B4** — `websh-core::mempool::*`.
- **B5a** — `websh-core::storage::{backend, error}.rs` (port-only; `StorageError` to `thiserror`).
- **B5b** — `websh-core::filesystem::*` (engine + merge; consumes the port from B5a).
- **B6** — `websh-core::storage::{idb, persist, github, mock, boot}` (adapters, gated at parent `mod` declaration).
- **B7** — `websh-core::runtime::*` + `runtime::{env, wallet}` + `admin.rs` + `error.rs` (the surviving non-AppError types).
- **B8** — `websh-core::shell::*` with `execute/` split per family: `mod, read, write, sync, env, info`.
- **B9** — visibility audit; tighten `pub` to `pub(crate)` where not cross-crate.
- **B10** — test relocation (`tests/{commit_integration, mempool_compose, crypto_homepage}.rs` → `crates/websh-core/tests/`).

After B10, the legacy `src/lib.rs` is a thin re-export shim. Phase D dismantles it.

## Re-export shim shape (in legacy `src/lib.rs` post-B)

```rust
pub mod models {
    pub use websh_core::domain::*;
    pub mod manifest { pub use websh_core::domain::manifest::*; }
    #[cfg(test)] pub use websh_core::domain::node_metadata::test_support;
}
pub mod core {
    pub mod engine { pub use websh_core::filesystem::*; }
    pub mod runtime { pub use websh_core::runtime::*; }
    pub mod storage { pub use websh_core::storage::*; }
    pub mod commands { pub use websh_core::shell::*; }
    pub mod changes { pub use websh_core::domain::changes::*; }
    pub mod merge { pub use websh_core::filesystem::merge::*; }
}
pub mod crypto { pub use websh_core::crypto::*; pub use websh_core::attestation::*; }
pub mod mempool { pub use websh_core::mempool::*; }
pub mod utils { pub use websh_core::utils::*; pub use websh_core::content_routes; }
pub mod config { pub use websh_core::config::*; }

// these stay native to legacy until Phase D
pub mod app;
pub mod components;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
```

## Verification per task

- `cargo fmt --check`
- `cargo check -p websh-core` and `cargo check -p websh-core --target wasm32-unknown-unknown`
- `cargo check --workspace` (legacy crate still builds)
- `cargo test --workspace` (touched tests at minimum)
- `cargo clippy --workspace --all-targets`

## Risks

1. **Re-export shim subtlety**. `crate::models::test_support` (cfg(test)) must survive the shim. `pub mod manifest` (module re-export) must be a `pub mod` not a `pub use`. Plan-agent flagged both.
2. **execute.rs split helper sharing**. Cross-family helpers (`require_write_access`, `resolve_path_arg`, `change_tag`, `is_pending_create`, etc.) live in `shell/execute/mod.rs` to avoid circular file imports. Tests for those helpers stay there too.
3. **Wallet ↔ format.rs ordering**. `models/wallet.rs` depends on `utils::format`. B1 (utils) lands before B2 (domain) so the dep is satisfied.
4. **Storage adapter cfg-gating**. Each adapter is `cfg(target_arch="wasm32")`-gated; the trait + `Scanned*` records + `MockBackend` (under `mock` feature) compile on both targets.

## Out of scope for Phase B

- Removing the legacy `src/lib.rs` re-exports — Phase D's job.
- Moving `app.rs`, `components/`, `cli/` — Phases C/D.
- Cleaning up `utils/markdown.rs`, `theme.rs`, `breakpoints.rs`, `wasm_cleanup.rs` — they stay in legacy, move in Phase D.
- Browser PGP verification feature — Phase G (held).
