# Phase 8 — Implementation Plan

**Design:** [`docs/superpowers/specs/2026-04-29-mempool-phase8-design.md`](../specs/2026-04-29-mempool-phase8-design.md) (v2)
**Date:** 2026-04-29

Three ordered steps, one commit each. `cargo check --target wasm32-unknown-unknown --lib` and `cargo test --lib` stay green at every step. Order is important: extract `Reader` with its own toolbar **before** dropping `SiteChrome.extra_actions`, so every commit has a working edit affordance.

---

## Step 1 — Rename `RendererPage` → `Reader`, extract toolbar, draft preservation

The bulk of Phase 8.

### 1.1 Rename file and component

- `git mv src/components/renderer_page.rs src/components/reader.rs`
- `git mv src/components/renderer_page.module.css src/components/reader.module.css`
- Inside `reader.rs`: rename `pub fn RendererPage(...)` → `pub fn Reader(...)`. Keep the body but plan to refactor it within this commit.
- Update the `stylance::import_crate_style!(css, ...)` path string.
- `src/components/mod.rs`: `pub mod renderer_page;` → `pub mod reader;`. Update the doc-comment line (`renderer_page` → `reader`, with description "View/edit reader page (also handles `/new` compose)").
- `src/components/router.rs`: replace all `RendererPage` mounts with `Reader`. Update the `use` import: `use crate::components::reader::Reader;`. There are three mount sites (the `/new` synthetic-frame mount + the two FS-resolved mounts in the match arm).

### 1.2 Add `ReaderToolbar` sub-component

Inside `reader.rs`, add a private `ReaderToolbar` component following the design §4 props:

```rust
#[component]
fn ReaderToolbar(
    mode: RwSignal<ReaderMode>,
    is_new: Memo<bool>,
    can_edit: Memo<bool>,
    saving: ReadSignal<bool>,
    on_edit: Callback<()>,
    on_preview: Callback<()>,
    on_save: Callback<()>,
    on_cancel: Callback<()>,
) -> impl IntoView { /* ... */ }
```

Render only when `(mode == View && can_edit) || mode == Edit`. The label cycles through:

- View on `/new`: `new draft · preview`
- View on existing mempool path: (no label)
- Edit on `/new`: `new draft`
- Edit on existing mempool path: `editing`

Right-aligned actions per design §3 + §9.1:
- View + can_edit: `[ edit ]` (calls `on_edit`).
- Edit: `[ preview ]` `[ cancel ]` `[ save ]` (call `on_preview`, `on_cancel`, `on_save` respectively; `cancel` and `save` disabled while `saving.get()`; `save` styled as primary).

### 1.3 Restructure `Reader`'s state and handlers

- Add `draft_dirty: RwSignal<bool>` (default `false`).
- Construction-time seed (existing for `/new`): set `draft_dirty = true` because the placeholder is a draft the user is now responsible for.
- `on_toggle_edit` handler: if `draft_dirty.get_untracked()`, *do not* re-seed `draft_body`; just flip `mode` to Edit. Otherwise, seed from `raw_source` and set `draft_dirty = true`. Clear `save_error` either way.
- `on_preview` (new): flip `mode` to View. Do not touch `draft_body` or `draft_dirty`.
- `on_cancel`:
  - `/new`: `replace_request_path("/ledger")` (unchanged).
  - existing: `draft_body.set(raw_source.get().unwrap_or_default())`, `draft_dirty.set(false)`, `save_error.set(None)`, `mode.set(View)`.
- `on_save` success branch (existing): also `draft_dirty.set(false)`. New-save success branch unchanged (the page navigates away).
- `<textarea on:input=...>`: also `draft_dirty.set(true)` on input.
- Path-change Effect (prev-guarded): on a real path change (not first mount), reset `draft_dirty = false` alongside `mode` and `save_error`. **Note:** the Effect does *not* reset `draft_body`. If Leptos keeps the component identity across navigation (`into_any()` boundary), an in-flight draft from entry A would survive in `draft_body` until the user next toggles to Edit (which then re-seeds because `draft_dirty == false`). Acceptable; add a code comment near the Effect to capture this invariant.

### 1.4 Replace the chrome `extra_actions` mount with the toolbar

Inside `Reader`'s `view!`:

- Drop the Phase 7 `let extra_actions: ChildrenFn = Arc::new(...)` block entirely.
- Drop `use std::sync::Arc;`.
- The `<SiteChrome>` call reverts to `<SiteChrome route=route />` (no `extra_actions=`).
- Between `<SiteChrome>` and the save_error banner, mount the new toolbar:

```rust
<ReaderToolbar
    mode=mode
    is_new=is_new_route
    can_edit=edit_visible
    saving=saving.read_only()
    on_edit=Callback::new(on_toggle_edit)
    on_preview=Callback::new(on_preview)
    on_save=Callback::new(on_save)
    on_cancel=Callback::new(on_cancel)
/>
```

