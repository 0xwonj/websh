# Phase H Design — Cross-target cfg discipline + thiserror sweep (revised)

## Status

Design — agreed after consensus review. See [B-review.md](./B-review.md)-style consolidation in `deviation-log.md` (Phase H section). Three review agents (architecture, idioms, conventions) ran in parallel; their findings are folded in below.

## Goal

Make `websh-core` truly target-portable (independent native build with wasm crates absent) and convert all hand-rolled `Display + Error` impls to `thiserror::Error` with per-domain placement.

## Sub-tasks

### H1 — Cfg-gate wasm-only deps in `websh-core/Cargo.toml`

Move from plain `[dependencies]` to `[target.'cfg(target_arch = "wasm32")'.dependencies]`:
`wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys`, `gloo-net`, `gloo-timers`, `idb`, `serde-wasm-bindgen`.

H1 must land together with H2 (deps gated without modules gated → compile fails).

### H2 — Module-level surgery to support cfg-gated deps

**Extract pure pieces into portable homes**:

- `runtime::wallet::chain_name(u64) -> &'static str` is pure (match over u64); move to a new portable submodule `runtime::wallet_chain` (no breaking-change of public path: `runtime::wallet` re-exports it on wasm32 only). Actually simpler: move to `domain::wallet::chain_name`. Existing `domain::WalletState` enum is the natural neighbor.
- `utils::sysinfo::get_uptime() -> Option<String>` becomes internally cfg-gated (`None` on host).

**`utils::dom` made portable via host stubs**:

`utils/dom.rs` top-level `use wasm_bindgen::JsCast; use web_sys::{Storage, Window};` becomes cfg-armed:

```rust
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
#[cfg(target_arch = "wasm32")]
use web_sys::{Storage, Window};

#[cfg(not(target_arch = "wasm32"))]
type Window = ();
#[cfg(not(target_arch = "wasm32"))]
type Storage = ();
```

Public signatures (`pub fn window() -> Option<Window>` etc.) stay the same; on host they resolve to `Option<()>`.

**Parent-mod cfg-gating (genuinely wasm-only modules)**:

- `utils/mod.rs`: `#[cfg(target_arch = "wasm32")] pub mod fetch;` + cfg-gate the `pub use fetch::{RaceResult, fetch_content, fetch_json, race_with_timeout}` re-exports.
- `runtime/mod.rs`: `#[cfg(target_arch = "wasm32")] pub mod wallet;` (after `chain_name` extraction).
- `storage/mod.rs`: `#[cfg(target_arch = "wasm32")] pub mod {idb, persist};`.
- `storage/github/mod.rs`: gate only `mod client;` and `pub use client::GitHubBackend;` — `manifest`, `graphql`, `path` stay unconditional.

**Sysinfo decision**: `get_uptime` becomes the internal-arm pattern (returns `None` on host) rather than parent-gated, because `shell::execute::execute_id` consumes it on both targets.

**Fix `shell/execute.rs` knock-ons**:

- Line 16: `use crate::runtime::{env, wallet};` → drop `wallet` import (chain_name moves to domain).
- Line 301: `wallet::chain_name(chain_id)` → `crate::domain::wallet::chain_name(chain_id)` or `crate::domain::chain_name(chain_id)`.
- Line 308: `sysinfo::get_uptime()` — already returns `Option<String>`; the call site already handles None gracefully via `if let Some(uptime) = ...`. No change needed once `get_uptime` is portable.

### H3 — Split `storage/boot.rs` cleanly across the layering

`storage/boot.rs` mixes pure helpers with wasm-bound bootstrappers. Split:

**Pure helpers → new `runtime/boot.rs`** (portable, both targets):
- `bootstrap_runtime_mount() -> RuntimeMount`
- `bootstrap_global_fs() -> GlobalFs`
- `seed_bootstrap_routes(_global: &mut GlobalFs)`
- `is_canonical_mount_root(path: &VirtualPath) -> bool`

**Wasm-bound bootstrappers stay in `storage/boot.rs`** (which becomes wasm-only):
- `build_backend_for_bootstrap_site(...)`
- `build_backend_for_declaration(...)`
- `hydrate_drafts(draft_id: &str)`
- `hydrate_remote_head(mount_id: &str)`

Then in `storage/mod.rs`: `#[cfg(target_arch = "wasm32")] pub mod boot;` (alongside idb/persist).

`runtime/loader.rs` updates:
- Pure helpers imported unconditionally from `runtime::boot::*`.
- Wasm-bound calls (`build_backend_for_*`, `hydrate_remote_head`) wrapped in `#[cfg(target_arch = "wasm32")]` arms within loader.

### H4 — `thiserror` conversion + per-domain error placement

