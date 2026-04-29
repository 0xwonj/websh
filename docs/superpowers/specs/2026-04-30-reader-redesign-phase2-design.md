# Phase 2 — Reader look (View only)

**Date:** 2026-04-30
**Master:** `docs/superpowers/specs/2026-04-30-reader-redesign-master.md`
**Status:** Approved (autonomous run)

## 1. Scope

Apply the archive look to the Reader's View paths. Split `reader/mod.rs` into focused modules. Add the `Ident` strip and `TitleBlock` (h1 + per-intent meta table). Add a PDF `Abstract` section sourced from `FileMeta.description`.

The toolbar still uses the existing layout — Phase 3 replaces it. Edit-mode textarea styling lands in this phase only insofar as it shares the page's CSS module; no new keybindings, no new variants.

## 2. Out of scope

- Toolbar redesign and keybindings — Phase 3.
- Split mode, code/hex renderers, append banner, tweaks panel, custom sig chip — never (master §2).
- Frontmatter title parsing, sha-256 / signed-by rows — out (master §3, §4.4).

## 3. File layout (final shape after this phase)

```
src/components/reader/
  mod.rs              — state · dispatch · save · current ReaderToolbar (untouched in P2)
  intent.rs           — unchanged
  meta.rs        ★    — pulls FileMeta + format helpers; produces ReaderMeta value
  title_block.rs ★    — Ident strip + h1 + MetaTable; per-intent row policy
  views/
    mod.rs       ★
    markdown.rs  ★    — MarkdownReaderView (View) and MarkdownEditorView (Edit)
    html.rs      ★
    plain.rs     ★
    pdf.rs       ★    — TitleBlock + optional Abstract section + iframe wrapper
    asset.rs     ★    — image
    redirect.rs  ★    — placeholder text
  reader.module.css   — rewritten end-to-end against Tier-3 tokens
```

`mod.rs` shrinks to: `Reader` component (state, dispatch, save), the existing `ReaderToolbar` block (lifted unchanged for now — Phase 3 replaces), `iso_today`, `load_renderer_content`, `load_asset`, `load_redirect`. Target ≤ 250 lines.

## 4. Data flow

### 4.1 `meta.rs`

```rust
pub struct ReaderMeta {
    pub title: String,                    // filename stem (extension trimmed)
    pub canonical_path: VirtualPath,      // for Ident left side
    pub modified_iso: Option<String>,     // formatted YYYY-MM-DD or None
    pub date: Option<String>,             // FileMeta.date, trimmed
    pub size_pretty: Option<String>,      // formatted size or None
    pub tags: Vec<String>,                // FileMeta clean_tags()
    pub description: String,              // FileMeta.description (trim'd; "" if none)
    pub media_type_hint: Option<&'static str>, // "UTF-8 · CommonMark", "UTF-8 · LF", "image/*", …
}

pub fn reader_meta(ctx: AppContext, intent: &ReaderIntent) -> ReaderMeta;
```

`reader_meta` calls `shared::file_meta_for_path` and combines that with `intent` to produce a single value the views consume. View files don't call `file_meta_for_path` directly.

`title` defaults to `canonical_path.file_name()` with extension stripped. No frontmatter parsing.

`media_type_hint` is the small dim suffix shown next to the `type` tag (e.g. `markdown   UTF-8 · CommonMark`):
- `Markdown` → `Some("UTF-8 · CommonMark")`
- `Html` → `Some("UTF-8 · sanitized")`
- `Plain` → `Some("UTF-8 · LF")`
- `Asset { media_type }` if starts with `image/` → `None` (the type tag already shows `image/png`)
- `Pdf` (`Asset` with `application/pdf`) → `None`
- `Redirect` → `None`

### 4.2 `title_block.rs`

```rust
#[component]
pub fn Ident(meta: Memo<ReaderMeta>) -> impl IntoView;

#[component]
pub fn TitleBlock(intent: Memo<ReaderIntent>, meta: Memo<ReaderMeta>) -> impl IntoView;
```

`Ident` renders:
```
<div class=css::ident>
    {if !canonical_path.is_empty(): <span class=css::identId><b>{canonical_path}</b></span>}
    {if let Some(rev) = display_date: <span class=css::identRev>{rev}</span>}
</div>
```
**No `—` / placeholder fallbacks.** If a side has no value, that span is not emitted. If both are empty, the whole strip is not emitted (per master §3 "no placeholders" rule).

