# Phase 2 — Plan: Reader look (View only)

**Date:** 2026-04-30
**Design:** `docs/superpowers/specs/2026-04-30-reader-redesign-phase2-design.md`

## Implementation order

Code first (small to large), then CSS, then wire-up.

### Step 1 — `meta.rs`
- Create `src/components/reader/meta.rs`.
- Define `ReaderMeta` struct.
- Implement `pub fn reader_meta(ctx: AppContext, intent: &ReaderIntent) -> ReaderMeta`:
  - Resolve `node_path` from intent variants.
  - `title` = `node_path.file_name()` with extension trimmed.
  - Call `shared::file_meta_for_path(ctx, &node_path)`; default `FileMeta` if `None`.
  - `modified_iso` = `meta.modified.map(|ts| format_date_iso(ts as i64 / 1000))`.
  - `date` = `meta.clean_date()`.
  - `size_pretty` = `meta.size.map(format_size)`.
  - `tags` = `meta.clean_tags()`.
  - `description` = `meta.description.trim().to_string()`.
  - `media_type_hint` = match per design §4.1.
- Add `#[cfg(test)] mod tests` per design §7.1. Tests use synthetic `ReaderIntent` + `FileMeta` constructors and don't touch `AppContext` — therefore extract a pure inner helper `fn build_reader_meta(intent: &ReaderIntent, meta: FileMeta) -> ReaderMeta` that the public `reader_meta` calls after fetching `meta`.

### Step 2 — `title_block.rs`
- Create `src/components/reader/title_block.rs`.
- Define `RowSpec` enum: `Type { tag, hint }`, `Size { value }`, `Date { value }`, `Tags { items }`, `Caption { text }`.
- Implement `pub fn rows_for(intent: &ReaderIntent, meta: &ReaderMeta) -> Vec<RowSpec>` per design §4.3 with the date-resolution logic.
- Add `#[component] pub fn Ident(meta: Memo<ReaderMeta>)` and `#[component] pub fn TitleBlock(intent: Memo<ReaderIntent>, meta: Memo<ReaderMeta>)`.
- TitleBlock renders `MetaTable` containing `MetaRow`s built from `rows_for(...)`. No empty rows.
- Add `#[cfg(test)]` covering the date-resolution logic per design §7.1.

### Step 3 — `views/*` files
- Create `src/components/reader/views/mod.rs` and the six view files.
- Each view is a thin component matching the signatures in design §4.4. Move logic from current `Reader` match arms.
- `views/markdown.rs` keeps two components: `MarkdownReaderView` (uses `MarkdownView`) and `MarkdownEditorView` (textarea — current implementation lifted unchanged).
- `views/pdf.rs` builds the `pdfFrame` + `pdfChrome` markup wrapping the iframe. Download/open buttons use the `url` directly.

### Step 4 — `reader/mod.rs` cleanup
- Strip the inline match arms from the View body — call into `views/*` instead.
- Add `meta` and `intent_memo` Memos.
- Render `Ident` + `TitleBlock` above the toolbar.
- Keep `ReaderToolbar` markup as-is (Phase 3 replaces).
- Keep `load_renderer_content`, `load_asset`, `load_redirect`, `iso_today` in `mod.rs`.
- Update imports for the new submodules.
- Verify final line count ≤ 300.

### Step 5 — `reader.module.css`
- Replace contents wholesale.
- Use only Tier-3 tokens (`--bg-primary`, `--bg-secondary`, `--bg-inset`, `--text-primary`, `--text-dim`, `--text-muted`, `--border-subtle`, `--border-muted`, `--surface-tint`, `--accent`, `--accent-muted`, `--terminal-green`, `--terminal-yellow`).
- Class list per design §5.2.
- Markdown body styling uses `:global(...)` selectors inside `.mdBody`.

### Step 6 — Verify
- `cargo fmt`.
- `cargo test --lib`.
- `cargo check --target wasm32-unknown-unknown --lib`.
- `trunk build`.
- `grep -E '\-\-ink|\-\-hex|\-\-amber' src/components/reader/reader.module.css` → no matches.
- `grep -E '"—"|"\?"' src/components/reader/` → no matches.
- `trunk serve` and walk through manual QA per design §7 / §8.

### Step 7 — Review + commit
- Spawn `superpowers:code-reviewer` with diff + design + master.
- Address CRITICAL/HIGH.
- Update master §6 Phase 2 → Complete; append §10 Decision Log row noting the Modified/Date collapse and unit-test addition.
- Commit:
  ```
  feat(reader): apply archive look to view paths (phase 2)
  ```

## Risk notes

- `Reader::mod.rs` currently has `RendererContent` enum used inside `load_renderer_content`. The split moves render markup into views/ but keeps `RendererContent` as the dispatch shape in `mod.rs`. Each match arm dispatches to a view component.
- `MarkdownEditorView` extraction must keep the same `prop:value` and `on:input` pattern so `draft_dirty` + `draft_body` semantics survive.
- `:global(...)` selectors inside stylance CSS modules: confirmed working in the existing `reader.module.css`. Same approach.
