# Phase 1 — Bedrock: `FileMeta` move

**Date:** 2026-04-30
**Master:** `docs/superpowers/specs/2026-04-30-reader-redesign-master.md`
**Status:** Approved (autonomous run)

## 1. Scope

Move the `FileMeta` struct (and its small impl block) from `src/components/explorer/preview/hook.rs` into a new `src/components/shared/file_meta.rs`. Add a pure helper `file_meta_for_path(ctx, path) -> Option<FileMeta>` extracted from the `Signal::derive` body inside `use_preview()`. **No behavior change.**

This is the only enabling step Reader needs from the explorer module. After Phase 1, Reader will reach `FileMeta` via `crate::components::shared::FileMeta` exactly the way it would reach `MetaTable` or `AttestationSigFooter`.

## 2. Out of scope

- No reader-side wiring. Reader continues to render exactly as it does today.
- No new fields on `FileMeta`. No frontmatter parsing changes.
- No CSS. No new components.
- `DirMeta`, `PreviewContent`, `PreviewData`, `use_preview` — unchanged.

## 3. Files

| File | Change |
|---|---|
| `src/components/shared/file_meta.rs` | **New.** Holds `FileMeta` struct + impl + `file_meta_for_path` helper. |
| `src/components/shared/mod.rs` | `pub mod file_meta;` + `pub use file_meta::{FileMeta, file_meta_for_path};` |
| `src/components/explorer/preview/hook.rs` | Drop `FileMeta` definition; import `FileMeta` from `crate::components::shared`. Replace inline `Signal::derive` body with a call to `file_meta_for_path`. |
| `src/components/explorer/preview/mod.rs` | Re-export now points to the shared location, but the `pub use hook::{DirMeta, FileMeta, …}` line stays so existing in-module consumers keep working without churn. |

`FileMetaStrip` (in `shared/file_meta_strip.rs`) is unrelated — it's a UI component with a different shape. The naming overlap is unfortunate but pre-existing; we won't rename it in this phase.

## 4. Public API

```rust
// src/components/shared/file_meta.rs
use crate::app::AppContext;
use crate::models::{FsEntry, VirtualPath};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileMeta {
    pub description: String,
    pub size: Option<u64>,
    pub modified: Option<u64>,
    pub date: Option<String>,
    pub tags: Vec<String>,
}

impl FileMeta {
    pub fn has_display_meta(&self) -> bool { /* unchanged */ }
    pub fn clean_date(&self) -> Option<String> { /* unchanged */ }
    pub fn clean_tags(&self) -> Vec<String> { /* unchanged */ }
}

/// Project the `FsEntry` at `path` into a `FileMeta`. Returns `None` for
/// directories, missing entries, or non-`File` variants.
pub fn file_meta_for_path(ctx: AppContext, path: &VirtualPath) -> Option<FileMeta>;
```

`use_preview` becomes:

```rust
let file_meta = Signal::derive(move || {
    selection
        .get()
        .filter(|s| !s.is_dir)
        .and_then(|s| crate::components::shared::file_meta_for_path(ctx, &s.path))
});
```

## 5. Tests

- Existing tests cover behavior; nothing should change. Run the full suite to confirm.
- No new tests in this phase. The helper is exercised end-to-end by the explorer preview tests already.

## 6. Acceptance

- `cargo test --lib` green.
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `trunk build` green.
- Explorer preview behaves identically: clicking a file in the explorer surfaces the same metadata as before.
- `code-reviewer` clears with no CRITICAL/HIGH findings.

## 7. Self-review

- Placeholders / TODOs: none.
- Contradictions with master: none — master §6 Phase 1 matches.
- Scope creep risk: tempted to "while we're here" rename `FileMetaStrip` — explicitly deferred.
- Risk: an external consumer imports `explorer::preview::FileMeta`. Verified by grep — no external consumers. Re-exporting from `preview/mod.rs` after the move keeps any future implicit consumers working.
