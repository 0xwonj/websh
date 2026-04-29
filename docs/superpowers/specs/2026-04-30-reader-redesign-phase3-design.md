# Phase 3 — Footnote toolbar + keybindings

**Date:** 2026-04-30
**Master:** `docs/superpowers/specs/2026-04-30-reader-redesign-master.md`
**Status:** Approved (autonomous run)

## 1. Scope

Replace the inline `ReaderToolbar` in `reader/mod.rs` with a footnote-mark style toolbar in a new `reader/toolbar.rs`. Add three keybindings (`r`, `e`, `⌘S` / `Ctrl+S`) with text-area-aware suppression for the letter keys. No new modes (View ↔ Edit only — split is permanently out per master §2).

## 2. Out of scope

- Other footnote variants (bracket / colon / prose / minimal) and other prototype toolbar styles (tabs / pill / kbd-only). Master §3 picked one — footnote-mark — and we stay there.
- Append banner. Master §2 defers it.
- Toolbar visibility rule changes. The current `edit_visible` predicate (author mode + mempool path, or `/new`) stays as is.
- Save-flow logic. `on_save` / `on_cancel` / `on_toggle_edit` / `on_preview` callbacks land in `toolbar.rs` unchanged from `mod.rs`.

## 3. File layout (after this phase)

```
src/components/reader/
  mod.rs              — state · dispatch · save (toolbar markup gone)
  intent.rs
  meta.rs
  title_block.rs
  toolbar.rs     ★    — footnote-mark ReaderToolbar + keybindings hookup
  views/…
  reader.module.css   — adds .modefn / .modefnRow / .modefnMark / .modefnLab /
                         .modefnOpt / .modefnSep / .modefnSpacer / .modefnState /
                         .modefnKbd / state-modifier classes
```

`mod.rs` shrinks by ~55 lines (the `#[component] fn ReaderToolbar` block moves out and the new component is one line of markup at the call site). Target after Phase 3: ≤ 420 lines (down from 470).

## 4. Toolbar layout

```
* mode  rendered [r] · edit [e]                  ● synced            (View, edit_visible)
* mode  rendered [r] · edit [e] · cancel · save [⌘S]    ● unsaved    (Edit, dirty)
* mode  rendered [r] · edit [e] · cancel · save [⌘S]    ● synced     (Edit, clean)
* mode  rendered [r] · edit [e] · cancel · save [⌘S]    ● saving…    (Edit, saving)
```

- The asterisk + `mode` label are fixed prefix per the footnote-mark variant.
- `rendered` and `edit` are click targets that switch mode; the active mode gets the `on` class and is non-interactive (clicking the on item is a no-op).
- `cancel` / `save` only render in Edit mode.
- `[r]` / `[e]` / `[⌘S]` are kbd hints (`<span class="modefnKbd">…</span>`).
- The state chip on the right shows the dirty / saving / clean state with the matching color (terminal-yellow for dirty/saving, terminal-green for synced).
- The whole bar is hidden when `edit_visible` is false.

## 5. Keybindings

