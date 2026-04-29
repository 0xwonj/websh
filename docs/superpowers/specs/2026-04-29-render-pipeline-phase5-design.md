# Phase 5 — Router Cleanup

**Master:** [`2026-04-29-render-pipeline-master.md`](./2026-04-29-render-pipeline-master.md)
**Date:** 2026-04-29
**Status:** Draft

## 1. Problem

After Phase 4, two pieces of router noise remain:

### 1.1 Repeated `Memo::new(move |_| route.get().expect(...))` boilerplate

Every dispatch arm for engine-routed intents constructs the prop Memo the same way:

```rust
view! { <Shell route=Memo::new(move |_| route.get().expect("frame available")) /> }
```

Three call sites (`Shell` for terminal, `Shell` for explorer, `LedgerPage` for content). Each is a `Memo<RouteFrame>` derived from the outer `Memo<Option<RouteFrame>>` with a runtime `expect`. The pattern works (the outer match arm only runs when `route` is `Some`), but the boilerplate distracts from the actual dispatch logic and the `.expect` repeats a non-obvious invariant three times.

### 1.2 Focus side-effect lives in the router

`router.rs:90-105` has an `Effect` that watches for transitions away from Reader-bound surfaces and refocuses the terminal input. The router's primary job is dispatch — focus management is a cross-cutting concern. The effect is 17 lines and uses `prev_was_reader: Option<bool>` plumbing to detect the transition.

Phase 5 is the smallest cleanup that addresses both without inventing new infrastructure.

## 2. Scope and Decisions

### 2.1 Extract a `static_route_memo` helper

```rust
/// Wraps a concrete `RouteFrame` in a `Memo` so it can be passed to a
/// component that expects a reactive prop, without the
/// `Option`-unwrap-and-`expect` dance every call site otherwise repeats.
fn static_route_memo(frame: RouteFrame) -> Memo<RouteFrame> {
    Memo::new(move |_| frame.clone())
}
```

The dispatch arms become:

```rust
RenderIntent::TerminalApp { .. } => view! {
    <Shell route=static_route_memo(frame.clone()) />
}.into_any(),
```

The `route.get().expect("frame available")` pattern disappears from every arm. The function captures the frame at outer-render time (consistent with the old behaviour — when `route` changes, the outer `move ||` re-runs and a fresh memo is created).

### 2.2 Move the focus side-effect into a private helper

The `Effect::new` block stays in `RouterView` (Leptos `Effect`s must be created inside a reactive root, and the only available root here is the router's), but the body moves into a free function:

```rust
fn install_terminal_focus_effect(
    raw_request: RwSignal<RouteRequest>,
    route: Memo<Option<RouteFrame>>,
) {
    Effect::new(move |prev_was_reader: Option<bool>| {
        if matches!(BuiltinRoute::detect(&raw_request.get()), Some(BuiltinRoute::Home)) {
            return false;
        }
        let is_reader = route.get().is_some_and(|frame| {
            !matches!(
                frame.intent,
                RenderIntent::TerminalApp { .. } | RenderIntent::DirectoryListing { .. }
            )
        });
        if prev_was_reader == Some(true) && !is_reader {
            focus_terminal_input();
        }
        is_reader
    });
}
```

`RouterView` calls `install_terminal_focus_effect(_raw_request, route);` once during construction. The body and behaviour are identical; the router body is shorter and the focus concern is now named and isolated.

### 2.3 What Phase 5 does NOT do

- Does **not** move the focus effect into `Shell` (Shell would need a global "previous surface" signal to replicate the transition logic; that's a bigger refactor).
- Does **not** introduce a new component or module.
- Does **not** touch `BuiltinRoute` (Phase 4 work).
- Does **not** rename anything beyond the helper additions.

The goal is "remove residual noise after the structural work in Phases 1-4 lands." Bigger router decomposition is out of scope.

## 3. File Inventory

| File | Change |
|---|---|
| `src/components/router.rs` | Add `static_route_memo` and `install_terminal_focus_effect` helpers. Replace three dispatch-arm `Memo::new(...)` constructions with `static_route_memo(frame.clone())`. Replace inline `Effect::new` block with `install_terminal_focus_effect(_raw_request, route)`. |

No tests added — the helpers are mechanical extractions; existing 524 tests cover the dispatch behaviour.

## 4. Risks

| Risk | Mitigation |
|---|---|
| `static_route_memo` clones the frame per memo sample. | The clone is `Clone` on `RouteFrame` (already required by the existing `Memo::new(move |_| route.get().expect(...))` pattern, which also clones via `.get()`). No regression. |
| `install_terminal_focus_effect` captures `_raw_request` and `route` by value — they are `RwSignal` / `Memo`, both `Copy`. | Verified — `RwSignal<T>` and `Memo<T>` are `Copy` in Leptos 0.7+. |
| Focus behaviour drift. | Body is moved verbatim into the helper; same conditions, same `prev_was_reader` plumbing. |

## 5. Acceptance

- `router.rs` no longer contains `route.get().expect("frame available")`.
- `router.rs` no longer contains an inline `Effect::new(...)` for the focus logic — that logic lives in `install_terminal_focus_effect`.
- 524 tests still pass (no new tests; no test removal).
- `cargo check --target wasm32-unknown-unknown --lib`, `cargo test --lib`, `trunk build` all green.
- Code-reviewer cleared with no outstanding CRITICAL or HIGH.
- Master Phase 5 row → `Complete`.
- Master §6 acceptance: every refactor-wide criterion satisfied.

## 6. Final-commit handoff

Phase 5 is the final phase. Once review clears, all working-tree changes (Phases 1-5 plus the earlier ledger connector refactor and all spec / plan documents) land in a single commit per master §3.1.
