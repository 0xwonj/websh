# Phase 3 — `layout` Field Audit and Removal

**Master:** [`2026-04-29-render-pipeline-master.md`](./2026-04-29-render-pipeline-master.md)
**Date:** 2026-04-29
**Status:** Draft

## 1. Audit Result

`grep -rn '\.layout\b' src/` (excluding sidecar persistence in `models/site.rs` and irrelevant matches) returns exactly **one** site:

```
src/core/engine/intent.rs:42:        .and_then(|meta| meta.layout.clone());
```

That is the **producer**. There are no consumers. The chain is:

1. `intent.rs:40-42` reads `LoadedNodeMetadata.layout` and stores it as a field on five `RenderIntent` variants.
2. Phase 2 propagated the same field onto three `ReaderIntent` variants (`Html`, `Markdown`, `Plain`).
3. Reader, Router, Shell, LedgerPage, HomePage — **none** of them ever destructure `.layout` or read it.

The field is dead. It propagates through every layer carrying nothing.

## 2. Goal

Drop `layout` from `RenderIntent`, `ReaderIntent`, and the engine's intent-build flow. Per CLAUDE.md ("Don't add features, refactor, or introduce abstractions beyond what the task requires"), the unused field is YAGNI debt that obscures real fields.

After Phase 3:
- Every `RenderIntent` and `ReaderIntent` variant carries only fields a consumer actually reads.
- `intent.rs::build_render_intent` no longer queries `node_metadata` (the `layout` lookup was its only reason to).
- Construction sites (router synthetic frames, conversions, tests) drop the `layout: None` argument.

## 3. Out of Scope

The sidecar persistence types (`FileSidecarMetadata.layout`, `DirectorySidecarMetadata.layout`, `LoadedNodeMetadata.layout`) **stay**. Three reasons:

1. They live in `models/site.rs` and are part of the on-disk JSON contract. Existing sidecar files in `0xwonj/websh-mempool` or the bundle source may carry `layout` keys; removing the field would either fail to deserialize or silently drop the value.
2. Future re-introduction of layout-aware rendering would re-add the field to intents but not re-add it to persistence — the persistence side's tolerance for an unread field is the safer asymmetry.
3. Removing persistence fields is a `models/` concern that would need its own migration design.

The dead field at the **runtime** layer (intent flow) is the genuine YAGNI violation. The persistence keeps the door open.

## 4. Type Changes

### 4.1 `RenderIntent` (engine)

```rust
// before
DirectoryListing { node_path, layout: Option<String> }
TerminalApp      { node_path, layout: Option<String> }
HtmlContent      { node_path, layout: Option<String> }
MarkdownContent  { node_path, layout: Option<String> }
PlainContent     { node_path, layout: Option<String> }
Asset            { node_path, media_type: String }
Redirect         { node_path }

// after
DirectoryListing { node_path }
TerminalApp      { node_path }
HtmlContent      { node_path }
MarkdownContent  { node_path }
PlainContent     { node_path }
Asset            { node_path, media_type: String }
Redirect         { node_path }
```

### 4.2 `ReaderIntent` (UI)

```rust
// before
Html     { node_path, layout: Option<String> }
Markdown { node_path, layout: Option<String> }
Plain    { node_path, layout: Option<String> }
Asset    { node_path, media_type: String }
Redirect { node_path }

// after
Html     { node_path }
Markdown { node_path }
Plain    { node_path }
Asset    { node_path, media_type: String }
Redirect { node_path }
```

### 4.3 `build_render_intent`

```rust
// before
pub fn build_render_intent(fs: &GlobalFs, resolution: &RouteResolution) -> Option<RenderIntent> {
    let layout = fs.node_metadata(&resolution.node_path).and_then(|meta| meta.layout.clone());
    let path = &resolution.node_path;
    Some(match resolution.kind { ... })
}

// after
pub fn build_render_intent(_fs: &GlobalFs, resolution: &RouteResolution) -> Option<RenderIntent> {
    let path = &resolution.node_path;
    Some(match resolution.kind { ... })
}
```

`fs` becomes unused; rename to `_fs` to keep the trait signature compatible (`global_fs.rs:26` expects `&GlobalFs` for `build_render_intent` on the `FsEngine` trait).

`content_intent_for_node`'s second parameter (`layout: Option<String>`) goes away.

## 5. Consumer Updates

### 5.1 `src/core/engine/intent.rs`

- Remove `layout` from every variant.
- Drop the `let layout = ...` block.
- Drop the `layout` argument from `content_intent_for_node`.
- Update every test assertion: drop the `layout: None` lines from `assert_eq!(intent, RenderIntent::HtmlContent { ... })`-style equality checks.

### 5.2 `src/components/reader.rs`

- Remove `layout` from each `ReaderIntent` variant.
- `From<ReaderIntent> for RenderIntent`: drop the `layout` propagation.
- `TryFrom<RouteFrame> for ReaderIntent` (effectively `for ReaderFrame`): drop the `ref layout` capture and `layout: layout.clone()` field.
- Tests: drop `layout: None` / `layout: Some(...)` from constructions; the `reader_intent_round_trip_html_with_layout` test needs renaming since there is no longer a `layout` field — the test loses its purpose. Replace with a simpler `reader_frame_round_trips_html` and lift the `Some("default")` behaviour off.

### 5.3 `src/components/router.rs`

- `new_compose_frame()`, `ledger_filter_frame()`, `builtin_home_frame()` all construct `RenderIntent::*Content { layout: None }` or `RenderIntent::DirectoryListing { layout: None }`. Drop the `layout` field from each.
- The dispatch arms in the main match destructure `RenderIntent::HtmlContent { node_path, layout }` etc. for the `From` flow. After Phase 3 they destructure only `node_path`.

### 5.4 No-op consumers

`shell.rs:118` matches `RenderIntent::DirectoryListing { .. }` (using `..`) — not affected.

The `is_reader` Effect at `router.rs:90-105` matches `RenderIntent::TerminalApp { .. } | RenderIntent::DirectoryListing { .. }` — not affected.

## 6. File Inventory

| File | Change |
|---|---|
| `src/core/engine/intent.rs` | Drop `layout` from variants, build flow, helper, and tests. |
| `src/components/reader.rs` | Drop `layout` from `ReaderIntent`, conversions, and round-trip tests. |
| `src/components/router.rs` | Drop `layout: None` from synthetic frames and dispatch destructuring. |

## 7. Risks

| Risk | Mitigation |
|---|---|
| A future feature wants per-route layout. | Re-add to `RenderIntent` then; persistence already preserves it. The structural change is reversible. |
| External tooling reads `RenderIntent` from a serialized form. | None — `RenderIntent` is not `Serialize`/`Deserialize`. Verified by grep. |
| Tests assert `layout: None` literally. | Mechanical removal; cargo test will surface any miss. |

## 8. Acceptance

- `grep -nF '\.layout\b' src/` returns matches only inside `models/site.rs` (the persistence types).
- `cargo test --lib` green.
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `trunk build` green.
- Code-reviewer cleared with no outstanding CRITICAL or HIGH.
