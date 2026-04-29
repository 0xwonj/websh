# Phase 4 — `BuiltinRoute` Partition — Implementation Plan

**Design:** [`../specs/2026-04-29-render-pipeline-phase4-design.md`](../specs/2026-04-29-render-pipeline-phase4-design.md)
**Status:** Approved

## Steps

### 4.1 — Inspect `is_ledger_filter_route_segment`

Read `src/components/ledger_routes.rs` to confirm whether the empty segment / root path is considered a filter — affects `detect`'s arm ordering.

### 4.2 — Add `BuiltinRoute` enum and `detect`

`src/components/router.rs`:
- Add `pub enum BuiltinRoute { Home, LedgerFilter, NewCompose }` and `impl BuiltinRoute { fn detect(...) }` per design §3.
- Place near the top of the file, after the imports.

### 4.3 — Reshape dispatch

Replace the three `if _raw_request.with(is_*) { ... }` early-return guards in `RouterView` with a single `match BuiltinRoute::detect(&request) { ... }` per design §4. The fall-through `None` arm contains the existing engine-side `match route.get() { ... }`.

### 4.4 — Rename helpers for symmetry

`builtin_home_frame` → `home_frame`. Other names already match.

### 4.5 — Verify the helpers are private

`grep -rn 'home_frame\|ledger_filter_frame\|new_compose_frame' src/` should only show `router.rs`.

### 4.6 — Add `builtin_route_tests` module

5 tests per design §6.

### 4.7 — Verify

```sh
cargo check --target wasm32-unknown-unknown --lib
cargo test --lib
trunk build
```

### 4.8 — Code review

`superpowers:code-reviewer`.

### 4.9 — Master update

Phase 4 → Complete; Phase 5 → Next.
