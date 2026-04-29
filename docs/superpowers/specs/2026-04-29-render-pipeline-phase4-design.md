# Phase 4 — Synthetic `RouteFrame` Policy

**Master:** [`2026-04-29-render-pipeline-master.md`](./2026-04-29-render-pipeline-master.md)
**Date:** 2026-04-29
**Status:** Draft

## 1. Problem

`router.rs` constructs `RouteFrame` values manually for three URL patterns the engine does not resolve:

- `new_compose_frame()` — `/new` (compose flow)
- `ledger_filter_frame(req)` — `/ledger` and `/<category>`
- `builtin_home_frame(req)` — `/`

These helpers fabricate `RouteResolution` and `RenderIntent` ad hoc. The router's body decides which one to use through three independent `is_*_route` predicates (`router.rs:90-130`). The structure works — but:

1. There is no single concept "this URL bypasses the engine." The bypass is implicit in the order of the three early-return guards.
2. The reserved URL list (`/`, `/ledger`, `/<category>`, `/new`) lives only as English in CLAUDE.md and the mempool master's anchor A9. Adding or removing a reserved URL means edits in three places without a type-system witness.
3. Each helper builds a complete `RouteFrame` even though the downstream consumer (HomePage, LedgerPage, Reader) only reads a small subset.

## 2. Decision: `BuiltinRoute` partition (engine stays UI-agnostic)

Two options were considered:

**A. Engine absorption.** Add reserved patterns to `engine::routing` so `resolve_route` returns proper `RouteResolution`s. Engine becomes the single producer of `RouteFrame`s.

> **Rejected.** The reserved URLs encode UI semantics — `/ledger` filtering, `/new` compose mode, the homepage. CLAUDE.md is explicit that "the UI should render engine output. It should not assemble filesystems or resolve backend details directly." Pulling UI semantics into the engine inverts that boundary.

**B. `BuiltinRoute` partition.** Keep the engine UI-agnostic. Introduce a typed enum that names every URL that bypasses the engine. The router does a two-stage dispatch: `BuiltinRoute::detect(request)` first, engine `route.get()` second. The synthetic-frame helpers stay but live behind the `BuiltinRoute` partition.

> **Adopted.** This makes the bypass explicit, gives the reserved list a type-system witness, and preserves CLAUDE.md's layering.

## 3. Type Design

```rust
// src/components/router/builtin.rs (new module)

pub enum BuiltinRoute {
    /// `/` — homepage.
    Home,
    /// `/ledger` and `/<category>` — ledger filter views.
    LedgerFilter,
    /// `/new` — mempool compose flow.
    NewCompose,
}

impl BuiltinRoute {
    /// Classify a request against the reserved URL list. `None` means the
    /// request goes through the engine.
    pub fn detect(request: &RouteRequest) -> Option<Self> {
        if request.url_path == "/" {
            return Some(Self::Home);
        }
        if is_ledger_filter_route_segment(request.url_path.trim_matches('/')) {
            return Some(Self::LedgerFilter);
        }
        if is_new_request_path(request) {
            return Some(Self::NewCompose);
        }
        None
    }
}
```

The variant payload is empty for `Home` and `NewCompose` (the URL is the whole identity); `LedgerFilter` doesn't carry the path either — the synthetic frame helper still reads `_raw_request.get()` when invoked, just as it does today.

The synthetic-frame helpers move into the same module:

```rust
pub(super) fn home_frame(request: RouteRequest) -> RouteFrame { ... }
pub(super) fn ledger_filter_frame(request: RouteRequest) -> RouteFrame { ... }
pub(super) fn new_compose_frame() -> RouteFrame { ... }
```

They are unchanged from `router.rs` today, just relocated. The `pub(super)` visibility keeps them accessible to `router.rs` and invisible to the rest of the crate.

## 4. Router Dispatch After Phase 4

```rust
view! {
    {move || {
        let request = _raw_request.get();
        match BuiltinRoute::detect(&request) {
            Some(BuiltinRoute::Home) => {
                let request_signal = _raw_request;
                view! {
                    <HomePage route=Memo::new(move |_| {
                        route.get().unwrap_or_else(|| home_frame(request_signal.get()))
                    }) />
                }.into_any()
            }
            Some(BuiltinRoute::LedgerFilter) => {
                let request_signal = _raw_request;
                view! {
                    <LedgerPage route=Memo::new(move |_| {
                        ledger_filter_frame(request_signal.get())
                    }) />
                }.into_any()
            }
            Some(BuiltinRoute::NewCompose) => {
                let reader_frame = ReaderFrame::try_from(new_compose_frame())
                    .expect("compose route always produces a Reader-bound intent");
                view! {
                    <Reader frame=Memo::new(move |_| reader_frame.clone()) />
                }.into_any()
            }
            None => match route.get() {
                Some(frame) => match frame.intent { /* engine variants */ },
                None => view! { <NotFound /> }.into_any(),
            }
        }
    }}
}
```