The handlers are extracted as named closures (each takes `()` to match `Callback<()>`'s signature). The save_error banner and textarea/markdown switch keep their Phase-7 positions (toolbar above, banner above content, content below).

### 1.5 CSS

**File:** `src/components/reader.module.css`

- **Delete** the `.editButton`, `.saveButton`, `.cancelButton` rule blocks (Phase 7 added these for the chrome-mounted buttons; the toolbar uses a new pair).
- **Add** `.toolbar`, `.toolbarLabel`, `.toolbarActions`, `.actionButton`, `.actionButtonPrimary` per design §5. Keep `.editorTextarea` and `.errorBanner` from Phase 7 (still needed).

### 1.6 Verification

- `cargo check --target wasm32-unknown-unknown --lib` clean.
- `cargo test --lib` 503 passing (no test-suite changes; component tests not added).
- Stylance class hashes change because the source path changed (`renderer_page.module.css` → `reader.module.css`); confirmed no external selector references with `grep -rE '(editButton|saveButton|cancelButton|actionButton|toolbar(Label|Actions)?)' assets/ src/ tests/`. The bundle regenerates cleanly.
- Live walk-through (per design §9):
  1. `/#/papers/foo` (any user): no toolbar. Site chrome unchanged.
  2. `/#/mempool/writing/<existing>` (non-author): no toolbar.
  3. `/#/mempool/writing/<existing>` (author): toolbar visible, right-aligned `edit` button. Click → label flips to `editing`, buttons swap to `preview cancel save`.
  4. Edit, type a few characters, click `preview` → renders the live draft (frontmatter fence visible). Toolbar label: (no label, since not `/new`); right side: `edit` button.
  5. Click `edit` from preview → returns to Edit, textarea content survives (draft_dirty preserved).
  6. Click `cancel` from Edit → reverts to on-disk source; `draft_dirty = false`.
  7. `/#/new` (author): toolbar with `new draft` label + `preview cancel save`. Textarea pre-seeded with placeholder.
  8. Type into placeholder, click `preview` → label becomes `new draft · preview`; rendered output shown.
  9. Click `edit` → returns to Edit; typed content preserved.
  10. Click `save` with title + valid category → URL navigates to view URL of new entry. Toolbar shows `edit` button (existing-mempool author View).
  11. Click `cancel` from `/new` Edit → URL replaces to `/#/ledger`.
  12. `/#/new` non-author → URL replaces to `/#/ledger`.

**Commit:** `feat(reader): extract Reader from RendererPage; toolbar, preview, draft preservation`

---

## Step 2 — Drop `SiteChrome.extra_actions` prop

After Step 1, no caller passes `extra_actions` to `SiteChrome`. Prune YAGNI.

**File:** `src/components/chrome/mod.rs`

- Remove the `#[prop(optional, into)] extra_actions: Option<ChildrenFn>` parameter from `SiteChrome` (lines 65-72 area).
- Remove the `{extra_actions.map(|c| c())}` mount inside `SiteChromeActions` (line 132 area).

**Verification:** `cargo check --target wasm32-unknown-unknown --lib` clean; `cargo test --lib` 503 passing.

**Commit:** `refactor(chrome): drop unused SiteChrome.extra_actions prop`

---

## Step 3 — Master plan + decision log

**File:** `docs/superpowers/specs/2026-04-28-mempool-master.md`

- §4 phase table: add Phase 8 row (Complete).
- §6 document index: add Phase 8 design + plan rows.
- §10 decision log: append the Phase 8 entry per design §7.
- A9 paragraph: no change (already settled in Phase 7; `/new` stays reserved, `/edit/` already dropped).

**Verification:** docs only.

**Commit:** `docs(mempool): Phase 8 master plan + design + plan`

---

## Final reviewer pass

After Step 3: `superpowers:code-reviewer` on the cumulative diff. Address HIGH/CRITICAL findings before declaring complete.

## Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| `draft_dirty` race: user toggles `preview` mid-input event, draft_body update lost | Low | `<textarea on:input>` runs synchronously before the `preview` button click can fire. Leptos signal updates are immediate. |
| `ReaderToolbar`'s callbacks capture stale signals after a mount-time signal swap | Low | All toolbar callbacks dispatch via `Callback<()>`; the closures inside `Reader` close over the originals. No staleness. |
| The `<header>` element's `<Show when=...>` hides the toolbar entirely when not author/mempool — but a layout shift may occur on author-mode flip mid-session | Low | `mode` and `can_edit` flips are rare (token-add events). A one-time layout reflow on flip is acceptable. |
| Renaming the file confuses `git blame` / future archaeology | Low | Use `git mv` so blame follows. Single-commit rename is easy to spot. |
| `LocalResource` source closures still subscribe to `refetch_epoch` after the file rename | Trivial | Identifiers and signal types are unchanged; only the filename changed. |
| Stylance class-name hashes change after the CSS file rename (`renderer_page.module.css` → `reader.module.css`) | Low | Cargo.toml's stylance config hashes by source path. Confirmed by `grep` that no external code references the emitted classes (`editButton`, `saveButton`, etc.). Bundle regenerates with new hashes; no breakage. |
| User has unsaved `/new` draft, hits browser back | Low | Page unmounts, draft is lost. Accepted: V1 has no autosave. |
| Phase 7's named handlers `on_save`/`on_cancel` already exist as `move |_|` (`MouseEvent`-shaped); converting to `Callback<()>` requires a small signature change | Trivial | `Callback::new(move |()| { ... })` — type-checked at extract time. |

## Total scope estimate

| Bucket | Lines |
|---|---|
| Step 1 — `extra_actions` removal | ~ -10 |
| Step 2 — rename + toolbar + draft_dirty + CSS | ~ +120 / -35 |
| Step 3 — docs | ~ +25 |
| Net | | ~ +100 |

Most of Step 2's added lines are `ReaderToolbar` (≈ 60 lines) + new CSS rules (≈ 30) + handler refactor (≈ 30). The component still fits under 500 lines.
