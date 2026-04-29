# Phase 2 — Narrow `ReaderIntent` Type

**Master:** [`2026-04-29-render-pipeline-master.md`](./2026-04-29-render-pipeline-master.md)
**Date:** 2026-04-29
**Status:** Draft

## 1. Problem

After Phase 1, `Reader::load_renderer_content` still has two arms that exist only because `RenderIntent` is over-broad:

```rust
RenderIntent::DirectoryListing { .. } => Ok(RendererContent::Unsupported(
    "Directory listings are handled by the explorer.".to_string(),
)),
RenderIntent::TerminalApp { .. } => Ok(RendererContent::Unsupported(
    "Applications are handled by websh.".to_string(),
)),
```

Both arms are unreachable in practice — the router's dispatch (`router.rs:132-146`) never sends `DirectoryListing` or `TerminalApp` to Reader. They exist purely because Rust's match-exhaustiveness forces them. They show up as dead code in `RendererContent::Unsupported`, which itself only exists to host these two strings.

The fix is structural, not cosmetic: narrow Reader's input type so the impossible arms become unrepresentable.

## 2. Goal

Introduce a `ReaderIntent` enum containing only the variants Reader can handle. Make Reader's input type `Memo<ReaderFrame>`, where `ReaderFrame` mirrors `RouteFrame` but with `intent: ReaderIntent`. Construction happens in the router, where the variant is already pattern-matched. After Phase 2:

- `Reader` cannot syntactically receive `DirectoryListing` or `TerminalApp`.
- The `Unsupported` arm is removed; `RendererContent::Unsupported` stays only if at least one legitimate use survives (it doesn't — see §5.3).
- The router's dispatch match constructs a `ReaderFrame` inline for each Reader-bound variant. The compiler enforces that every Reader-bound `RenderIntent` variant is mapped.

Phase 2 does not address the `layout` audit (Phase 3), synthetic frames (Phase 4), or router cleanup (Phase 5).

## 3. Type Changes

### 3.1 New types — `components/reader.rs`

```rust
/// Reader-bound subset of `RenderIntent`. Constructed by the router; carries
/// only the variants `Reader` can actually render.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderIntent {
    Html         { node_path: VirtualPath, layout: Option<String> },
    Markdown     { node_path: VirtualPath, layout: Option<String> },
    Plain        { node_path: VirtualPath, layout: Option<String> },
    Asset        { node_path: VirtualPath, media_type: String },
    Redirect     { node_path: VirtualPath },
}

/// Reader's narrowed equivalent of `RouteFrame`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderFrame {
    pub request: RouteRequest,
    pub resolution: RouteResolution,
    pub intent: ReaderIntent,
}
```

`ReaderIntent` lives in `reader.rs` (UI-layer narrowing of an engine-layer type). The variant names drop the `Content` suffix from the new `RenderIntent` variants — within `ReaderIntent` they're already implicitly Reader-bound, so `Html` / `Markdown` / `Plain` are clearer.

### 3.2 Removed

- `RendererContent::Unsupported(String)` — the only consumer was the two dead arms; nothing else constructs or matches it. Verified via `grep RendererContent::Unsupported` showing only `reader.rs`.

### 3.3 Reader signature change

```rust
// before
#[component]
pub fn Reader(route: Memo<RouteFrame>) -> impl IntoView { ... }

// after
#[component]
pub fn Reader(frame: Memo<ReaderFrame>) -> impl IntoView { ... }
```

Inside Reader, `route.get().resolution.node_path` becomes `frame.get().resolution.node_path`, etc. — same field paths, different prop name and intent type.

## 4. Construction

The router's existing intent match already discriminates per-variant. Phase 2 weaves `ReaderIntent` construction into the Reader-bound arms. Where the router previously had a `_ => view! { <Reader route=... /> }` catch-all, Phase 2 expands it into one arm per Reader-bound variant so the conversion is exhaustive.

Sketch — `src/components/router.rs:132-146` after Phase 2:

```rust
match route.get() {
    Some(frame) => {
        let request = frame.request.clone();
        let resolution = frame.resolution.clone();
        match frame.intent.clone() {
            RenderIntent::TerminalApp { .. } => view! { <Shell ... /> }.into_any(),
            RenderIntent::DirectoryListing { .. }
                if frame.surface() == RouteSurface::Explorer =>
            {
                view! { <Shell ... /> }.into_any()
            }
            RenderIntent::DirectoryListing { .. } => view! { <LedgerPage ... /> }.into_any(),
            RenderIntent::HtmlContent { node_path, layout } => mount_reader(
                request, resolution, ReaderIntent::Html { node_path, layout },
            ),
            RenderIntent::MarkdownContent { node_path, layout } => mount_reader(
                request, resolution, ReaderIntent::Markdown { node_path, layout },
            ),
            RenderIntent::PlainContent { node_path, layout } => mount_reader(
                request, resolution, ReaderIntent::Plain { node_path, layout },
            ),
            RenderIntent::Asset { node_path, media_type } => mount_reader(
                request, resolution, ReaderIntent::Asset { node_path, media_type },
            ),
            RenderIntent::Redirect { node_path } => mount_reader(
                request, resolution, ReaderIntent::Redirect { node_path },
            ),
        }
    }
    None => view! { <NotFound /> }.into_any(),
}
```

Where `mount_reader` is a small helper inside `router.rs`:

```rust
fn mount_reader(
    request: RouteRequest,
    resolution: RouteResolution,
    intent: ReaderIntent,
) -> AnyView {
    let frame = ReaderFrame { request, resolution, intent };
    view! { <Reader frame=Memo::new(move |_| frame.clone()) /> }.into_any()
}
```

The compiler now enforces that every variant of `RenderIntent` is handled at the dispatch site — no catch-all, no possibility of forgetting a variant.

## 5. Consumer Updates

### 5.1 `src/components/reader.rs`

- Add `pub enum ReaderIntent` and `pub struct ReaderFrame` (§3.1).
- Change the `Reader` component's prop from `route: Memo<RouteFrame>` to `frame: Memo<ReaderFrame>`. Mechanical rename inside the body: `route.get()` → `frame.get()`.
- Rewrite `load_renderer_content` to take `ReaderIntent`:
  ```rust
  async fn load_renderer_content(
      ctx: AppContext,
      path: VirtualPath,
      intent: ReaderIntent,
  ) -> Result<RendererContent, String> {
      match intent {
          ReaderIntent::Markdown { .. } => /* read_text + render_markdown */,
          ReaderIntent::Html     { .. } => /* read_text + sanitize + render */,
          ReaderIntent::Plain    { .. } => /* read_text + Text */,
          ReaderIntent::Asset    { .. } => load_asset(ctx, &path).await,
          ReaderIntent::Redirect { .. } => load_redirect(ctx, &path).await,
      }
  }
  ```
  No `DirectoryListing` / `TerminalApp` arms; no `_` catch-all.
- Remove `RendererContent::Unsupported` from `RendererContent` enum and its handling in the view dispatch (the corresponding view arm in `reader.rs:318-320`).
- Update the `content` `LocalResource` factory to read `frame.get().intent.clone()` instead of `frame.get().intent.clone()` (the call shape is unchanged, only the prop name differs).

### 5.2 `src/components/router.rs`

- Add `use crate::components::reader::{ReaderFrame, ReaderIntent};` (or relative module path).
- Replace the dispatch match per §4.
- Add the `mount_reader` helper.
- The `Effect::new(...)` at `router.rs:90-105` (the focus side-effect) currently checks:
  ```rust
  let is_reader = route.get().is_some_and(|frame| {
      !matches!(
          frame.intent,
          RenderIntent::TerminalApp { .. } | RenderIntent::DirectoryListing { .. }
      )
  });
  ```
  This still works on `RenderIntent` (it inspects the unprocessed route, before narrowing). No change. Phase 5 may rework this side-effect entirely.
- The synthetic-frame helpers (`new_compose_frame`, `ledger_filter_frame`, `builtin_home_frame`) construct `RouteFrame` and feed it into the match. They route to `<Reader>` indirectly via the new dispatch. The match arms in §4 will pick up these synthetic intents the same way real ones flow through, so no synthetic-helper change is needed in Phase 2. (Phase 4 audits the synthetic-frame approach overall.)

### 5.3 `RendererContent::Unsupported` removal

Verified via grep:
- Constructed only in the two dead arms of `load_renderer_content`.
- Pattern-matched only in the view dispatch (`reader.rs:318-320`).
- No external consumer.

Both sites disappear in Phase 2, so the variant goes too.

### 5.4 Tests

`reader.rs` has no existing unit tests. Phase 2 adds:

- `reader_intent_round_trip_html` — `ReaderIntent::Html { ... }` carries layout faithfully.
- `reader_intent_round_trip_asset` — `media_type` preserved.
- `reader_intent_round_trip_redirect` — `node_path` preserved.

These are minimal type-level smoke tests; they protect the conversion shape, not the runtime render path. No test for the router's dispatch (component-mount tests are not part of this codebase's test infrastructure).

