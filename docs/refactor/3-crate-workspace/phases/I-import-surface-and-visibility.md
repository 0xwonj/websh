# Phase I Design — Web import surface cleanup + visibility audit + dead-code sweep

## Status

Design — picks up Phase B wrap-up deferred items 1, 8, 9.

## Goal

Make `websh-web` consume `websh-core` through direct paths, drop the `pub mod core { ... }` / `pub mod crypto { ... }` / `models` / `config` / `content_routes` / `mempool` re-export shims, narrow visibility on items not actually consumed across crates, and audit `#[allow(dead_code)]` annotations.

## Sub-tasks

### I1 — Drop shim re-exports; rewrite call sites

`crates/websh-web/src/lib.rs` collapses to:

```rust
#![cfg(target_arch = "wasm32")]

pub mod app;
pub mod components;
pub mod utils;
```

All shim re-exports (`models`, `config`, `content_routes`, `mempool`, `crypto`, `core`) are deleted. Call sites in the web crate move to direct `websh_core::*` paths.

**Path mapping**:

| Shim path | Direct path |
|---|---|
| `crate::core::engine::*` | `websh_core::filesystem::*` |
| `crate::core::commands::*` | `websh_core::shell::*` |
| `crate::core::merge::*` | `websh_core::filesystem::merge::*` |
| `crate::core::changes::*` | `websh_core::domain::changes::*` |
| `crate::core::parser::*` | `websh_core::shell::parser::*` |
| `crate::core::wallet::*` | `websh_core::runtime::wallet::*` |
| `crate::core::env::*` | `websh_core::runtime::env::*` |
| `crate::core::runtime::*` | `websh_core::runtime::*` |
| `crate::core::storage::*` | `websh_core::storage::*` |
| `crate::core::admin::*` | `websh_core::admin::*` |
| `crate::core::AutocompleteResult` | `websh_core::shell::AutocompleteResult` |
| `crate::core::Command` | `websh_core::shell::Command` |
| `crate::core::CommandResult` | `websh_core::shell::CommandResult` |
| `crate::core::SideEffect` | `websh_core::shell::SideEffect` |
| `crate::core::autocomplete` | `websh_core::shell::autocomplete` |
| `crate::core::execute_pipeline` | `websh_core::shell::execute_pipeline` |
| `crate::core::get_hint` | `websh_core::shell::get_hint` |
| `crate::core::parse_input` | `websh_core::shell::parse_input` |
| `crate::core::DirEntry` | `websh_core::domain::DirEntry` |
| `crate::models::*` | `websh_core::domain::*` |
| `crate::crypto::attestation::*` | `websh_core::attestation::artifact::*` |
| `crate::crypto::ledger::*` | `websh_core::attestation::ledger::*` |
| `crate::crypto::subject::*` | `websh_core::attestation::subject::*` |
| `crate::crypto::ack::*` | `websh_core::crypto::ack::*` |
| `crate::crypto::eth::*` | `websh_core::crypto::eth::*` |
| `crate::crypto::pgp::*` | `websh_core::crypto::pgp::*` |
| `crate::config::*` | `websh_core::config::*` |
| `crate::content_routes::*` | `websh_core::content_routes::*` |
| `crate::mempool::*` | `websh_core::mempool::*` |

**Sed order matters**: do longest-prefix first so `crate::core::engine::` substitutes before `crate::core::` would catch it. Bare items (`SideEffect`, `parse_input`, etc.) are handled with full-string replacements, not prefix substitution.

**Grouped imports** (`use crate::core::{env, runtime, wallet};`) need manual rewrite since the brace-list members aren't prefixed. Run `cargo check -p websh-web --target wasm32-unknown-unknown` after the sweep to surface the unresolved imports; rewrite each to its direct path.

The web crate's own internal modules — `crate::app::*`, `crate::components::*`, `crate::utils::*` — are unaffected.

### I2 — Visibility audit

`websh-web` has no consumers (no other crate depends on it). Every `pub fn`, `pub struct`, `pub mod` inside `crates/websh-web/src/` that isn't reached from `lib.rs`'s public surface or `main.rs` should be `pub(crate)`. The shim removal in I1 narrows the public surface to nearly nothing, making this audit cheap.

`websh-core` has two consumers: `websh-web` and `websh-cli`. Walk every `pub` item in `crates/websh-core/src/` and check whether it's consumed across the crate boundary:

```bash
grep -rn "websh_core::<item>" crates/websh-web/src crates/websh-cli/src
```

If zero hits, narrow to `pub(crate)`. Common targets per the Phase B wrap-up review:
- `storage::persist::DraftPersister` — used by web only? Keep `pub`.
- Several `pub use` re-exports in `attestation/mod.rs`.
- `runtime::commit::commit_backend` etc. — depends on call site.

### I3 — `#[allow(dead_code)]` sweep

Walk every `#[allow(dead_code)]` annotation in the workspace. For each:

1. **Genuinely dead post-migration** — delete the item.
2. **Wasm-only / test-only** — keep the allow with a one-line `// reason:` comment naming the use case.
3. **Visibility mismatch** — narrow visibility instead of allowing dead-code.

Specific files flagged in the Phase B wrap-up: `storage/{backend, error, github/client, github/graphql, idb, persist}.rs`, `domain/{filesystem, node_metadata}.rs`, plus the new module-scope allows added in Phase H (`storage/github/manifest.rs`, `storage/github/path.rs`, `runtime/boot.rs`).

## Sequencing & commits

| Commit | Scope | Verification |
|---|---|---|
| 1 | I1 — shim removal + call-site rewrites + lib.rs collapse | `cargo check --workspace --all-targets`, `cargo check -p websh-web --target wasm32-unknown-unknown`, `cargo test --workspace`, `trunk build` all green |
| 2 | I2 — visibility audit | `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets` clean; spot-check a few `pub(crate)` narrowings via `grep` |
| 3 | I3 — `#[allow(dead_code)]` sweep | `cargo check --workspace --all-targets`, no new dead-code warnings without a `// reason:` comment |

## Risks & mitigations

1. **Sed order**. Longest-prefix replacement first prevents `crate::core::engine::` from being caught by an earlier `crate::core::` rule. Run `cargo check` after each pass to catch ambiguity.
2. **Grouped imports** like `use crate::core::{env, runtime, wallet};` — sed won't catch these. After the bulk sed, `cargo check` surfaces unresolved imports; rewrite manually.
3. **Test files**. `crates/websh-web/tests/*` (if any) likely use the same shim paths. Include them in the sweep.
4. **`crate::crypto::attestation::*` mapping**. Note the rename to `attestation::artifact` (not `attestation::attestation`). Caught by `cargo check` if missed.
5. **Re-exporting from `websh-web/src/utils/mod.rs`**. Web's local utils may currently re-export some `websh_core::utils::*` items via the shim path. Check before assuming the re-exports stay valid.
6. **Trunk's stylance hooks** depend on file paths inside `crates/websh-web/src/`. Module renames are not in scope for Phase I; only path-import migrations.

## Out of scope (Phase J)

- File splits: `shell/execute.rs` (2237 lines), `cli/mempool.rs` (1297), `cli/attest.rs` (1200), `filesystem/global_fs.rs` (1080), `shell/mod.rs` (935), `crypto/ack.rs` (813), `attestation/ledger.rs` (802).
- CLI engine extraction from clap shims.
