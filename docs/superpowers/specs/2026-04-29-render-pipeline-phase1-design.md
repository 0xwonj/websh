# Phase 1 — Engine Classification Unification

**Master:** [`2026-04-29-render-pipeline-master.md`](./2026-04-29-render-pipeline-master.md)
**Date:** 2026-04-29
**Status:** Draft

## 1. Problem

Extension-based content classification happens in **three** places today:

1. `src/core/engine/routing.rs:325-370` (`classify_candidate`) — extension → `ResolvedKind`.
2. `src/core/engine/intent.rs:71-83` — `(ResolvedKind::Page, ext)` → specific `RenderIntent` (`HtmlPage` / `MarkdownPage` / fallback `DocumentReader`).
3. `src/components/reader.rs:426-451` — `RenderIntent::DocumentReader` → `FileType::from_path(...)` → final renderer behaviour (Asset / Html / Markdown / Redirect / plain).

Reader's match also has two **dead arms** (`DirectoryListing` / `TerminalApp` returning `Unsupported(...)`) because `RenderIntent` carries variants Reader can never receive — the router never dispatches them to Reader. (Phase 2 removes those; Phase 1 leaves them in place but eliminates the *content* duplication.)

Adding a new file extension today therefore requires touching the engine **and** Reader. The classification policy is split across the engine/UI boundary.

## 2. Goal

Make the engine the single source of truth for content classification. After Phase 1:

- Every extension-based decision lives in `intent.rs` (it may delegate to `FileType::from_path` and `media_type_for_path`).
- `RenderIntent`'s content-bearing variants describe **what to render**, not **how the URL was classified**.
- Reader's `load_renderer_content` becomes a pure dispatcher — no second-tier `match FileType` inside any arm.
- A new file extension is a single-file change in `intent.rs` (with `FileType` if it gains a new bucket).

Phase 1 does **not** address narrowing Reader's input type, the `layout` field's usage, synthetic frames, or router cleanup. Those are Phases 2–5.

## 3. Type Changes

### 3.1 Before

```rust
// src/core/engine/intent.rs
pub enum RenderIntent {
    HtmlPage         { node_path, layout: Option<String> },
    MarkdownPage     { node_path, layout: Option<String> },
    DirectoryListing { node_path, layout: Option<String> },
    TerminalApp      { node_path, layout: Option<String> },
    DocumentReader   { node_path },
    Redirect         { node_path },
    Asset            { node_path, media_type: String },
}
```

### 3.2 After

```rust
pub enum RenderIntent {
    DirectoryListing { node_path, layout: Option<String> },
    TerminalApp      { node_path, layout: Option<String> },

    // Content variants. `layout` is preserved for all three; Phase 3 audits
    // whether documents legitimately carry layout. Until then we forward
    // whatever `node_metadata` returns rather than silently dropping it.
    HtmlContent      { node_path, layout: Option<String> },
    MarkdownContent  { node_path, layout: Option<String> },
    PlainContent     { node_path, layout: Option<String> },

    // Leaf operations.
    Asset            { node_path, media_type: String },
    Redirect         { node_path },
}
```

Net change: `HtmlPage` + `MarkdownPage` + `DocumentReader` collapse into `HtmlContent` + `MarkdownContent` + `PlainContent`. Page/Document came from `ResolvedKind` (routing internals); the renderer never used the distinction.

### 3.3 Behaviour change to call out

Documents previously did not carry their `layout` field (the `DocumentReader` variant lacked the field). After Phase 1, `MarkdownContent { node_path: "/blog/hello.md", layout }` will have whatever `node_metadata("/blog/hello.md").layout` returns. In the current codebase this is `None` for files (layouts live on directory metadata), so this is a no-op in practice. Phase 3's `layout` audit will confirm and decide whether the field stays.

## 4. Build Logic (`build_render_intent`)

The classification table after Phase 1:

