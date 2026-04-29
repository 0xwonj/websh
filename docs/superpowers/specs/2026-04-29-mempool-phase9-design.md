# Phase 9 — Mempool Collapse/Expand

**Status:** Design + Plan v2 (closes reviewer pass 1)
**Date:** 2026-04-29
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)

---

## 1. Goal

Let the user collapse the mempool section on `/ledger`. Click the header bar to toggle; the row list hides while the count stays visible.

## 2. UX

| State | What's visible |
|---|---|
| Expanded (default) | `▾ mempool · X pending` (header) + entries below |
| Collapsed | `▸ mempool · X pending` (header only); entries hidden |

The chevron (`▾` / `▸`) sits to the left of the `mempool` label, treated as the visual toggle indicator. The whole header bar is the click target — the `+ compose` link inside it stops event propagation so clicking compose doesn't fold the section.

Default is expanded so first-time visitors see the same surface they see today.

## 3. State

- `collapsed: RwSignal<bool>` lives in `LedgerPage`, **not** in `Mempool` itself. Reason: `LedgerPage` rebuilds the `Mempool` component on every filter-route change (`ledger_page.rs:88-168`'s `move || ledger.get().map(...)` re-runs when `route` changes), so a signal local to `Mempool` would be dropped and reset on each filter switch. Lifting the signal one level up (to a parent that survives filter changes) preserves the collapse choice across `/#/ledger` ↔ `/#/writing` ↔ `/#/papers` etc.
- `LedgerPage` declares `let mempool_collapsed = RwSignal::new(false);` once near the existing signals (e.g. next to `mempool_files`).
- `Mempool`'s prop surface gains a single new prop: `collapsed: RwSignal<bool>`. The component reads / writes it directly; no separate read+write split needed.
- Default `false` (expanded).
- **Session-only.** Per `CLAUDE.md` ("Do not read `localStorage` or `sessionStorage` from feature code"), persistence across reloads is out of scope; the section also resets on intra-app navigation back to `/#/ledger` because `LedgerPage` itself remounts on hash change. If users ask for "remember my collapse choice across reloads", that's a follow-up phase that wires the preference through `AppContext` or a generic prefs adapter.

## 4. Component changes

### `src/components/ledger_page.rs`

- Declare `let mempool_collapsed = RwSignal::new(false);` alongside the existing `mempool_files` and `author_mode` declarations.
- Pass it down: `<Mempool model=... author_mode=author_mode collapsed=mempool_collapsed />`.

### `src/components/mempool/component.rs`

- `Mempool`'s signature gains `collapsed: RwSignal<bool>`.
- Header click handler: `move |_| collapsed.update(|v| *v = !*v)`. Mounted on `<div class=css::mpHead>`.
- Keyboard handler on the header: Enter / Space toggles (with `event.prevent_default()` on Space to suppress page-scroll). The header gets `role="button" tabindex="0"`.
- ARIA: `aria-expanded=move || (!collapsed.get()).to_string()` on the header. The entries list gets `id="mempool-rows"`, and the header gets `aria-controls="mempool-rows"` so screen readers know which region the disclosure controls. (Mirrors the `signature_footer.rs:80` and `site_chrome.rs:298` patterns.)
- Add a `<span class=css::mpToggle aria-hidden="true">` inside the header (left of `mpLabel`) that renders `▾` when expanded, `▸` when collapsed.
- Compose link gets `on:click=|ev| ev.stop_propagation()` (load-bearing, not optional — a hash-`<a>` click bubbles even when navigation happens; Enter on a focused anchor produces a synthetic click that also bubbles). Without this, clicking compose folds the section.
- Wrap the entries container `<div class=css::mpList id="mempool-rows">` in `<Show when=move || !collapsed.get()>...</Show>`.
- The empty-state branch (`mpEmpty`) is also gated by the same `<Show>` — when collapsed, neither rows nor the empty-state placeholder render.

### `src/components/mempool/mempool.module.css`

- `.mpHead`: add `cursor: pointer;`. On hover, shift `.mpLabel`'s color to `var(--ledger-ink)` (subtle confirmation that the header is interactive); the header's other text stays the same. Add `:focus-visible { outline: 1px solid var(--ledger-accent); outline-offset: -1px; }` on `.mpHead` for keyboard focus.
- `.mpToggle`: small monospace glyph, color `var(--ledger-faint)`, `font-size: 0.85em`. `display: inline-block; width: 1em; text-align: center;` so the layout doesn't jitter between `▾` (wider) and `▸` (narrower).
- The compose anchor's existing `cursor: pointer` and hover styling stay as-is; the surrounding `.mpHead`'s `cursor: pointer` is harmless on it.
- No animation in V1 — instant flip matches the ledger.html aesthetic. (Animations are a follow-up if asked.)

## 5. Tests

This is UI behavior on a Leptos component; no unit tests added (consistent with prior mempool phases). The verification is live:

1. `/ledger` as any user → mempool section visible, expanded by default. Header shows `▾ mempool · X pending`.
2. Click header → entries hide; chevron flips to `▸`. Count stays visible.
3. Click header again → entries reappear; chevron flips back to `▾`.
4. As author, click `+ compose` → URL changes to `/#/new`; mempool section state at the previous page is irrelevant. (No accidental fold.)
5. Tab onto the header, press Enter → toggles. Press Space → toggles. Focus outline visible.
6. Filter mempool by category (e.g., `/#/writing`) → header shows the filtered count; collapse state persists across filter changes because the signal lives in `LedgerPage` (which the filter route does not re-mount, only re-renders). Verify in the live walk-through that toggling collapsed on `/#/ledger`, navigating to `/#/writing`, and back to `/#/ledger` keeps the collapsed state.
7. Hash-navigate away (e.g. `/#/new`) and back to `/#/ledger` — collapse state DOES reset, because `LedgerPage` itself re-mounts on hash-change. This is intentional V1 behavior; persistence across navigation is out of scope.

## 6. Implementation steps

Single commit:

1. Edit `src/components/mempool/component.rs` per §4.
2. Edit `src/components/mempool/mempool.module.css` per §4.
3. Add Phase 9 row to master plan §4 phase table; index in §6; decision-log entry in §10.

Verify: `cargo check --target wasm32-unknown-unknown --lib`; `cargo test --lib` 503 passing (unchanged).

**Commit:** `feat(mempool): collapse/expand toggle on the mempool header`

## 7. Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| Click on `+ compose` accidentally toggles the section | Medium | `stop_propagation` on the compose anchor's click handler. Verified in the live walk-through. |
| Filter change re-mounts `Mempool` and loses collapse state | **Resolved by design** | Reviewer pass 1 confirmed `LedgerPage`'s `move || ledger.get().map(...)` re-runs the inner closure on filter changes, so a `Mempool`-local signal would reset. Phase 9 lifts the signal to `LedgerPage` (which only re-mounts on full hash-change, not filter change) and passes it down. |
| Keyboard focus indicator on the header looks wrong against the dashed border | Low | Uses `outline` with `outline-offset: -1px` so it nests inside the existing border. Matches `.mpItem:focus-visible` pattern from Phase 6 D. |
| Empty mempool state — collapsing hides the "no pending entries" placeholder, leaving a bare header | Low | Acceptable: collapsing means "I don't want to see the section." The placeholder reappears when expanded. |

## 8. Out of scope

- Persistence across reloads / sessions (would need an `AppContext`-mediated preference store).
- Animation of the collapse / expand.
- Per-category collapse memory.
- Indicator that the section has unread / new entries when collapsed.

## 9. Decision-log entry (to append at completion)

| Date | Decision | Reference |
|---|---|---|
| 2026-04-29 | Phase 9 — collapse/expand toggle on the mempool section header. Click anywhere on `.mpHead` (except the `+ compose` link, which stops propagation) toggles a session-only `collapsed: RwSignal<bool>` local to the `Mempool` component. Chevron `▾`/`▸` indicator, keyboard support (Enter/Space) on the `role="button" tabindex="0"` header. Default expanded. Persistence deferred — `CLAUDE.md`'s "no localStorage in feature code" rule pushes prefs through `AppContext`, scope creep for V1. | §3 |
