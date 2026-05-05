# CLAUDE.md

This file gives coding agents the current repository map and operating rules. The authoritative architecture docs are under `docs/architecture/`.

## Workspace Layout

Four crates live under `crates/`:

- `websh-core`: host + wasm shared library. Owns domain contracts, public facades, filesystem, shell, runtime coordination, mempool helpers, attestation primitives, storage ports, and support helpers.
- `websh-site`: host + wasm site-policy crate. Owns deployed identity, public key constants, acknowledgement data, and site-specific copy/policy.
- `websh-cli`: host binary. Owns Clap adapters, command workflows, process/filesystem/GitHub/GPG/Trunk adapters, deploy, mempool, mount, content, and attestation commands.
- `websh-web`: wasm Leptos app. Owns `AppContext`, runtime services, browser storage adapters, wallet/DOM/fetch/object URL platform code, feature views, and CSS modules.

`websh-cli` and `websh-web` must not depend on each other. Both depend on `websh-core` and may use `websh-site`.

## Active Architecture Boundaries

- `websh-core::engine` is private. External crates import from `websh_core::{domain, filesystem, runtime, shell, mempool, attestation, crypto, ports, support, errors}`.
- `VirtualPath` is the only engine path type for canonical filesystem paths.
- Runtime overlay paths are owned by `runtime_state_root()` and `is_runtime_overlay_path()`.
- `StorageBackend` is a local, non-`Send` browser-friendly port using `Rc<dyn StorageBackend>`.
- CLI command modules should parse arguments and delegate to `workflows`.
- CLI `infra` owns process execution and typed wrappers around `git`, `gh`, `gpg`, and `trunk`.
- Web feature code should use `AppContext` and `RuntimeServices`; browser storage belongs in `runtime`, browser APIs in `platform`.
- Do not expose raw GitHub tokens through rendered runtime state.

## Current Module Map

`websh-core`:

- `domain/`: stable data contracts, paths, manifests, metadata, mounts, wallet, changes.
- `engine/`: private implementation modules.
- `filesystem.rs`, `runtime.rs`, `shell.rs`, `mempool.rs`, `attestation.rs`, `crypto.rs`, `ports/`, `support/`, `errors.rs`: public facades.

`websh-cli`:

- `cli.rs`: top-level Clap dispatch.
- `commands/`: thin adapters from args to workflow options.
- `workflows/`: use-case logic.
- `infra/`: process, GitHub, Git, JSON, and filesystem adapters.

`websh-web`:

- `app/`: root component, context, services, terminal state.
- `runtime/`: loader, mounts, browser persistence, wallet, storage state, draft persistence.
- `platform/`: DOM, fetch, object URL, redirect, time, breakpoint helpers.
- `features/`: chrome, home, ledger, mempool, reader, router, terminal.
- `shared/`: reusable UI components.
- `render/`: markdown and theme rendering.

## Commands

```bash
trunk serve
trunk build --release
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo check -p websh-core --target wasm32-unknown-unknown
cargo check -p websh-web --target wasm32-unknown-unknown
cargo test -p websh-core --features mock --test commit_integration
cargo run -p websh-cli -- <subcommand> [args...]
npm run lint:css
npm run docs:drift
npm run perf:budgets -- dist
npm run e2e
just verify
```

Use focused checks while developing, then run the relevant wider gate before finishing. Browser runtime changes should include `cargo check -p websh-web --target wasm32-unknown-unknown`; native `cargo check` can miss wasm-only imports.

## Trunk And Generated Artifacts

`Trunk.toml` pre-build hooks run:

1. Stylance to regenerate `assets/bundle.css`.
2. `cargo run --quiet -p websh-cli -- content manifest`.
3. `cargo run --quiet -p websh-cli -- attest build`.

Do not edit generated sidecars, `content/manifest.json`, `content/ledger.json`, `assets/bundle.css`, or `assets/crypto/attestations.json` as if they were hand-authored unless the task explicitly targets generated outputs. Prefer running the owning command.

`attest build` skips non-release Trunk profiles unless forced. `WEBSH_NO_SIGN=1` disables signing and leaves subjects pending.

## Security Notes

- Treat mounted content as untrusted.
- Markdown and HTML must be sanitized before rendering.
- Access metadata is an advisory UI filter, not confidentiality.
- Keep GitHub PATs out of command history, rendered filesystem state, logs, and docs.
- Deployment anti-framing and CSP are header responsibilities.

## Documentation Rule

Current architecture lives in `docs/architecture/`. Historical refactor documents under `docs/refactor/3-crate-workspace/` are useful context but do not override the current docs.
