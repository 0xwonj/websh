# 0002 — Cross-target cfg discipline + thiserror sweep

- **Status**: Accepted
- **Date**: 2026-05-02
- **Phase**: H

## Context

Phase B's wrap-up review surfaced three coupled boundary smells. `websh-core`'s wasm-only deps (`wasm-bindgen`, `web-sys`, `gloo-net`, `gloo-timers`, `idb`, `serde-wasm-bindgen`, `js-sys`, `wasm-bindgen-futures`) sat in plain `[dependencies]`; several modules referenced wasm types unconditionally; `storage::boot` exported `GlobalFs`-shaped engine helpers from the adapter layer; and every error type still carried hand-rolled `Display + Error` impls despite `thiserror = "2"` being in workspace deps and mandated by `conventions.md`. Native `cargo check -p websh-core` worked only because wasm crates happen to no-op on the host triple — the two-target invariant was nominal, not load-bearing.

Phase H makes the boundary real. `websh-core` must build on `wasm32-unknown-unknown` and the host triple with the wasm crates absent on host. Adapter and runtime modules must split cleanly along the wasm/native fault line. Error types convert to `thiserror` and relocate to per-domain homes (`error.rs` is deleted).

## Decision

1. **Wasm-only deps move under `[target.'cfg(target_arch = "wasm32")'.dependencies]`** in `crates/websh-core/Cargo.toml`. Pure-Rust deps stay unconditional.

2. **Module-level cfg gating** at parent `mod` declarations:
   - `utils::fetch`, `utils::sysinfo`: wasm-only
   - `storage::idb`, `storage::persist`, `storage::boot`: wasm-only
   - `storage::github::client` (only the HTTP client, not `manifest`/`graphql`/`path`): wasm-only
   - `runtime::wallet`, `runtime::loader`: wasm-only
   `utils::dom` and `utils::asset` keep internal cfg-arms (host stubs return `None` / data URLs) so portable callers don't need to know.

3. **`runtime::boot` is created** to host the pure scaffolding helpers (`bootstrap_runtime_mount`, `bootstrap_global_fs`, `seed_bootstrap_routes`, `assemble_global_fs`) that previously lived in `storage::boot`. `storage::boot` becomes wasm-only and now contains only the GitHub backend constructors and IDB hydrators. This restores the layering: adapter modules don't ship engine constructors. Visibility is `pub(crate)` for all helpers — none are cross-crate consumed.

4. **Web crate becomes wasm-only at the lib level** via `#![cfg(target_arch = "wasm32")]` on `crates/websh-web/src/lib.rs` and `main.rs`. On host the lib resolves to an empty crate; the binary is a no-op `fn main()`. This is simpler than dependency-cfg-gating Leptos / wasm-bindgen / web-sys across the entire web crate's `[dependencies]`, and it accurately reflects that no host build of the web crate has runtime meaning.

5. **Errors convert to `thiserror::Error`** with per-domain placement:
   - `WalletError` moves from `error.rs` to `runtime/wallet.rs` (becomes wasm-only — its only consumers are wasm-side)
   - `EnvironmentError` moves to `runtime/state.rs` (the canonical home for env operations)
   - `FetchError` moves to `filesystem/content.rs` (the portable consumer; `utils::fetch` imports it back)
   - `AckError`, `EthVerifyError`, `UrlValidationError`: `thiserror` derive in place. `UrlValidationError` also gets the missing `std::error::Error` impl.
   - `shell::parser::ParseError` → renamed to `ShellParseError`; `domain::virtual_path::ParseError` → renamed to `VirtualPathParseError` (per breaking-changes-only / no-aliases policy).
   - `StorageError` keeps a hand-written `Display` because two variants format dynamically (SHA truncation in `Conflict`; `retry_after` switch in `RateLimited`); thiserror's literal-arg form would obscure intent. The trait still derives via thiserror in spirit (manual but minimal). Reason captured as a one-line comment in the file.
   - `crates/websh-core/src/error.rs` is **deleted**.

## Consequences

- **Positive** — `cargo check -p websh-core` on host no longer pulls in wasm crates; the two-target invariant becomes load-bearing. Adapter/engine layering is restored. Error types follow the conventions doc.
- **Positive** — `error.rs` removal eliminates a cross-domain bucket; each error's home is now obvious from the import path.
- **Positive** — web crate's host-empty-lib pattern means `cargo check --workspace` on host succeeds without forcing dependency-level cfg gates on Leptos & friends.
- **Negative (intentional)** — `cargo test -p websh-web` on host now runs zero tests, forever. Web tests must live in `crates/websh-web/tests/` with their own wasm32 cfg gates, or run via `wasm-bindgen-test`. Acceptable trade-off given the web crate is genuinely wasm-only at runtime (Trunk only ever builds wasm32).
- **Negative** — host-side `cargo check` reports `dead_code` for several pure helpers (`storage::github::manifest`, `storage::github::path`, `storage::boot`, `runtime::boot`) that are exercised on wasm32 or in tests. Allowed at module scope with a comment explaining why; acknowledging the gap rather than papering over it.
- **Follow-on** — Phase I tightens cross-crate import surface and audits remaining `pub` visibility; Phase J splits oversized files (`shell/execute.rs`, the CLI engines).

## Alternatives considered

- **Cfg-gate web crate's deps individually.** Each `[dependencies]` entry would carry `target = 'cfg(target_arch = "wasm32")'`; the lib would compile on host with all symbols missing. Tried briefly — produces noisy errors deep inside Leptos macros when the host build can't find `wasm-bindgen`. The cfg-the-lib approach is one line and unambiguous; the cfg-the-deps approach is dozens of lines and fragile.
- **Move `FetchError` to `utils::fetch`** as the design originally specified. Broke `filesystem::content` on host because `utils::fetch` is wasm-only. Resolved by placing `FetchError` in `filesystem::content` (the portable consumer); `utils::fetch` imports it back.
- **Convert `StorageError` to `thiserror::Error` with literal-arg formatters.** Two variants (Conflict's SHA truncation, RateLimited's retry_after switch) would have required helper functions or a fallback to manual `Display`; the cleanest end-state was to keep manual `Display` for this type alone with a comment naming the reason. The remaining seven types are pure thiserror.
- **Keep `error.rs` as a single file** with all three errors. Rejected: the design's per-domain placement is a real architectural improvement, especially with `WalletError` becoming wasm-only alongside its sole module.

## References

- Architecture: §4 (layered design within `websh-core`), §5.4 (error handling).
- Phase H design: `phases/H-cross-target-cfg.md`.
- Phase B wrap-up: deferred items 2 ("thiserror adoption"), 3 ("wasm-only deps"), 4 ("storage adapters cfg-gating"), 5 ("storage::boot reaches up into engine").
