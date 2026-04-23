# CLAUDE.md

This file provides guidance to Claude Code when working with this repository.

## Build Commands

```bash
# Development server
trunk serve

# Production build
trunk build --release

# Rust tests
cargo test

# Mock commit integration
cargo test --features mock --test commit_integration

# Browser QA after starting release Trunk on 4173
NODE_PATH=target/qa/node_modules target/qa/node_modules/.bin/playwright test tests/e2e --reporter=line --workers=1
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

- `src/app.rs`: root component, `AppContext`, terminal/explorer state.
- `src/core/engine/`: canonical filesystem, routing, content reads, render intents.
- `src/core/runtime/`: runtime assembly, runtime state projection, commit coordination.
- `src/core/storage/`: backend-neutral storage trait plus GitHub/IDB implementations.
- `src/core/commands/`: parsing and pure command execution.
- `src/components/`: Leptos UI components.
- `src/models/`: shared data structures.
- `src/utils/`: DOM, fetch, markdown/HTML sanitization, URL validation, formatting.
- `src/config.rs`: bootstrap source and app constants.

## State Model

`AppContext` owns the runtime state snapshot used to render `/state`.
Browser storage is a persistence adapter, not a feature-layer dependency.

Important rules:

- Do not read `localStorage` or `sessionStorage` from feature code.
- Mutate runtime state through the runtime/state adapter and update `AppContext.runtime_state`.
- Do not expose raw GitHub tokens under `/state`; expose only safe markers.
- Commit code receives auth through `CommitRequest`, not hidden browser reads.

## Storage and Commit Rules

- Backend scans return `ScannedSubtree`.
- Runtime loader mounts scans directly into `GlobalFs`.
- Commit preparation merges staged canonical changes into a mount snapshot.
- GitHub manifest JSON is private serialization inside `core::storage::github`.
- GitHub commit paths must respect the backend content prefix.
- Recursive directory deletes must expand to concrete file deletions.
- Empty directories must survive manifest export/import.

## Command Patterns

When adding commands:

1. Add parser support in `src/core/commands/`.
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
