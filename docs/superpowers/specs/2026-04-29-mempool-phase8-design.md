# Phase 8 — Reader Component Refinement

**Status:** Design v2 (closes reviewer pass 1)
**Date:** 2026-04-29
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Refines:** [`2026-04-29-mempool-phase7-design.md`](./2026-04-29-mempool-phase7-design.md)

---

## 1. Problem

Phase 7 wired view/edit affordances into `SiteChrome.extra_actions`. That slot is *site-scoped* (nav, palette picker, theme), so a document-level action like "edit this entry" stuffed into it reads as a foreign element in the global top bar. Users see "edit" appear next to the global navigation, which feels strange.

Two structural mistakes:
1. **Wrong placement.** Document-level actions belong with the document, not the site chrome.
2. **No real Reader component.** "Reader" is treated as a feature bolted onto `RendererPage`, not a designed unit. The component file has grown to ~340 lines mixing routing-frame interpretation, content fetching, draft state, save logic, toolbar rendering, and viewport surface — all in one place.

## 2. Goal

Extract a proper `Reader` component that owns view/edit/save/cancel as part of the article frame. The toolbar lives at the top of the document, styled to match the ledger.html aesthetic (monospace, dashed rules, accent on hover). `SiteChrome.extra_actions` receives no document-scoped content — its slot stays available for future site-level affordances.

## 3. UX

A small toolbar sits at the top of the article, above the rendered content (or above the textarea in Edit). The toolbar only renders when there are actions or a label to show — pure-view of canonical (non-mempool) content stays clean.

Visual language: matches the existing mempool list header (`.mpHead`, `.mpCompose`) — dashed border-bottom, monospace, uppercase letter-spacing, accent buttons.

| State | Toolbar contents |
|---|---|
| Pure view (non-mempool, OR non-author on mempool) | Toolbar not rendered |
| View on mempool path, author mode | Right-aligned `[ edit ]` button |
| Edit on existing mempool entry | Left: `editing` label · right: `[ cancel ] [ save ]` |
| Edit on `/new` | Left: `new draft` label · right: `[ cancel ] [ save ]` |
| View on `/new` (preview of unsaved draft) | Left: `new draft · preview` label · right: `[ edit ]` button |

Save errors render in a banner directly below the toolbar (still document-scoped). The `<textarea>` and the markdown rendering both live below the banner.

## 4. Component plan

### Extract `Reader` to its own module

- Rename `src/components/renderer_page.rs` → `src/components/reader.rs`. Component renames `RendererPage` → `Reader`.
- The previous `src/components/reader/` directory was deleted in Phase 6 E; the path is free. We use a flat `src/components/reader.rs` (single file) since the component is self-contained — no need for a sub-directory.
- `src/components/reader.module.css` (renamed from `renderer_page.module.css`) keeps the existing markdown / asset / pdf styles plus the new toolbar / textarea / error-banner rules. Phase 7's `editButton`/`saveButton`/`cancelButton`/`editorTextarea`/`errorBanner` rules survive — they just move to the toolbar context.

### Toolbar sub-component

`ReaderToolbar` lives inside `reader.rs` (private; same file is fine — it's tightly coupled to `Reader`'s state). Props:

```rust
struct ReaderToolbarProps {
    mode: RwSignal<ReaderMode>,
    is_new: Memo<bool>,
    can_edit: Memo<bool>,        // author && mempool path (also true on /new)
    saving: ReadSignal<bool>,
    on_edit: Callback<()>,
    on_save: Callback<()>,
    on_cancel: Callback<()>,
}
```

Render: `<header class=css::toolbar>` only when one of:
- `mode == View && can_edit` (Edit button visible)
- `mode == Edit` (Save+Cancel visible)

Otherwise the toolbar `<header>` returns nothing.

Inside the header:
- Left: `<span class=css::label>` carrying the state label (`new draft`, `new draft · preview`, `editing`, or empty)
- Right: `<div class=css::actions>` with the buttons

