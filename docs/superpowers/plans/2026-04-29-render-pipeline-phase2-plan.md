# Phase 2 — Narrow `ReaderIntent` Type — Implementation Plan

**Design:** [`../specs/2026-04-29-render-pipeline-phase2-design.md`](../specs/2026-04-29-render-pipeline-phase2-design.md)
**Master:** [`../specs/2026-04-29-render-pipeline-master.md`](../specs/2026-04-29-render-pipeline-master.md)
**Status:** Approved

## Step ordering rationale

`reader.rs` and `router.rs` are tightly coupled by the prop signature. The cleanest sequence is:

1. Add the new types to `reader.rs` (no consumers yet → compiles).
2. Switch Reader's signature, internal references, and `load_renderer_content` over `ReaderIntent` in one shot. Router still passes `route: Memo<RouteFrame>` so this **breaks** until step 3 lands.
3. Update the router's dispatch + add `mount_reader`. Compiles again.
4. Drop `RendererContent::Unsupported`.
5. Add unit tests, verify, review.

Steps 2 and 3 must land together; the working tree is briefly red between Step 2.2 and Step 2.3.

## Steps

### Step 2.1 — Add `ReaderIntent` and `ReaderFrame` to `reader.rs`

**File:** `src/components/reader.rs`

Insert at the top of the module (after the existing `use` block, before `enum ReaderMode`):

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReaderIntent {
    Html     { node_path: VirtualPath, layout: Option<String> },
    Markdown { node_path: VirtualPath, layout: Option<String> },
    Plain    { node_path: VirtualPath, layout: Option<String> },
    Asset    { node_path: VirtualPath, media_type: String },
    Redirect { node_path: VirtualPath },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderFrame {
    pub request: RouteRequest,
    pub resolution: RouteResolution,
    pub intent: ReaderIntent,
}
```

Add `RouteRequest`, `RouteResolution` to the existing `engine` import line.

### Step 2.2 — Switch Reader to `ReaderFrame`

**File:** `src/components/reader.rs`

- Component signature: `pub fn Reader(frame: Memo<ReaderFrame>)` (was `route: Memo<RouteFrame>`).
- Replace every `route.get()` inside the body with `frame.get()`. (Field paths `.resolution.node_path`, `.request.url_path`, `.intent` are identical between `RouteFrame` and `ReaderFrame`.)
- Replace the local `let intent = frame.intent.clone();` inside the `content` `LocalResource` factory — type changes from `RenderIntent` to `ReaderIntent`.
- Rewrite `load_renderer_content`'s match per design §5.1:
  ```rust
  ReaderIntent::Markdown { .. } => /* read_text + render_markdown */
  ReaderIntent::Html     { .. } => /* read_text + sanitize + render */
  ReaderIntent::Plain    { .. } => /* read_text + Text */
  ReaderIntent::Asset    { .. } => load_asset(ctx, &path).await
  ReaderIntent::Redirect { .. } => load_redirect(ctx, &path).await
  ```
  No `DirectoryListing` / `TerminalApp` arms, no `_` catch-all.
- Remove the `RenderIntent` import (no longer used in this file). Keep `push_request_path`, `replace_request_path` imports.

After this step, `cargo check` will fail at `router.rs` until Step 2.3 lands.

### Step 2.3 — Update `router.rs` dispatch

**File:** `src/components/router.rs`

- Add `use crate::components::reader::{ReaderFrame, ReaderIntent};`.
- Replace the inner `match` body of the `Some(frame) =>` arm per design §4. Every `RenderIntent` variant is mapped explicitly; no `_` catch-all.
- Add the `mount_reader` helper:
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
  Place between `RouterView` and `is_builtin_home_route`. Add any missing imports (`AnyView` is already present via Leptos prelude).
- Verify the `Effect::new(...)` at `router.rs:90-105` compiles unchanged — it uses `RenderIntent::TerminalApp` / `RenderIntent::DirectoryListing` variant names which still exist on the engine type.

### Step 2.4 — Drop `RendererContent::Unsupported`

**File:** `src/components/reader.rs`

- Remove the `Unsupported(String)` variant from `enum RendererContent`.
- Remove the corresponding `Ok(RendererContent::Unsupported(message)) => view! { <div class=css::error>{message}</div> }.into_any(),` arm in the view dispatch (currently `reader.rs:318-320` — line numbers have shifted from the earlier read; locate by pattern).

### Step 2.5 — Add unit tests

**File:** `src/components/reader.rs`

Append a `#[cfg(test)] mod reader_intent_tests` at the bottom:

```rust
#[cfg(test)]
mod reader_intent_tests {
    use super::*;

    #[test]
    fn reader_intent_round_trip_html() {
        let intent = ReaderIntent::Html {
            node_path: VirtualPath::from_absolute("/index.html").unwrap(),
            layout: Some("default".to_string()),
        };
        match intent {
            ReaderIntent::Html { layout: Some(l), .. } => assert_eq!(l, "default"),
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn reader_intent_round_trip_asset() {
        let intent = ReaderIntent::Asset {
            node_path: VirtualPath::from_absolute("/cover.png").unwrap(),
            media_type: "image/png".to_string(),
        };
        if let ReaderIntent::Asset { media_type, .. } = intent {
            assert_eq!(media_type, "image/png");
        } else {
            panic!("unexpected variant");
        }
    }

    #[test]
    fn reader_intent_round_trip_redirect() {
        let intent = ReaderIntent::Redirect {
            node_path: VirtualPath::from_absolute("/x.link").unwrap(),
        };
        if let ReaderIntent::Redirect { node_path } = intent {
            assert_eq!(node_path.as_str(), "/x.link");
        } else {
            panic!("unexpected variant");
        }
    }
}
```

These are minimal — they assert the variant shape compiles and round-trips.

### Step 2.6 — Verify

```sh
cargo check --target wasm32-unknown-unknown --lib
cargo test --lib
trunk build
```

Manual smoke test in `trunk serve`: open a markdown post, an HTML page, a PDF, an image, a redirect link.

### Step 2.7 — Code review

Invoke `superpowers:code-reviewer` on the Phase 2 diff.

### Step 2.8 — Update master

- §2 row Phase 2 status → `Complete`; Phase 3 status → `Next`.
- §4 Document Index — add design + plan rows.
- §5 Decision Log — append entry.
- §7 State — bump active phase pointer.

## Risks (re-checked)

| Risk | Mitigation |
|---|---|
| Reader's prop rename (`route` → `frame`) breaks template references. | Cargo will flag every site; mechanical rename. |
| The router's `is_reader` Effect still inspects `RenderIntent`. | Confirmed unchanged; relies on `TerminalApp` / `DirectoryListing` variant names that still exist. |
| Synthetic frames pass `RouteFrame` with `RenderIntent` to the dispatch match — we expect them to flow through. | Each variant has an explicit arm; synthetic frames flow through the same arms as real ones. |
| `RendererContent::Unsupported` is removed; some forgotten match arm may resurface. | Cargo exhaustiveness check catches any. |

## Acceptance (mirrors design §8)

- All cargo / trunk commands green.
- No `RendererContent::Unsupported` left in the tree.
- No `DirectoryListing` or `TerminalApp` references in `reader.rs`.
- No `_` catch-all in the router's dispatch match.
- Manual QA passes.
- Code-reviewer: no outstanding CRITICAL or HIGH.
