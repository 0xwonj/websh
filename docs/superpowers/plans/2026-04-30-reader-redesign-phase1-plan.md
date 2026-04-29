# Phase 1 — Plan: `FileMeta` move

**Date:** 2026-04-30
**Design:** `docs/superpowers/specs/2026-04-30-reader-redesign-phase1-design.md`

## Steps

1. Create `src/components/shared/file_meta.rs` with the `FileMeta` struct, impl block, and `file_meta_for_path` helper. Imports: `crate::app::AppContext`, `crate::models::{FsEntry, VirtualPath}`.
2. Edit `src/components/shared/mod.rs`: add `pub mod file_meta;` and `pub use file_meta::{FileMeta, file_meta_for_path};` next to the other re-exports (alphabetical order: between `file_meta_strip` and `meta_table`).
3. Edit `src/components/explorer/preview/hook.rs`:
   - Remove the `FileMeta` struct + impl block (lines 13–44 in the current file).
   - Add `use crate::components::shared::{FileMeta, file_meta_for_path};` (or merge into the existing `use crate::utils::…` cluster).
   - Replace the `file_meta` `Signal::derive` body with a call to `file_meta_for_path(ctx, &s.path)`.
4. Edit `src/components/explorer/preview/mod.rs`: keep the `pub use hook::{DirMeta, FileMeta, …}` line so `FileMeta` remains accessible via the historical path. (`use crate::components::explorer::preview::FileMeta` will keep compiling.)
5. Run `cargo fmt` to normalize.
6. Run `cargo test --lib`.
7. Run `cargo check --target wasm32-unknown-unknown --lib`.
8. Run `trunk build` (release-style check; visual QA not required for this phase).
9. Spawn `superpowers:code-reviewer` agent with the diff + design + master.
10. Address CRITICAL/HIGH findings (if any). LOW/MEDIUM may be deferred with a Decision Log entry.
11. Update master §6 Phase 1 status → Complete; append §10 Decision Log row.
12. Commit:
    ```
    refactor(reader): move FileMeta to shared so reader can consume it (phase 1)
    ```
    Include: the four `.rs` changes + the master + this design + this plan.

## Risks

- `FileMeta` and `FileMetaStrip` share a prefix; future contributors may confuse them. Mitigation: the doc comment on `FileMeta` should disambiguate. (One-line note.)
- `use_preview` borrows `ctx` by `Copy` capture; `file_meta_for_path` takes `AppContext` by value — `AppContext` is `Copy` (Signal-based fields), so this is fine. Verify by build.

## Acceptance signal

Steps 6–8 all green; reviewer pass clears without CRITICAL/HIGH; master updated.
