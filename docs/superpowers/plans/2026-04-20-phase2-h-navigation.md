# Phase 2 Track H: Navigation — Summary

**Issues:** M1 (forward_stack desync with browser history), M11 (pop_forward idiom).

## Design decision (recorded in master decision log)

**Delete `forward_stack` entirely; delegate to browser history.**

Rationale: the in-app stack couldn't track navigations initiated by the browser's own back/forward buttons, causing silent desync. Since hash-based routing already goes through browser history, the browser IS the authoritative source — tracking separately was strictly worse than delegating.

## Changes

- `ExplorerState`: removed `forward_stack` field and 4 methods (`push_forward`, `pop_forward`, `clear_forward`, `can_go_forward`).
- `components/explorer/header.rs`: back/forward handlers call `window.history().back()` / `.forward()`. Dropped `can_forward` signal and disabled-class logic on the forward button. Back button kept `is_root`-disabled to avoid exiting the app.
- `components/router.rs`: ReaderOverlay close handler simplified.
- `components/explorer/file_list.rs`: dropped `clear_forward()` call.

## Verification

- `cargo test --bin websh`: 189 / 4 pre-existing fail (no regression).
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- No references to `forward_stack` / `*_forward` in `src/`.

## Minor polish applied

- Dropped now-unused `route_ctx` param from `NavButtons` signature.