| `ResolvedKind` | Extension                                     | Resulting `RenderIntent`                |
|----------------|-----------------------------------------------|------------------------------------------|
| `Directory`    | n/a                                           | `DirectoryListing`                       |
| `App`          | n/a                                           | `TerminalApp`                            |
| `Redirect`     | n/a                                           | `Redirect`                               |
| `Asset`        | `.png` / `.jpg` / `.jpeg` / `.gif` / `.webp` / `.svg` / `.pdf` | `Asset { media_type }` |
| `Asset`        | other                                         | `Asset { media_type: "application/octet-stream" }` |
| `Page`         | `.html` / `.htm`                              | `HtmlContent`                            |
| `Page`         | `.md`                                         | `MarkdownContent`                        |
| `Page`         | other                                         | `PlainContent`                           |
| `Document`     | `.html` / `.htm`                              | `HtmlContent`                            |
| `Document`     | `.md`                                         | `MarkdownContent`                        |
| `Document`     | `.pdf` / `.png` / `.jpg` / `.jpeg` / `.gif` / `.webp` / `.svg` | `Asset { media_type }` |
| `Document`     | `.link`                                       | `Redirect`                               |
| `Document`     | other                                         | `PlainContent`                           |

The implementation reuses two existing helpers:

- `crate::models::FileType::from_path` — extension → `FileType` enum.
- `crate::utils::media_type_for_path` (`src/utils/asset.rs:27`) — extension → media-type string.

Sketch:

```rust
pub fn build_render_intent(fs: &GlobalFs, resolution: &RouteResolution) -> Option<RenderIntent> {
    let layout = fs
        .node_metadata(&resolution.node_path)
        .and_then(|meta| meta.layout.clone());
    let path = &resolution.node_path;

    Some(match resolution.kind {
        ResolvedKind::Directory => RenderIntent::DirectoryListing {
            node_path: path.clone(),
            layout,
        },
        ResolvedKind::App => RenderIntent::TerminalApp {
            node_path: path.clone(),
            layout,
        },
        ResolvedKind::Redirect => RenderIntent::Redirect {
            node_path: path.clone(),
        },
        ResolvedKind::Asset => RenderIntent::Asset {
            node_path: path.clone(),
            media_type: media_type_for_path(path.as_str()).to_string(),
        },
        ResolvedKind::Page | ResolvedKind::Document => {
            content_intent_for_node(path, layout)
        }
    })
}

fn content_intent_for_node(path: &VirtualPath, layout: Option<String>) -> RenderIntent {
    match FileType::from_path(path.as_str()) {
        FileType::Html => RenderIntent::HtmlContent {
            node_path: path.clone(),
            layout,
        },
        FileType::Markdown => RenderIntent::MarkdownContent {
            node_path: path.clone(),
            layout,
        },
        FileType::Pdf | FileType::Image => RenderIntent::Asset {
            node_path: path.clone(),
            media_type: media_type_for_path(path.as_str()).to_string(),
        },
        FileType::Link => RenderIntent::Redirect {
            node_path: path.clone(),
        },
        FileType::Unknown => RenderIntent::PlainContent {
            node_path: path.clone(),
            layout,
        },
    }
}
```

## 5. Consumer Updates

### 5.1 `src/components/reader.rs`

`load_renderer_content` becomes:

```rust
async fn load_renderer_content(
    ctx: AppContext,
    path: VirtualPath,
    intent: RenderIntent,
) -> Result<RendererContent, String> {
    match intent {
        RenderIntent::HtmlContent { .. } => ctx
            .read_text(&path)
            .await
            .map(|html| RendererContent::Html(rendered_from_html(sanitize_html(&html))))
            .map_err(|e| e.to_string()),
        RenderIntent::MarkdownContent { .. } => ctx
            .read_text(&path)
            .await
            .map(|markdown| RendererContent::Html(render_markdown(&markdown)))
            .map_err(|e| e.to_string()),
        RenderIntent::PlainContent { .. } => ctx
            .read_text(&path)
            .await
            .map(RendererContent::Text)
            .map_err(|e| e.to_string()),
        RenderIntent::Asset { .. } => load_asset(ctx, &path).await,
        RenderIntent::Redirect { .. } => load_redirect(ctx, &path).await,
        RenderIntent::DirectoryListing { .. } | RenderIntent::TerminalApp { .. } => {
            Ok(RendererContent::Unsupported(
                "directory and terminal surfaces do not route to Reader".into(),
            ))
        }
    }
}
```

