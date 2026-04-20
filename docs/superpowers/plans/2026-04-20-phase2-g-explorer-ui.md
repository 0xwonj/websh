# Phase 2 Track G: Explorer UI cleanup + a11y — Summary

**Issues:** H10 (debug console.log cleanup), H11 (FileListItem keyboard nav), M10 (dropdown focus-out dedup).

## Changes

- **H10**: Removed 11 `web_sys::console::log_1` debug calls from stub handlers in `components/explorer/header.rs` (6) and `components/reader/mod.rs` (5). Stub bodies now empty.
- **H11**: `FileListItem` handlers refactored to share `do_select` / `do_open` closures; `on:keydown` added — Enter opens, Space selects, both `prevent_default`.
- **M10**: Extracted `close_on_focus_out(WriteSignal<bool>) -> impl Fn(FocusEvent)` helper; `NewMenu` and `MoreMenu` share it.

## Verification

- `cargo test --bin websh`: 189 / 4 pre-existing (no regression, no new tests).
- `cargo build --release --target wasm32-unknown-unknown`: clean.