### `SiteChrome.extra_actions` removal

`Reader` does not pass `extra_actions` to `SiteChrome`. **The `extra_actions` prop on `SiteChrome` is also removed entirely** (no callers post-Phase-8; YAGNI). If a real site-level affordance arrives later, the prop can be re-introduced together with its first caller. Phase 8 deletes:

- `extra_actions: Option<ChildrenFn>` parameter on `SiteChrome` (`src/components/chrome/mod.rs:65-72`)
- The `{extra_actions.map(|c| c())}` mount inside `SiteChromeActions` (`src/components/chrome/mod.rs:132`)

### Files modified

| Path | Change |
|---|---|
| `src/components/renderer_page.rs` | Renamed to `src/components/reader.rs`; component renamed `RendererPage` → `Reader`. Toolbar moves out of `extra_actions` into a `<ReaderToolbar>` sub-component. |
| `src/components/renderer_page.module.css` | Renamed to `src/components/reader.module.css`. Adds `.toolbar`, `.toolbarLabel`, `.toolbarActions`, `.actionButton`, `.actionButtonPrimary`. Drops the old `editButton`/`saveButton`/`cancelButton` rules (replaced by the `actionButton`/`actionButtonPrimary` pair). |
| `src/components/mod.rs` | `pub mod renderer_page;` → `pub mod reader;`. Doc-comment updated. |
| `src/components/router.rs` | Imports `Reader` from `crate::components::reader`. Updates the four mount sites where `RendererPage` was named. |

### Files NOT changed

- All editing logic (`save_raw`, `derive_new_path`, `placeholder_frontmatter`, `RawMempoolMeta`) — pure helpers, still right.
- Master plan A9 reserved-prefix list — Phase 7 already settled this.

## 5. Toolbar CSS sketch

```css
.toolbar {
  display: flex;
  align-items: baseline;
  gap: var(--space-3_5);
  padding: var(--space-1_5) 0;
  margin-bottom: var(--space-4);
  border-bottom: 1px dashed var(--border-subtle);
  font-family: var(--font-mono);
  font-size: var(--font-size-xs);
  letter-spacing: 0.04em;
  color: var(--text-dim);
  text-transform: uppercase;
}

.toolbarLabel {
  color: var(--terminal-yellow);
  font-weight: 600;
  letter-spacing: 0.06em;
}

.toolbarActions {
  margin-left: auto;
  display: flex;
  gap: var(--space-2);
  text-transform: none;
  letter-spacing: 0;
}

.actionButton,
.actionButtonPrimary {
  border: 1px solid var(--border-muted);
  background: transparent;
  color: var(--text-dim);
  padding: 0 var(--space-2);
  font-family: inherit;
  font-size: 11px;
  letter-spacing: 0.04em;
  cursor: pointer;
  line-height: 1.6;
}

.actionButtonPrimary {
  color: var(--accent);
  border-color: var(--accent);
}

.actionButton:hover,
.actionButtonPrimary:hover {
  color: var(--text-primary);
  border-color: var(--accent);
}

.actionButton:disabled,
.actionButtonPrimary:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
```

The `.errorBanner` and `.editorTextarea` rules carry forward unchanged from Phase 7's `renderer_page.module.css`.

## 6. State diagram

State logic stays as Phase 7 defined: `mode`, `draft_body`, `save_error`, `saving`, `refetch_epoch`, `raw_source`, the prev-guarded path-change reset Effect, the `/new` author-mode redirect Effect. Phase 8 changes the presentation of the action affordances, plus one model addition:

### 6.1 Draft preservation across `preview` round-trip

Phase 7's `on_toggle_edit` handler re-seeds `draft_body` from `raw_source` whenever the user toggles to Edit (`renderer_page.rs:144-153`). This is correct for the *first* toggle into Edit (we want the on-disk source as the starting point), but it would clobber an in-flight draft on the round trip Edit → `preview` → `edit`.