**Eight error types** (corrected list from the agreed reviewer findings):

| Type | Old location | New location | Rename? |
|---|---|---|---|
| `WalletError` | `error.rs` | `runtime/wallet.rs` (becomes wasm-gated; OK because all consumers are wasm-side) | no |
| `EnvironmentError` | `error.rs` | `runtime/env.rs` | no |
| `FetchError` | `error.rs` | `utils/fetch.rs` (cfg-gated with the module) | no |
| `StorageError` | `storage/error.rs` | stays | no |
| `EthVerifyError` | `crypto/eth.rs` | stays | no |
| `AckError` | `crypto/ack.rs` | stays | no |
| `shell::parser::ParseError` | `shell/parser/mod.rs` | stays | rename to `ShellParseError` |
| `domain::virtual_path::ParseError` | `domain/virtual_path.rs` | stays | rename to `VirtualPathParseError` |
| `UrlValidationError` | `utils/url.rs` | stays | no — but add missing `Error` impl (it has Display, no Error) |

**Delete `crates/websh-core/src/error.rs` entirely** after relocations.

**Display string cleanup** during conversion (per `conventions.md § Error handling`: lowercase, no trailing period, no internal jargon):
- `WalletError::NoWindow`: "Browser window not available" → "browser window not available"
- `WalletError::NotInstalled`: "No EIP-1193 wallet detected. Please install a browser wallet extension." → "no wallet provider detected; install a browser wallet extension"
- `WalletError::RequestCreationFailed`: "Failed to create wallet request" → "failed to create wallet request"
- `WalletError::RequestRejected({0})`: "Wallet request rejected: {0}" → "wallet request rejected: {0}"
- `FetchError`: lowercase all messages, drop trailing periods.
- (StorageError, EnvironmentError, EthVerifyError, AckError, both ParseErrors are already convention-compliant — preserve.)

**`StorageError::Conflict` SHA truncation**: use literal-arg form:
```rust
#[error("remote changed (now {}). run 'sync refresh'", short_sha(remote_head))]
Conflict { remote_head: String },
```
or keep `#[error(transparent)]` + manual Display arm. Test `crates/websh-core/src/storage/error.rs::tests::test_conflict_display` must pass unchanged.

**Drop per-variant `///` doc-comments** that just paraphrase the variant name (`error.rs:13-25, 45-55, 76-94`) — `#[error("…")]` carries the message; the variant name names the variant.

## Sequencing & commits

| Commit | Scope | Verification |
|---|---|---|
| 1 | H1 + H2 (Cargo.toml gates + module surgery + chain_name extraction) | host `cargo check -p websh-core` clean; wasm32 `cargo check -p websh-core --target wasm32-unknown-unknown` clean; `cargo test --workspace` green |
| 2 | H3 (split boot, relocate pure helpers to runtime/boot, gate wasm parts) | both target checks; touched tests pass |
| 3 | H4 (thiserror conversion + per-domain placement + Display cleanup + ParseError rename + UrlValidationError fix) | both target checks; behavior tests unchanged |

## Risks & mitigations

1. **`StorageError::Conflict` Display test** — explicit verification against the test file. Use literal-arg form.
2. **Web shim re-exports** — `crates/websh-web/src/lib.rs` re-exports `crate::core::storage::*`, `crate::core::runtime::*`, etc. Web crate is `cdylib`-only (wasm32), so wasm-gated items resolve fine. **No change needed in this phase**; Phase I removes the shim. (Reviewer confirmed.)
3. **`ParseError` rename** — call sites: `shell::parser::ParseError` is referenced by `shell::Pipeline::parse` callers in `shell/mod.rs:312, 331, 354`; `domain::virtual_path::ParseError` is referenced by `VirtualPath::from_absolute` callers across the workspace. The renames are mechanical via `grep` + replace.
4. **`runtime/loader.rs` cfg-armed calls** — `RuntimeLoad` and `bootstrap_runtime_load` must remain portable. Confirmed: `RuntimeLoad` is a value type; `bootstrap_runtime_load` does the wasm-bound work, callable on host but no-ops/errors gracefully there. Or split into `bootstrap_runtime_load` (wasm-gated) and `bootstrap_runtime_load_pure` for the test paths.
5. **`runtime/wallet.rs` becoming wasm-only** drags `WalletError` with it. That's acceptable: `WalletError` is consumed only by wasm-side code (the wallet primitives + `components/wallet.rs`), so co-locating with the wallet module is the right home.

## Out of scope (Phase I/J)

- Removing the web-side `pub mod core` shim (Phase I).
- B9 visibility audit (Phase I).
- `#[allow(dead_code)]` cleanup (Phase I).
- File splits (Phase J).
- Engine extraction from CLI clap shims (Phase J).