The three independent `is_*` predicates collapse into a single match. The reserved list becomes the `BuiltinRoute` enum's variants — adding a new builtin URL is one variant + one `detect` arm, both in `builtin.rs`.

## 5. File Inventory

| File | Change |
|---|---|
| `src/components/router/mod.rs` | Renamed from `src/components/router.rs`; same content minus the relocated helpers and reshaped dispatch. |
| `src/components/router/builtin.rs` (new) | Holds `BuiltinRoute`, `detect`, and the three synthetic-frame helpers. |
| `src/components/mod.rs` | Update `pub mod router;` (path unchanged but now references a directory module). |

If a single-file form is preferable (avoid the directory split for a ~50-line addition), keep `router.rs` and add `BuiltinRoute` inline. Decision: **single-file** for Phase 4 — the addition is small enough that splitting into a directory adds more navigation overhead than it removes. The change is reversible if the file grows.

So actually:

| File | Change |
|---|---|
| `src/components/router.rs` | Add `BuiltinRoute` enum + `detect` method. Reshape dispatch to two-stage. Rename helpers (`builtin_home_frame` → `home_frame`) for symmetry under the new partition. |

## 6. Tests

Add `#[cfg(test)] mod builtin_route_tests` in `router.rs`:

```rust
#[test]
fn detects_home() {
    assert!(matches!(
        BuiltinRoute::detect(&RouteRequest::new("/")),
        Some(BuiltinRoute::Home)
    ));
}

#[test]
fn detects_ledger_root() { /* /ledger → LedgerFilter */ }

#[test]
fn detects_ledger_category() { /* /papers → LedgerFilter */ }

#[test]
fn detects_compose() { /* /new → NewCompose */ }

#[test]
fn rejects_engine_routes() {
    for path in ["/blog/hello.md", "/websh", "/explorer/foo", "/papers/x.pdf"] {
        // /papers/x.pdf goes to engine because it's a file, not the bare /papers filter
        // Actually verify: ledger filter detection uses is_ledger_filter_route_segment
        // which strips trailing slashes. /papers (no slash) → LedgerFilter.
        // /papers/x.pdf → LedgerFilter? Need to check is_ledger_filter_route_segment.
    }
}
```

Note: `is_ledger_filter_route_segment` is the existing predicate at `ledger_routes.rs`. Phase 4 reuses it; the test for `rejects_engine_routes` is conditional on understanding what segments it considers a filter. Will inspect during implementation.

## 7. Risks

| Risk | Mitigation |
|---|---|
| `BuiltinRoute::detect` order matters (e.g., `/` matches `is_ledger_filter_route_segment(\"\")`?). | Inspect `is_ledger_filter_route_segment` during implementation; if it accepts the empty segment, the Home check at the top of `detect` short-circuits before the ledger check. Tests cover this. |
| Renaming `builtin_home_frame` → `home_frame` breaks no other call sites. | Verified via grep: only `router.rs` uses these helpers. |
| The `route.get().unwrap_or_else(|| home_frame(...))` pattern remains for `HomePage`. | Phase 5 may rework. Phase 4 preserves current behaviour. |

## 8. Acceptance

- `BuiltinRoute` enum exists with three variants.
- `BuiltinRoute::detect` is the single entry point for builtin classification; the three `is_*` early-return guards in `RouterView` collapse into one match.
- Synthetic helpers (`home_frame`, `ledger_filter_frame`, `new_compose_frame`) are private to `router.rs` (no external callers verified by `grep -rn 'home_frame\|ledger_filter_frame\|new_compose_frame' src/` returning hits only inside `router.rs`).
- 5 new tests in a `builtin_route_tests` module.
- `cargo test --lib`, `cargo check --target wasm32-unknown-unknown --lib`, `trunk build` all green.
- Code-reviewer cleared with no outstanding CRITICAL or HIGH.
