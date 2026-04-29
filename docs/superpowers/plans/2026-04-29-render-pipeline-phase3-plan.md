# Phase 3 — `layout` Field Removal — Implementation Plan

**Design:** [`../specs/2026-04-29-render-pipeline-phase3-design.md`](../specs/2026-04-29-render-pipeline-phase3-design.md)
**Status:** Approved

## Steps

### 3.1 — Trim `RenderIntent`

`src/core/engine/intent.rs`:
- Remove `layout: Option<String>` from every variant where it appears.
- Remove the `let layout = ...` block at the top of `build_render_intent`. Rename the `fs` parameter to `_fs` to keep the trait signature.
- Remove the `layout` argument from `content_intent_for_node`.
- Update every test assertion to drop `layout: None`.

### 3.2 — Trim `ReaderIntent` and conversions

`src/components/reader.rs`:
- Remove `layout` from `Html` / `Markdown` / `Plain` `ReaderIntent` variants.
- `From<ReaderIntent> for RenderIntent`: drop `layout` from each arm.
- `TryFrom<RouteFrame> for ReaderFrame`: drop `ref layout` from each arm.
- Update `reader_intent_tests`:
  - `reader_intent_round_trip_html` — drop `layout: Some(...)`; the assertion becomes "is Html variant for path X".
  - `reader_frame_round_trips_html_with_layout` — rename to `reader_frame_round_trips_html`, drop the layout assertion.
  - All other round-trip tests that destructure `layout` lose that field.
  - `reader_intent_to_render_intent_preserves_fields` — drop the layout half (the asset half stays since `media_type` is unaffected).

### 3.3 — Trim router synthetic frames

`src/components/router.rs`:
- `new_compose_frame`, `ledger_filter_frame`, `builtin_home_frame` — drop `layout: None`.
- Dispatch match arms destructuring `{ node_path, layout }` — drop `layout`.

### 3.4 — Verify

```sh
cargo check --target wasm32-unknown-unknown --lib
cargo test --lib
trunk build
```

`grep -nF '.layout' src/components src/core/engine` should return zero matches outside `models/site.rs` (which Phase 3 leaves alone).

### 3.5 — Code review

Invoke `superpowers:code-reviewer`.

### 3.6 — Master update

Phase 3 status → Complete; Phase 4 → Next; document index + decision log + state pointer.

## Risks

Mechanical change. Cargo will surface any miss. No runtime behaviour change (the field was unread).