`display_date` is the same value resolved by §4.3 — `meta.date` if present, else `modified_iso`, else `None`.

`TitleBlock` renders `<h1>{title}</h1>` followed by a `MetaTable` whose rows are generated per the intent (§4.3 below). Empty rows are not emitted.

### 4.3 Per-intent row policy

Each row is `(label, content)`. Rows whose `content` is empty are skipped.

| Intent     | Rows produced                                                                                  |
|------------|------------------------------------------------------------------------------------------------|
| Markdown   | `Type` (`<tag>markdown</tag><dim>{hint}</dim>`) · `Date` · `Tags` (chips)                      |
| Html       | `Type` (`<tag>html</tag><dim>{hint}</dim>`) · `Date`                                           |
| Plain      | `Type` (`<tag>text</tag><dim>{hint}</dim>`) · `Size` · `Date`                                  |
| Asset/PDF  | `Type` (`<tag>application/pdf</tag>`) · `Size` · `Date` · `Tags`                               |
| Asset/img  | `Type` (`<tag>{media_type}</tag>`) · `Size` · `Date` · `Caption` (description)                 |
| Redirect   | n/a — redirect view has no title block.                                                        |

**One date row only**, labeled `Date`. Value resolves as: `meta.date` (author-declared, manifest field) **else** `modified_iso` (filesystem timestamp formatted YYYY-MM-DD) **else** row omitted. Master §4.4 originally listed both `Modified` and `Date` for Markdown; the two-row arrangement is awkward when they conflict and redundant when they don't, so this design collapses to one row that prefers author intent over filesystem mechanics. Per advisor recommendation. Master §10 Decision Log will record the divergence.

### 4.4 View files

Each `views/<intent>.rs` exposes one component. They share styling through the same `css::` module. None of them know about state or dispatch — they receive what they need as props.

```rust
// views/markdown.rs
#[component]
pub fn MarkdownReaderView(rendered: Signal<RenderedMarkdown>) -> impl IntoView;
#[component]
pub fn MarkdownEditorView(
    draft_body: RwSignal<String>,
    on_input_dirty: Callback<()>,  // bumps draft_dirty
) -> impl IntoView;

// views/html.rs
#[component]
pub fn HtmlReaderView(rendered: Signal<RenderedMarkdown>) -> impl IntoView;

// views/plain.rs
#[component]
pub fn PlainReaderView(text: String) -> impl IntoView;

// views/pdf.rs
#[component]
pub fn PdfReaderView(
    title: Signal<String>,    // for chrome bar
    url: String,
    size_pretty: Option<String>,
    abstract_text: String,    // empty -> section omitted
) -> impl IntoView;

// views/asset.rs
#[component]
pub fn AssetReaderView(url: String, media_type: String, alt: String) -> impl IntoView;

// views/redirect.rs
#[component]
pub fn RedirectingView() -> impl IntoView;
```

`mod.rs::Reader` orchestrates: builds `ReaderMeta` → renders `Ident` + `TitleBlock` + (View body | Edit body) + `AttestationSigFooter`. The View body branch dispatches to the right `views/*` component based on `RendererContent` (which already exists).

## 5. CSS

### 5.1 Token map (prototype → Tier-3, used in `reader.module.css`)

| Prototype     | Used as                                | Tier-3 token                                    |
|---------------|----------------------------------------|-------------------------------------------------|
| `--bg`/`--paper` | page bg                              | `--bg-primary`                                  |
| `--ink`       | strong text                            | `--text-primary`                                |
| `--ink-dim`   | dim text                               | `--text-dim`                                    |
| `--ink-faint` | faint text / borders for kbd hints     | `--text-muted`                                  |
| `--rule`      | hairlines                              | `--border-subtle`                               |
| `--rule-bright` | callouts / blockquote                | `--border-muted`                                |
| `--chrome`    | bar / status backgrounds               | `--bg-secondary`                                |
| `--chrome-2`  | pre/code blocks                        | `--bg-inset`                                    |
| `--tint`      | meta-table key-cell tint               | `--surface-tint`                                |
| `--accent`    | accents / current item                 | `--accent`                                      |
| `--accent-dim` | accent halo                           | `--accent-muted`                                |
| `--hex`       | code spans, "synced" dot               | `--terminal-green`                              |
| `--amber`     | "dirty" / unsaved                      | `--terminal-yellow`                             |