## 6. File Inventory

| File | Change |
|---|---|
| `src/components/reader.rs` | Add `ReaderIntent` + `ReaderFrame`; rename `route` prop → `frame`; rewrite `load_renderer_content`; drop `Unsupported`; +3 unit tests. |
| `src/components/router.rs` | Replace dispatch match per §4; add `mount_reader` helper; add new `use`. |
| `src/components/mod.rs` (if applicable) | Re-export `ReaderIntent` / `ReaderFrame` if needed by other components — verify; likely no change. |

## 7. Risks

| Risk | Mitigation |
|---|---|
| Reader's prop name change (`route` → `frame`) cascades to template usage. | Mechanical rename; cargo will flag every site. |
| The `is_reader` Effect still uses `RenderIntent` variant names. | `TerminalApp` / `DirectoryListing` variant names are unchanged — no edit needed. Phase 5 reworks this. |
| `RendererContent::Unsupported` removal could break a forgotten consumer. | grep verified §5.3; cargo would error otherwise. |
| Synthetic frames (`new_compose_frame`) construct `RenderIntent::MarkdownContent` (after Phase 1) — still flows through the new dispatch. | The match in §4 has an explicit arm for `MarkdownContent`, so synthetic frames continue to mount Reader. |

## 8. Acceptance

- `cargo test --lib` green (508 + 3 new = 511).
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `trunk build` green.
- `grep -nF 'RendererContent::Unsupported' src/` returns no matches.
- `grep -nF 'DirectoryListing' src/components/reader.rs` returns no matches (the dead arms are gone).
- `grep -nF 'TerminalApp' src/components/reader.rs` returns no matches.
- The router's dispatch match has no `_ =>` catch-all for Reader-bound intents.
- Manual QA in `trunk serve`: a markdown post, an HTML page, a PDF, an image, a redirect link — all render as before.
- Code-reviewer cleared with no outstanding CRITICAL or HIGH.