Fix: a `draft_dirty: RwSignal<bool>` flag — set to `true` whenever the user types into the textarea (or by `on_toggle_edit` itself when seeding from `raw_source`); `on_toggle_edit` skips the re-seed when `draft_dirty` is already `true`. Cancel resets `draft_dirty = false` along with reverting `draft_body`. Save success also resets it.

Equivalent shape: track the seed source — initial seed comes from `raw_source` once; subsequent Edit entries skip the seed unless `draft_body` is empty. Both work; the `draft_dirty` flag is more legible.

**`preview` does not change `canonical_path`.** It only flips `mode` (Edit → View). The prev-guarded path-change reset Effect (Phase 7 §7.1, post-fix `renderer_page.rs:103-110`) keys on `canonical_path.get()`, so `preview` toggles are invisible to the reset Effect and `mode` is safe.

## 7. Decision-log entry (to append at completion)

| Date | Decision | Reference |
|---|---|---|
| 2026-04-29 | Phase 8 — Reader component extraction. Phase 7 placed view/edit affordances into `SiteChrome.extra_actions`, which conflated document-level actions with site-level chrome. Phase 8 extracts `Reader` (renamed from `RendererPage`) into its own module with an internal `ReaderToolbar` sub-component. Toolbar lives at the top of the article frame, matches the ledger.html aesthetic (dashed rule, monospace, accent buttons), only renders when there are actions or a label to show. State logic unchanged; pure UX/structure refactor. | §3 |

## 8. Out of scope

- Live split-pane preview (toggle is enough for V1; Phase 7 already settled).
- Markdown editor enhancements (autocomplete, syntax highlighting, hot-key palette).
- Renaming the router's `mod.rs` references — only the four `RendererPage` → `Reader` mount sites get touched.
- Adding back-end paths to `SiteChromeActions` for site-level affordances; the slot is parked for future use.

## 9. Verification

Build state stays green at each step. Live walk-through after impl:

1. Visit a non-mempool path (e.g. `/#/papers/foo`) as any user → no toolbar above the article. Site chrome unchanged.
2. Visit a mempool path as non-author → no toolbar.
3. Visit a mempool path as author → toolbar with right-aligned `edit` button. Click → mode flips to Edit. Toolbar shows `editing` label + `cancel` + `save`.
4. Click `cancel` → reverts. `save` → flips back to View, refetches.
5. Visit `/#/new` as author → toolbar with `new draft` label + `cancel` + `save`. Textarea pre-filled with placeholder. Toggle to View (via the right-side `edit` button after first toggling? No — see §3 table: the toggle from Edit→View on /new is via `edit`'s sibling not yet specced). **Decision below.**
6. Visit `/#/new` as non-author → URL replaces to `/#/ledger`.

### 9.1 Edit ↔ Preview round-trip

Phase 7's design promised "View mode renders `draft_body` through `render_markdown` so an unsaved draft preview matches what View shows" — but Phase 7 shipped no UI to *trigger* View while editing. The user has to save or cancel to leave Edit. Phase 8 closes that gap, on-spec for Phase 7's design contract.

Decision: in Edit mode (on `/new` *and* on existing entries), the toolbar shows a `preview` button alongside `cancel` and `save`. Clicking `preview` flips `mode` to View while leaving `draft_body` and `draft_dirty` untouched. From View mode, the existing `edit` button returns to Edit (and skips the `raw_source` re-seed because `draft_dirty == true`, preserving the user's typed work — see §6.1).

Updated §3 table for Edit-mode toolbar:

| State | Toolbar contents |
|---|---|
| Edit on existing mempool entry | Left: `editing` label · right: `[ preview ] [ cancel ] [ save ]` |
| Edit on `/new` | Left: `new draft` label · right: `[ preview ] [ cancel ] [ save ]` |

This is HackMD-like: peek-the-rendered, return-to-source. Save is intentionally absent from View mode — the user flips to Edit before saving so the action site is unambiguous.
