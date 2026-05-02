# 3-Crate Workspace Migration — Master Index

This directory is the single source of truth for the workspace migration. Anyone (human or AI) picking up this branch reads this file first.

## Branch

`refactor/3-crate-workspace` — created from `main` at commit `fe49a6d` (the post-review checkpoint capturing Phases 1-5 of the architecture review).

## What we are doing

Migrating `websh` from a single-crate layout to a 3-crate Cargo workspace:

```
websh-core   pure-Rust shared library  (wasm + native)
websh-cli    native build-time binary  (clap dispatchers + engine modules)
websh-web    Leptos cdylib for trunk   (UI only)
```

The migration is a **breaking change**. No legacy paths, no backwards-compatibility shims, no `v2`-style names — we replace, we don't accumulate.

## Documents (read in this order)

1. **[architecture.md](./architecture.md)** — what we are building. Full crate layout, module hierarchy, dependency rules, design rationale. The "what" and "why."
2. **[workflow.md](./workflow.md)** — how we execute. Phase breakdown, per-task loop, decision rules, stop/escalate conditions, agent usage. The "how."
3. **[conventions.md](./conventions.md)** — commit message format, comment policy, naming, error handling, file size limits, versioning. Mechanical rules that apply to every change.
4. **[principles.md](./principles.md)** — design patterns and idioms we apply (and avoid). Leptos best practices, Rust idioms, hexagonal where it earns its keep.
5. **[adrs/](./adrs/)** — Architecture Decision Records. One ADR per phase (and per material deviation). [Template here.](./adrs/0000-template.md)
6. **[deviation-log.md](./deviation-log.md)** — append-only running log of every place execution diverged from the architecture, with rationale and ADR pointer.

## Status

| Phase | Description | Status |
|---|---|---|
| A | Workspace skeleton | pending |
| B | `websh-core` populated | pending |
| C | `websh-cli` populated + engine extraction | pending |
| D | `websh-web` populated + UI consolidation | pending |
| E | Trunk + asset paths working | pending |
| F | Docs + repository hygiene | pending |
| G | Browser PGP verification (held — separate branch later) | held |

Status is updated in this table at every phase boundary commit.

## How an AI agent picks this up

If you are an AI agent resuming this migration:

1. Read this README to orient.
2. Read [workflow.md](./workflow.md) end-to-end — it defines the loop you follow.
3. Read [conventions.md](./conventions.md) and [principles.md](./principles.md) — they define the rules every change must follow.
4. Skim [architecture.md](./architecture.md) — read the section relevant to the current phase in full.
5. Check `git log --oneline refactor/3-crate-workspace` to see what has landed.
6. Check the Status table above to see which phase is in flight.
7. Read the most recent ADRs in `adrs/` to see what decisions have been made and recorded.
8. Read [deviation-log.md](./deviation-log.md) for any in-flight deviations from the architecture.
9. Continue the workflow from where it left off.

You do not ask the user before proceeding unless one of the explicit stop conditions in [workflow.md](./workflow.md) fires.

## How a human picks this up

The branch is migrating-in-place, so you can `git log` the branch to see real progress, `cargo test --workspace` for verification status, and read the deviation log to understand any choices that diverged from the original architecture.

To pause the migration mid-phase: just stop. The work-in-progress is on the branch; resume by re-reading the docs and continuing.

To redirect: edit the architecture doc and append to the deviation log; the workflow's decision-rules section already permits this.

## Top-level project conventions

This document does **not** restate project-wide guidance from `/CLAUDE.md` (the root project guide). The migration's docs add migration-specific rules on top. When the two conflict, migration docs win for migration commits; project guidance wins everywhere else.

## Tooling expectations

```
cargo build --workspace
cargo test --workspace
cargo test --features mock --test commit_integration   # one specific test
cargo clippy --workspace --all-targets
cargo check -p websh-core --target wasm32-unknown-unknown
cargo check -p websh-web --target wasm32-unknown-unknown
cargo fmt --check
trunk build
```

Trunk's `pre_build` hook will call `cargo run -p websh-cli -- content manifest` once the migration lands.

## When the migration is done

- All 6 phases (A-F) committed on the branch.
- The Status table reads "complete" for A-F (G remains "held").
- All verification commands above pass.
- `architecture.md` § Deviation log + `deviation-log.md` reflect every place execution diverged.
- One ADR per phase committed under `adrs/`.
- A summary commit on the branch updates `/CLAUDE.md` with the new build commands and file paths.

The branch is then ready to merge to `main` as a single squash or as the granular history the workflow produced (project preference).
