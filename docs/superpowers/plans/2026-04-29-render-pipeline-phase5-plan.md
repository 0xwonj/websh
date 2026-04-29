# Phase 5 — Router Cleanup — Implementation Plan

**Design:** [`../specs/2026-04-29-render-pipeline-phase5-design.md`](../specs/2026-04-29-render-pipeline-phase5-design.md)
**Status:** Approved

## Steps

### 5.1 — Add `static_route_memo` helper

Place near the existing free functions in `router.rs` (after `home_frame`, before `NotFound`).

```rust
fn static_route_memo(frame: RouteFrame) -> Memo<RouteFrame> {
    Memo::new(move |_| frame.clone())
}
```

### 5.2 — Replace dispatch-arm Memo constructions

Three call sites in `RouterView`'s engine match:

- `RenderIntent::TerminalApp { .. } => view! { <Shell route=static_route_memo(frame.clone()) /> }.into_any()`
- `RenderIntent::DirectoryListing { .. } if Explorer surface => same with Shell`
- `RenderIntent::DirectoryListing { .. } => same with LedgerPage`

Each loses `Memo::new(move |_| route.get().expect("frame available"))`.

### 5.3 — Add `install_terminal_focus_effect` helper

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

Place after `static_route_memo`.

### 5.4 — Replace inline focus Effect

In `RouterView`, replace the `Effect::new(move |prev_was_reader: Option<bool>| { ... });` block with `install_terminal_focus_effect(_raw_request, route);`.

### 5.5 — Verify

```sh
cargo check --target wasm32-unknown-unknown --lib
cargo test --lib
trunk build
```

`grep -nF 'route.get().expect' src/components/router.rs` should return zero matches. `grep -nE 'Effect::new\(move' src/components/router.rs` should return zero matches inside `RouterView` (the helper has it).

### 5.6 — Code review

`superpowers:code-reviewer` on the diff plus design + plan.

### 5.7 — Master update

- §2 row Phase 5 status → `Complete`.
- §4 Document Index — add Phase 5 design + plan rows.
- §5 Decision Log — append entry.
- §7 State — `Complete; ready for final commit`.

### 5.8 — Final commit

Stage every working-tree change since the start of the refactor (Phases 1-5 code + all spec and plan docs + master + the earlier ledger connector refactor) and create a single commit. **This step happens only after the user confirms** — Phase 5's design says final commit is the explicit handoff to the user.
