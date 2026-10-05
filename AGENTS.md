# AGENTS.md

This file gives coding agents the current repository map and operating rules. The authoritative architecture docs are under `docs/architecture/`.

## Workspace Layout

Four crates live under `crates/`:

- `websh-core`: host + wasm shared library. Owns domain contracts, public facades, filesystem, shell, runtime coordination, mempool helpers, attestation primitives, storage ports, and support helpers.
- `websh-site`: host + wasm site-policy crate. Owns deployed identity, public key constants, acknowledgement data, and site-specific copy/policy.
- `websh-cli`: host binary. Owns Clap adapters, command workflows, filesystem/GPG/Pinata adapters, deterministic content generation, verification, local draft import, acknowledgement, signing, and publishing workflows.
- `websh-web`: wasm Leptos app. Owns `AppContext`, runtime owners, browser storage adapters, wallet/DOM/fetch/object URL platform code, feature views, and CSS modules.

`websh-cli` and `websh-web` must not depend on each other. Both depend on `websh-core` and may use `websh-site`.

## Active Architecture Boundaries

- `websh-core::engine` is private. External crates import from `websh_core::{domain, filesystem, runtime, shell, mempool, attestation, crypto, ports, support}`.
- `VirtualPath` is the only engine path type for canonical filesystem paths.
- Runtime overlay paths are owned by `runtime_state_root()` and `is_runtime_overlay_path()`.
- `StorageBackend` is a local, non-`Send` browser-friendly port using `Rc<dyn StorageBackend>`.
- CLI command modules should parse arguments and delegate to `workflows`.
- CLI `infra` owns GPG/Pinata process execution and atomic artifact writes. Git editing/publishing and Trunk builds remain explicit developer operations.
- CLI `sync` only generates; `check` only verifies; signing is explicit through `attest`.
- ContentSnapshot validates one authored tree before generation or import writes. Markdown frontmatter and binary/directory declarations own authored metadata; derived metadata exists only in generated artifacts.
- Web feature code should use the `Content`, `Wallet`, and `Preferences` owners composed by `AppContext`, and `RuntimeServices`; browser storage belongs in `runtime`, browser APIs in `platform`.
- Browser content is read-only; wallet connection grants no write privilege.
- Cache only accepted external listings in `websh-cache`. Cache data is disposable.
- Accept only current contracts; migrate owned data rather than adding compatibility readers or aliases.
- Validate terminal commands before echo/history; unsupported input is never retained.
- Keep necessary cryptographic domain versions and toolchain pins; avoid decorative internal versions.

## Current Module Map

`websh-core`:

- `domain/`: stable data contracts, paths, manifests, metadata, mounts, wallet.
- `engine/`: private implementation modules.
- `lib.rs`: public `filesystem`, `runtime`, `shell`, `mempool`, `attestation`, and `crypto` facades; `ports/` and `support/`: storage/read and support contracts. Errors belong to their owning facades.

`websh-cli`:

- `cli.rs`: top-level Clap dispatch.
- `commands/`: thin adapters from args to workflow options.
- `workflows/`: use-case logic.
- `infra/`: GPG, Pinata, JSON, filesystem, and deployment environment adapters.

`websh-web`:

- `app/`: root component, context, services, terminal state.
- `runtime/`: loader, mount refresh/state, bounded mount/text caches, browser preference persistence, and wallet.
- `platform/`: DOM, fetch, object URL, redirect, time, breakpoint helpers.
- `features/`: chrome, home, ledger, mempool, reader, router, terminal.
- `shared/`: reusable UI components.
- `render/`: markdown and theme rendering.

## Commands

```bash
just --list
just serve
just build
just publish
just verify
just test-wasm [filter]
just test-tools
just build-check
just e2e [filter]
just size [dist]
just clean --dry-run
cargo test --locked -p websh-cli
cargo run --locked -p websh-cli -- <subcommand> [args...]
```

`justfile` owns the executable gate. See `docs/architecture/tooling.md` for bootstrap,
output ownership, and scoped cleanup. Do not load deployment `.env` into general
checks or development commands.

Use focused checks while developing, then run the relevant wider gate before finishing. Browser runtime changes should include `cargo check -p websh-web --target wasm32-unknown-unknown`; native `cargo check` can miss wasm-only imports.

Keep tests focused on current behavior at the owning layer. Remove obsolete and duplicate
cases; do not retain historical-format galleries or tests of trivial wrappers/derived traits.
Share small scoped fixtures, not a generic testing framework. See the test ownership and
focused commands in `docs/architecture/verification.md`.

## Trunk And Generated Artifacts

`Trunk.toml` runs independent Stylance and `websh-cli sync` hooks. Same-stage
hooks may run concurrently; `sync` is the only content artifact writer. All profiles
use the same deterministic manifest, ledger, acknowledgement, and subject generation.
Trunk never signs. `just publish` explicitly runs sync, signing, release build, and deploy.

Do not hand-edit `content/manifest.json`, `content/.websh/ledger.json`,
`assets/bundle.css`, `assets/crypto/ack.commitment.json`, or
`assets/crypto/attestations.json`. Markdown frontmatter, `file.ext.meta.json` for
binary metadata, and `_index.dir.json` directory/bundle declarations are authored inputs.
Only current authored fields are accepted; derived fields are computed from bytes.
Unchanged subjects retain valid signatures. Changed/new subjects remain pending until
explicitly signed. `check --require-signatures` requires the deployed site's PGP identity;
Ethereum attestations are supplemental. `deploy` publishes only an already-built `dist/`
that matches current project artifacts and passes signature checks.

## Security Notes

- Treat mounted content as untrusted.
- Markdown and HTML must be sanitized before rendering.
- Access metadata is an advisory UI filter, not confidentiality.
- Keep GitHub PATs out of command history, rendered filesystem state, logs, and docs.
- Deployment anti-framing and CSP are header responsibilities.

## Documentation Rule

Current architecture lives in `docs/architecture/`. Current contracts have one native representation. Completed historical proposals live in Git history, not active architecture guidance.
