# Phase 1 — Engine Classification Unification — Implementation Plan

**Design:** [`../specs/2026-04-29-render-pipeline-phase1-design.md`](../specs/2026-04-29-render-pipeline-phase1-design.md)
**Master:** [`../specs/2026-04-29-render-pipeline-master.md`](../specs/2026-04-29-render-pipeline-master.md)
**Status:** Approved

## Step ordering rationale

Variant renames cascade to all consumers; intermediate compile errors are unavoidable between Step 1 and Step 3. The plan therefore batches all variant edits before the first cargo check. Tests run after the consumer updates so test failures isolate to logic, not transient API mismatch.

## Steps

### Step 1.1 — Rewrite `intent.rs`

**File:** `src/core/engine/intent.rs`

- Replace `RenderIntent` enum with the 7-variant set from design §3.2.
- Rewrite `build_render_intent` per design §4 (helper `content_intent_for_node`).
- Add imports: `crate::models::FileType`, `crate::utils::media_type_for_path`.
- Update existing tests:
  - `builds_html_page_intent` → assert `HtmlContent { layout: None }` for `/index.html`.
  - `builds_markdown_page_intent` → assert `MarkdownContent { layout: None }` for `/about`.
  - `builds_terminal_app_intent` → unchanged variant name.
  - `builds_directory_listing_intent` → unchanged.
  - `builds_redirect_intent_with_source_node_path` → unchanged.
- Add five new tests per design §5.4:
  - `builds_html_content_intent_for_html_document` (`/blog/hello.html`)
  - `builds_markdown_content_intent_for_md_document` (`/blog/hello.md`)
  - `builds_asset_intent_for_pdf_document` (`/papers/draft.pdf`)
  - `builds_redirect_intent_for_link_document` (`/links/x.link`)
  - `builds_plain_content_intent_for_unknown_document` (`/notes/x.txt`)

### Step 1.2 — Update `reader.rs`

**File:** `src/components/reader.rs`

- Rewrite `load_renderer_content` body per design §5.1.
- Keep the `use crate::models::{FileType, VirtualPath};` import — `raw_source` still uses `FileType::Markdown`.
- Drop `media_type_for_path` from `use crate::utils::{...}` if no longer referenced (verify with cargo).
- Remove the inner `match FileType::from_path(...)` block.
- Keep `Unsupported` arms for `DirectoryListing` / `TerminalApp` (Phase 2 removes them).

### Step 1.3 — Update `router.rs`

**File:** `src/components/router.rs`

- `router.rs:174` (`new_compose_frame`): replace
  ```rust
  intent: RenderIntent::DocumentReader { node_path },
  ```
  with
  ```rust
  intent: RenderIntent::MarkdownContent { node_path, layout: None },
  ```
- `router.rs:132-146` (router dispatch match): the `_ => Reader` catch-all already handles content variants. Verify no explicit `DocumentReader` arm exists outside the catch-all (it does not). No edit needed here unless cargo flags one.

### Step 1.4 — Verify

In order, all green required:

```sh
cargo check --target wasm32-unknown-unknown --lib
cargo test --lib --target $(rustc -vV | sed -n 's/host: //p')   # default host target for tests
trunk build
```

If any step fails, fix root-cause before proceeding to Step 1.5.

### Step 1.5 — Code review

Invoke `superpowers:code-reviewer` agent with:

- Design doc path
- Plan doc path
- Diff scope (intent.rs / reader.rs / router.rs)

Address every CRITICAL and HIGH finding before declaring Phase 1 complete.

### Step 1.6 — Update master

Edit `docs/superpowers/specs/2026-04-29-render-pipeline-master.md`:

- §2 row Phase 1 status → `Complete`.
- §2 row Phase 2 status → `Next`.
- §4 Document Index — add design and plan rows.
- §5 Decision Log — append entry summarising the decomposition + reviewer outcome.
- §7 State — bump active phase pointer to Phase 2.

## Risks (re-checked)

| Risk | Mitigation |
|---|---|
| `/new` compose flow regression after `DocumentReader` → `MarkdownContent` swap | Smoke-test `/new` route in `trunk serve` after Step 1.3 — frontmatter editor should still mount in Edit mode. |
| Documents now carry `layout` (design §3.3) | No-op in current data. Phase 3 audit will confirm. |
| Reader's `raw_source` still uses `FileType::Markdown` and could drift from `MarkdownContent` | Out-of-scope for Phase 1; flagged in design §5.1. Phase 3 may align them. |

## Acceptance (mirrors design §8)

- All commands in Step 1.4 pass.
- `grep -nF 'FileType::from_path' src/components/reader.rs` returns at most the `raw_source` line.
- Manual QA (`/`, `/ledger`, a markdown post, an HTML page, a PDF, an image, a redirect link) — all render as before.
- Code-reviewer: no outstanding CRITICAL or HIGH.
