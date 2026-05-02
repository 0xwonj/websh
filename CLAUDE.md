# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Workspace layout

Three crates under `crates/`:

- `websh-core` — pure-Rust shared library. Domain types, engines, and the storage hexagonal port. Compiles for both `wasm32-unknown-unknown` and the host triple.
- `websh-cli` — native build-time binary. Clap dispatchers + engine modules for content sync, attestation building, mempool subcommands.
- `websh-web` — Leptos UI compiled to wasm32-unknown-unknown. Trunk's target.

## Build Commands

```bash
# Development server
trunk serve

# Production build
trunk build --release

# All Rust tests across the workspace
cargo test --workspace

# Mock commit integration (websh-core + mock feature)
cargo test -p websh-core --features mock --test commit_integration

# Native CLI
cargo run -p websh-cli -- <subcommand> [args...]

# Per-crate target compile checks
cargo check -p websh-core
cargo check -p websh-core --target wasm32-unknown-unknown
cargo check -p websh-web --target wasm32-unknown-unknown

# Browser QA after starting release Trunk on 4173
WEBSH_E2E_BASE_URL=http://127.0.0.1:4173 NODE_PATH=target/qa/node_modules target/qa/node_modules/.bin/playwright test tests/e2e --reporter=line --workers=1
```

## Prerequisites

- Rust with `wasm32-unknown-unknown`: `rustup target add wasm32-unknown-unknown`
- Trunk: `cargo install trunk`
- Stylance CLI: `cargo install stylance-cli`
- Playwright for browser QA

## Architecture

Websh is a client-side browser runtime over one canonical filesystem rooted at `/`.
Runtime assembly flows through `config::BOOTSTRAP_SITE -> core::runtime::loader -> RuntimeLoad`.

Core filesystem concepts:

- `GlobalFs`: canonical tree for `/site`, `/mnt/<name>`, and `/state`.
- `RuntimeMount`: mount ownership and write metadata.
- `ScannedSubtree`: backend-neutral scan result.
- `StorageBackend`: scan/read/commit contract.
- `RouteRequest`, `RouteResolution`, `RouteFrame`, `RenderIntent`: route and render decision surface.

The UI should render engine output. It should not assemble filesystems or resolve backend details directly.

## Module Structure

`websh-core` (cross-target shared library):

- `crates/websh-core/src/domain/`: pure data types (filesystem, manifest, mempool, changes, virtual_path, etc.).
- `crates/websh-core/src/filesystem/`: canonical filesystem engine, routing, content reads, render intents, change-merge.
- `crates/websh-core/src/runtime/`: runtime assembly, state projection, commit coordination, env/wallet adapters.
- `crates/websh-core/src/storage/`: `StorageBackend` trait + GitHub/IDB/persist/mock adapters (cfg-gated to wasm32 where applicable).
- `crates/websh-core/src/shell/`: command parser + executor (shell ran in the browser via the terminal UI).
- `crates/websh-core/src/mempool/`: pure mempool helpers (parse, serialize, form, manifest_entry).
- `crates/websh-core/src/attestation/`: artifact, ledger, subject (verification surface).
- `crates/websh-core/src/crypto/`: ack, eth, pgp primitives.
- `crates/websh-core/src/utils/`: format, time, ring_buffer, asset, dom, fetch, sysinfo, url.
- `crates/websh-core/src/{config,theme,content_routes,admin,error}.rs`: top-level shared constants and helpers.

`websh-cli` (native build-time binary):

- `crates/websh-cli/src/cli/`: clap dispatchers + engine logic for `attest`, `content`, `mempool`, `mount`, `ledger`, `crypto`, `pgp`, `ack`, `deploy`. (Engine extraction from clap shims is tracked as a follow-up.)

`websh-web` (Leptos cdylib):

- `crates/websh-web/src/app.rs`: root component, `AppContext`, terminal/explorer state.
- `crates/websh-web/src/components/`: Leptos UI components.
- `crates/websh-web/src/utils/`: DOM utilities, breakpoints (leptos-use), markdown rendering (comrak/ammonia), wasm_cleanup, theme application, fetch.
- `crates/websh-web/src/main.rs`: trunk's wasm entrypoint.

## State Model

`AppContext` owns the safe runtime state snapshot used to render `/state`.
Browser storage is a persistence adapter, not a feature-layer dependency.

Important rules:

- Do not read `localStorage` or `sessionStorage` from feature code.
- Mutate runtime state through the runtime/state adapter and update `AppContext.runtime_state`.
- Do not expose raw GitHub tokens under `/state`; expose only safe markers.
- Commit code receives auth through a narrow runtime secret accessor and `CommitRequest`, not through `/state`.

## Storage and Commit Rules

- Backend scans return `ScannedSubtree`.
- Runtime loader mounts scans directly into `GlobalFs`.
- Commit preparation normalizes staged canonical changes into a backend-neutral `CommitDelta` and merged mount snapshot.
- GitHub manifest JSON is private serialization inside `core::storage::github`.
- GitHub commit paths must validate and respect the backend content prefix.
- Recursive directory deletes must expand to concrete file deletions.
- Empty directories must survive manifest export/import.

## Command Patterns

When adding commands:

1. Add parser support in `crates/websh-core/src/shell/`.
2. Keep execution pure and return `CommandResult`.
3. Express UI mutations as `SideEffect`.
4. Dispatch async/browser effects from the UI/runtime boundary.
5. Add command parse and execution tests.

## Security Notes

- Treat mounted content as untrusted.
- Render Markdown and HTML only after sanitization, or isolate richer HTML in a sandboxed iframe.
- Access metadata is advisory UI filtering, not cryptographic access control.
- GitHub tokens should use minimum scopes and be kept out of rendered filesystem content.
- Anti-framing must be enforced by deployment headers, not HTML meta tags.
