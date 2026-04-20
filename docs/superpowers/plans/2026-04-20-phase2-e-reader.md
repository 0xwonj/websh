# Phase 2 Track E: Reader Race Condition Fix — Implementation Summary

**Issue:** H9. `src/components/reader/mod.rs` used `Effect::new` + `spawn_local` for content fetching. When `content_url` or `file_type` changed faster than the previous fetch completed, a stale future's `set_content.set(...)` could overwrite the current result.

**Fix:** Migrated fetch logic to `LocalResource::new(...)`, which tracks dependencies and drops stale futures when inputs change. Mirrors the pattern already used in `src/components/explorer/preview/hook.rs::use_preview`.

## Changes

- New internal enum `ReaderContent { Html | Text | Redirected | Error }` captures the four result states.
- `LocalResource<ReaderContent>` replaces `Effect + spawn_local + 3 RwSignals`.
- Derived signals (`content`, `loading`, `error`) are computed from the resource via `Signal::derive`.
- Removed `wasm_bindgen_futures::spawn_local` import (no longer needed).
- PDF/Image types return `ReaderContent::Text(String::new())` as an inert placeholder — their render paths don't consume `content`; they use `image_url` / PDF iframe directly.

## Verification

- `cargo build --release --target wasm32-unknown-unknown`: clean.
- `cargo test --bin websh`: 189 pass / 4 pre-existing fail (no regression).
- No test coverage exists for this UI code (wasm-only rendering paths); visual smoke test recommended at the Phase 2 close.

## Done Criteria (met)

- Race condition closed — cancellation is automatic via `LocalResource`.
- `Effect::new` + `spawn_local` fetch pattern removed from `reader/mod.rs`.
- Other reader functionality (keyboard handlers, menu stubs, breadcrumb navigation) unchanged.