No new tokens. No `:root { … }` block in `reader.module.css`.

### 5.2 Class set (final)

`reader.module.css` exports:
```
surface         — outer wrapper (col flex, min-height 100vh)
page            — main column (max-width 92ch, mono font)
ident           — paper-id + revision strip
identId
identRev
titleBlock      — h1 + meta wrapper
title           — h1
metaTable       — bordered table
metaRow         — grid 110px 1fr
metaKey
metaValue
metaTag         — small bordered chip (re-used inline)
metaDim
sectionTitle    — h2 for "Abstract", "Document"
mdBody          — body wrapper for MarkdownReaderView (h1/h2/p/ul/code/pre…)
htmlBody        — same idea, less aggressive (HTML may bring its own styling)
rawText         — <pre> for plain
pdfFrame        — iframe wrapper (border + chrome bar)
pdfChrome       — top status bar (filename · size · download · open)
pdfChromeDot
pdfChromeTitle
pdfChromeCtrl
pdfViewer       — the iframe itself
imageFigure
image
redirecting
loading
error
errorBanner
toolbar         — phase-3-only; for P2 keep current toolbar markup but apply fresh palette
toolbarLabel
toolbarActions
actionButton
actionButtonPrimary
editorTextarea
```

The toolbar classes still exist in P2 (Reader's existing toolbar markup keeps using them). P3 will rename / restructure.

The `mdBody` class wraps `MarkdownView`'s injected HTML and uses `:global(...)` selectors (since the HTML inside is server-rendered). Same pattern Home uses on `.home p / .home h1` etc.

### 5.3 Layout details

- `surface` uses `min-height: 100vh`, column flex.
- `page` uses `max-width: 92ch`, `padding: 22px 28px 0`, `font-family: var(--font-mono)`, `font-size: 14px`, `line-height: 1.6`.
- `ident` uses flex space-between, `border-bottom: 1px solid var(--border-subtle)`, `padding-bottom: 6px`, `margin-bottom: 14px`.
- `metaTable` uses 1px solid `--border-subtle` border, `metaRow` uses `grid-template-columns: 110px 1fr` and bottom border between rows.
- `sectionTitle` uses `h2` styling: 14px, semibold, margin `18px 0 6px`, optional `[data-n]::before` per Home convention.
- `mdBody h1` 20px / `mdBody h2` 14px / `mdBody p` 13px / `mdBody pre` border + bg-inset background, etc.

## 6. Reader's `mod.rs` after split

```rust
#[component]
pub fn Reader(frame: Memo<ReaderFrame>) -> impl IntoView {
    // existing state setup: canonical_path, filename, attestation_route,
    // author_mode, is_new_route, edit_visible, draft_body, draft_dirty,
    // save_error, saving, refetch_epoch, raw_source, content, mode,
    // chrome_route — unchanged.

    let meta = Memo::new(move |_| reader_meta(ctx, &frame.get().intent));
    let intent_memo = Memo::new(move |_| frame.get().intent.clone());

    view! {
        <div class=css::surface>
            <SiteChrome route=chrome_route />
            <main class=css::page>
                <Ident meta=meta />
                <TitleBlock intent=intent_memo meta=meta />
                <ReaderToolbar … />        // unchanged for P2
                <Show when=move || mode.get() == ReaderMode::Edit fallback=…>
                    <MarkdownEditorView … />
                </Show>
                <AttestationSigFooter route=attestation_route />
            </main>
        </div>
    }
}
```

The View-fallback dispatches `RendererContent` to the right view component (verbatim translation of the current match arms).

## 7. Tests

`reader_meta` and the per-intent row builder are pure functions — they get unit tests.

### 7.1 New unit tests

`src/components/reader/meta.rs` adds a `#[cfg(test)] mod tests` covering:
- Markdown intent + populated `FileMeta` → `ReaderMeta { title, date: Some(...), tags: ..., media_type_hint: Some("UTF-8 · CommonMark") }`.
- Plain intent + size-only `FileMeta` → no date / tags rows.
- PDF intent + description-only `FileMeta` → description preserved on `ReaderMeta`.
- Image intent + empty `FileMeta` → no description fallback.
- Redirect intent → `ReaderMeta` is still constructable but downstream views won't read it.

`src/components/reader/title_block.rs` adds tests covering:
- `meta.date = Some("2026-04-22")`, `modified_iso = Some("2026-04-30")` → row value resolves to `"2026-04-22"` (author-declared wins).
- `meta.date = None`, `modified_iso = Some("2026-04-30")` → row resolves to `"2026-04-30"`.
- Both `None` → no `Date` row emitted.

These exercise pure data flow; no Leptos render needed (we expose row generation as a `pub fn rows_for(intent: &ReaderIntent, meta: &ReaderMeta) -> Vec<RowSpec>` so tests can assert against `Vec<(label, content_kind)>` without touching `view!`). `RowSpec` is a small enum-or-struct that names the row variant + its data; the component side maps `RowSpec` to `MetaRow` markup.

### 7.2 Existing tests stay green

`ReaderIntent` round-trips, mempool draft / save tests, router tests — all unchanged.

Manual QA checklist (run in `trunk serve` after build):

1. `/about` (or any markdown route) — Ident shows path + modified, h1 shows filename, meta table shows `Type · Modified · Date · Tags` (rows with no value omitted), body renders.
2. `/index.html` (or any html route) — Ident + h1 + Type/Modified rows + body.
3. Plain `.txt` route — Type/Size/Modified rows + `<pre>` body.
4. PDF route — Type/Size/Modified/Tags rows + (if description present) `<h2>Abstract</h2><p>{description}</p>` + `<h2>Document</h2>` + iframe with chrome bar.
5. Image route — Type/Size/Modified/Caption rows + figure/img.
6. Redirect (`.link`) route — short "Redirecting…" text.
7. Toggle palette via SiteChrome picker — reader colors retone (no per-theme stylesheet exists; Tier-3 cascading does the work).
8. Mempool path with author mode — Edit toggle still works (toolbar markup unchanged for P2).
9. `/new` — Edit mode entered with placeholder, save flow works.

## 8. Acceptance

- `cargo test --lib` green.
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `trunk build` green.
- Manual QA checklist (§7) passes for at least: markdown / html / plain / pdf / image / redirect / palette switch / mempool save.
- No `--ink` / `--accent` (the prototype's `--accent`) / `--hex` / `--amber` strings appear in `reader.module.css` (verify via `grep`). All tokens are Tier-3 (`--bg-*`, `--text-*`, `--border-*`, `--surface-tint`, `--archive-bar-bg`, `--accent` (the project's), `--terminal-*`).
- No literal em-dash placeholder (`"—"`) in any new component file (verify via `grep` over `src/components/reader/`). Empty values omit their host element.
- `code-reviewer` clears with no CRITICAL/HIGH.