Implemented via a window-level `keydown` listener installed in an `Effect::new` inside `Reader` (not `ReaderToolbar`, since the listener has to outlive the toolbar's `<Show>` gate).

| Key                   | Behavior                                                                  | Suppressed in textarea? |
|-----------------------|---------------------------------------------------------------------------|-------------------------|
| `r`                   | When `mode == Edit` and not `saving`, run `on_preview` (back to View).    | Yes — letters conflict with typing. |
| `e`                   | When `mode == View` and `edit_visible`, run `on_toggle_edit`.             | Yes.                    |
| `⌘S` / `Ctrl+S`       | When `mode == Edit` and not `saving`, run `on_save`. `preventDefault()`.  | No — `⌘S` is the natural save gesture even from inside the textarea. |

"Suppressed in textarea" is detected by `event.target.tagName == "TEXTAREA"`. Same pattern the prototype uses.

## 6. Component API

```rust
// src/components/reader/toolbar.rs
#[component]
pub fn ReaderToolbar(
    mode: RwSignal<ReaderMode>,
    is_new: Memo<bool>,
    can_edit: Memo<bool>,
    saving: ReadSignal<bool>,
    dirty: ReadSignal<bool>,    // NEW prop — was previously not surfaced
    on_edit: Callback<()>,
    on_preview: Callback<()>,
    on_save: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView;
```

`mode`, `is_new`, `can_edit`, `saving`, and the four callbacks remain unchanged from Phase 2's signature. The new `dirty` prop is `draft_dirty.read_only()` from `mod.rs`.

`is_new` is no longer rendered as a label inside the toolbar (the prototype's footnote-mark has no label slot). The `/new` context is conveyed by URL alone. If users miss the "new draft" cue, we can revisit in a follow-up.

## 7. CSS additions to `reader.module.css`

```
.modefn { margin-top: 6px; padding-top: 0; font-size: 11px; color: var(--text-dim); }
.modefnRow { display: flex; align-items: baseline; gap: 6px; line-height: 1.6; flex-wrap: wrap; }
.modefnMark { color: var(--accent); margin-right: 2px; }
.modefnLab { color: var(--text-muted); margin-right: 4px; }
.modefnOpt { color: var(--text-dim); cursor: pointer; user-select: none; padding: 0 2px; }
.modefnOpt:hover { color: var(--text-primary); }
.modefnOptOn { color: var(--accent); border-bottom: 1px solid var(--accent); cursor: default; }
.modefnOptDisabled { opacity: 0.45; cursor: not-allowed; }
.modefnKbd { margin-left: 5px; color: var(--text-muted); font-size: 9.5px; border: 1px solid var(--border-subtle); padding: 0 3px; border-radius: 1px; }
.modefnSep { color: var(--text-muted); margin: 0 1px; }
.modefnSpacer { flex: 1; }
.modefnState { font-size: 10.5px; color: var(--terminal-green); }
.modefnState::before { content: "● "; }
.modefnStateDirty { color: var(--terminal-yellow); }
```

The old `.toolbar` / `.toolbarLabel` / `.toolbarActions` / `.actionButton` / `.actionButtonPrimary` rules are removed — Phase 2 left them in for the inline toolbar; Phase 3 deletes them. (Verify: no external consumers.)

## 8. Tests

Unit tests cover the toolbar's pure label logic, not Leptos rendering. Two pieces of pure logic to test:

1. `state_label(saving, dirty) -> &'static str` — returns `"saving…"` / `"unsaved"` / `"synced"`.
2. `state_class(saving, dirty) -> &'static str` — returns the modefnStateDirty class on dirty/saving, otherwise `""`.

These get a `#[cfg(test)] mod tests` block in `toolbar.rs` with three cases each. No new resource integration.

Manual QA items (in `trunk serve`):

- Open a markdown route in author mode (token present). Toolbar shows `* mode  rendered [r] · edit [e]    ● synced`. Press `e` outside the textarea → switches to Edit. Type a character → state flips to `● unsaved`. Press `r` → switches back to View, draft preserved. Press `⌘S` while still typing → save fires.
- Without author mode: toolbar is hidden on every route.
- `/new` route: toolbar appears in Edit mode immediately. Save fires on `⌘S`.
- Saving indicator: throttle network in DevTools, click save → state shows `● saving…` while in flight.
- Switch palette: toolbar colors retone correctly (no per-theme overrides).

## 9. Acceptance

- `cargo test --lib` green.
- `cargo check --target wasm32-unknown-unknown --lib` green.
- `trunk build` green.
- No new stylance dead-code warnings (the consolidated `pub(crate) css` from Phase 2 means the new classes auto-propagate).
- Manual QA list passes.
- `code-reviewer` clears with no CRITICAL/HIGH.

## 10. Self-review

- Placeholders / TODOs: none.
- Contradictions vs master: none. §6 Phase 3 matches.
- Scope creep risk: tempted to add an "append banner" while wiring save state — explicitly deferred to a future phase or never. Tempted to add a status bar at the bottom of `mod.rs` — out of scope.
- Risks:
  - Window-level `keydown` listener: must not double-fire. Use `Effect::new(move |prev| { if prev.is_none() { … install … } })` pattern, or attach via `WindowEventHandler` if Leptos provides one. Verify against Leptos 0.8 idioms — alternative is `wasm_bindgen::closure::Closure::wrap` + `add_event_listener_with_callback`. Check what `terminal/` does for global keys.
  - Suppression of `r` / `e` in textareas: relies on `event.target` being the textarea. Confirmed pattern in prototype `useEffect` handler. The Leptos handler reads `ev.target()?.dyn_ref::<HtmlTextAreaElement>()` to detect.
  - `⌘S` should fire from inside the textarea — `preventDefault()` on it stops the browser's "save page" dialog.
- What if a user hits `e` while reading non-mempool markdown (no author mode)? `edit_visible` is false; the keybinding gate checks `can_edit.get_untracked()` first and returns early. No accidental Edit mode switch.
