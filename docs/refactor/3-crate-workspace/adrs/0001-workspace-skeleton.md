# 0001 — Adopt hybrid root-package-plus-workspace layout for the migration

- **Status**: Accepted
- **Date**: 2026-05-03
- **Phase**: A

## Context

The migration moves a single-crate project to a 3-crate Cargo workspace. The intermediate state must keep the existing build (`trunk serve`, `cargo test`, the existing `[[bin]]` for `websh-cli`) working while empty member crates are introduced. Doing the workspace declaration and member creation as a single big-bang commit risks a long red period; the migration's workflow asks for granular commits where each one compiles.

## Decision

Adopt a hybrid layout: the existing root `[package]` table stays in place and continues to build the legacy crate; a new `[workspace]` declaration alongside it lists three empty member crates under `crates/`. Shared metadata moves to `[workspace.package]` and `[workspace.dependencies]`, both of which the legacy root package starts to consume immediately. The legacy root package shrinks as Phases B-D move code into the members; it is removed entirely at the end of Phase D.

## Consequences

- **Positive** — every commit on the branch compiles. Phase A landed without touching application code.
- **Positive** — `[workspace.dependencies]` is in place from day one, so member crates can adopt it as they are populated.
- **Negative** — the working tree carries a duplicate-feeling structure (`src/` plus `crates/<name>/src/`) until Phase D. Readers of in-flight commits need to understand both exist together.
- **Follow-on** — the root `[package]` removal in Phase D is its own ADR, since it changes how `trunk` discovers the project.

## Alternatives considered

- **Virtual workspace from the start (no root package)**. Cleanest end state, but requires moving every `src/` file before the workspace check passes, which contradicts the granular-commit workflow.
- **Workspace-only root with a temporary fourth member shim crate** holding the legacy code. Adds churn (rename existing files into a temporary crate, then move them out again). Hybrid is simpler.
- **Defer the workspace declaration to Phase D** and do all phases B and C inside the legacy crate first. Gives up the cross-crate compile-time enforcement that's the entire point of the migration; rejected.

## References

- Architecture: §2 (top-level layout), §6 (workspace configuration sketches).
- Workflow: § Phases A-F.