## 9. Self-review

- **Placeholders**: none. All proposed APIs name concrete types.
- **Contradictions vs master**: none. §4.4 in master is reproduced faithfully here.
- **Scope creep**: tempting to also redesign the toolbar in this phase since it sits next to the metadata. Held off — toolbar shape change brings keybindings + behavior change which deserves its own review.
- **Risks**:
  - `meta.rs::reader_meta` calls `file_meta_for_path` which reads `ctx.view_global_fs.with(...)`. The reactive system needs this to re-run when the FS changes. Wrapping `reader_meta` inside a `Memo::new` should re-track each frame change. Cross-check: signals read inside the Memo body — `ctx.view_global_fs` is a `Signal`, so the `.with` access registers a reactive read. **Verify: the existing `dir_meta` Signal::derive in `preview/hook.rs` works identically — so this pattern is sound.**
  - HTML in `mdBody` uses `:global(...)` selectors. Stylance scoping won't reach into `inner_html` content; this is the same trick `MarkdownView` already relies on in current `reader.module.css`. Pattern continues.
  - `media_type_for_path` is the engine's concern; reader uses media type from the `Asset` intent variant directly. Already classified.
- **What if `description` is multi-line / contains markdown?** The Abstract section renders it as plain `<p>` text. Markdown inside `description` won't be interpreted. This matches how `FileMetaStrip` and the explorer preview already treat it — no escalation here.