The dead `DirectoryListing` / `TerminalApp` arms remain (Phase 2 deletes them via `ReaderIntent`). The internal `match FileType` is gone.

`reader.rs:129` (`raw_source` resource) currently checks `FileType::from_path(path) == FileType::Markdown` to decide whether to fetch the raw source for the editor. That check stays as-is — it's an editor concern, not a render-intent concern. Phase 3 may revisit if the editor's Markdown gating wants to align with `MarkdownContent`.

### 5.2 `src/components/router.rs`

Three `RenderIntent::*` references update:

- `router.rs:97-100` (focus effect, currently `TerminalApp | DirectoryListing`): unchanged — those variant names are not renamed.
- `router.rs:134-146` (dispatch match): Reader no longer receives `DocumentReader`; rename arms to handle `HtmlContent | MarkdownContent | PlainContent`. Effective change: `_ => Reader` arm continues to catch all content variants plus `Asset` / `Redirect`, so the match shape barely shifts.
- `router.rs:174` (`new_compose_frame`): `intent: RenderIntent::DocumentReader { node_path }` becomes `intent: RenderIntent::MarkdownContent { node_path, layout: None }` (the compose flow always edits Markdown frontmatter).

### 5.3 `src/components/terminal/shell.rs`

`shell.rs:118` matches `RenderIntent::DirectoryListing { .. }`. Variant unchanged — no edit needed.

### 5.4 Tests

`intent.rs` already has five tests (`builds_html_page_intent`, `builds_markdown_page_intent`, `builds_terminal_app_intent`, `builds_directory_listing_intent`, `builds_redirect_intent_with_source_node_path`). Update to the new variants and add three more:

- `builds_html_content_intent_for_document` — `/blog/hello.html` → `HtmlContent`.
- `builds_markdown_content_intent_for_document` — `/blog/hello.md` → `MarkdownContent`.
- `builds_asset_intent_for_pdf_document` — `/papers/draft.pdf` → `Asset { media_type: "application/pdf" }`.
- `builds_redirect_intent_for_link_document` — `/links/x.link` → `Redirect`.
- `builds_plain_content_intent_for_unknown_document` — `/notes/x.txt` → `PlainContent`.

The final test, ensuring extension-policy is single-source, is implicit: any new bucket added is exercised by an `intent.rs` test.

## 6. File Inventory

| File | Change |
|---|---|
| `src/core/engine/intent.rs` | Replace `RenderIntent` variants; rewrite `build_render_intent`; add `content_intent_for_node` helper; update existing tests; add five new tests. |
| `src/core/engine/mod.rs` | No change — re-exports `RenderIntent` already. |
| `src/components/reader.rs` | Drop `use crate::models::FileType` if no longer needed (line 22 still uses it for `raw_source` gating, so keep import); rewrite `load_renderer_content` match. |
| `src/components/router.rs` | Update dispatch match arms; update `new_compose_frame` synthetic intent. |
| `src/components/terminal/shell.rs` | None. |

## 7. Risks

1. **Behaviour drift on document layout** (§3.3) — addressed by Phase 3 audit.
2. **`/new` compose flow** uses `RenderIntent::DocumentReader` synthetically (`router.rs:174`). Switching to `MarkdownContent { layout: None }` matches the actual rendering behaviour of `/new` (it edits raw markdown), but verify the Reader's `is_new_route` / `derive_new_path` flow still works post-rename.
3. **Variant names in error messages / logs** — none currently log variant names. Verified via `grep RenderIntent::DocumentReader` returning only the five sites listed in §5.

## 8. Acceptance

- `cargo test --lib` green, including the new intent tests.
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `trunk build` green.
- `grep -rn 'FileType::from_path' src/components/reader.rs` returns at most the `raw_source` line — no occurrence inside `load_renderer_content`.
- Manually open `/`, `/ledger`, a markdown post, an html page, a PDF, an image, and a redirect link in `trunk serve` — all render as before.
- Code-reviewer agent has cleared the diff (no outstanding CRITICAL or HIGH findings).
