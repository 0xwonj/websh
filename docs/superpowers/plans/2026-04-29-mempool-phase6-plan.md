# Phase 6 — Implementation Plan

**Design:** [`docs/superpowers/specs/2026-04-29-mempool-phase6-design.md`](../specs/2026-04-29-mempool-phase6-design.md) (v3)
**Date:** 2026-04-29

This plan executes the design in six ordered phases (A–F). Each phase keeps `cargo check --target wasm32-unknown-unknown --lib` and `cargo test --lib` green so the work can be committed incrementally.

---

## Phase A — Infrastructure (foundation; no UI change)

Goal: land the helpers and API extensions that subsequent phases depend on. After Phase A the existing app behaves identically; new symbols are unused.

### A1. `dom::dispatch_hashchange()`

**File:** `src/utils/dom.rs`

Add a public helper at the bottom of the file:

```rust
pub fn dispatch_hashchange() {
    if let Some(window) = window() {
        if let Ok(event) = web_sys::Event::new("hashchange") {
            let _ = window.dispatch_event(&event);
        }
    }
}
```

Verification: `cargo check --target wasm32-unknown-unknown --lib`.

### A2. `replace_request_path` dispatches `hashchange`

**File:** `src/core/engine/routing.rs:125-127`

Modify:

```rust
pub fn replace_request_path(path: &str) {
    dom::replace_hash(&format!("#{}", normalize_request_path(path)));
    dom::dispatch_hashchange();
}
```

Backwards-compat: only `shell.rs:112` calls this (via `RouteRequest::replace()`); the existing `if frame.request.url_path != canonical` guard prevents the synthetic event from re-firing the same Effect. Manual sanity: load shell, observe URL bar still normalizes correctly, no infinite loop.

Verification: `cargo check --target wasm32-unknown-unknown --lib`; `trunk serve` and visit `/#/websh/something-non-canonical` → URL bar normalizes once and stops. **Note:** A2 only confirms no infinite loop in the existing `shell.rs:112` flow — the actual `dispatch_hashchange` regression coverage (a route-changing redirect) lives at C4 step "non-author visits `/#/new`".

### A3. Path-shape helpers in `routing.rs`

**File:** `src/core/engine/routing.rs`

Add near the existing helpers (e.g., after `normalize_request_path`):

```rust
pub fn is_new_request_path(req: &RouteRequest) -> bool {
    req.url_path.trim_matches('/') == "new"
}

pub fn edit_request_path_inner(req: &RouteRequest) -> Option<&str> {
    req.url_path.strip_prefix("/edit/")
}
```

Add unit tests:

- `is_new_request_path` accepts `/new`, `new`, `/new/`; rejects `/news`, `/new/foo`, `/`, `/edit`.
- `edit_request_path_inner` returns `Some("mempool/writing/foo")` for `/edit/mempool/writing/foo`; `None` for `/edit` (no slash), `/new`, `/foo`, `/`.

Re-export from `core/engine/mod.rs` (line 19 area).

Verification: `cargo test --lib`.

### A4. `SiteChrome.extra_actions` slot

**File:** `src/components/chrome/mod.rs:65-128`

Modify the `SiteChrome` component signature:

```rust
#[component]
pub fn SiteChrome(
    route: Memo<RouteFrame>,
    #[prop(optional, into)] extra_actions: Option<ChildrenFn>,
) -> impl IntoView {
    // ... existing body
    <SiteChromeActions>
        <SiteChromeNav>...</SiteChromeNav>
        <SiteChromeDivider />
        <SiteChromePalettePicker theme=theme />
        {extra_actions.map(|c| c())}
    </SiteChromeActions>
    // ...
}
```

Existing call sites (`renderer_page.rs`, `ledger_page.rs`, `home.rs`, `terminal/shell.rs` — verified by grep) pass only `route=` and keep working.

Verification: `cargo check --target wasm32-unknown-unknown --lib`. `trunk serve` and confirm chrome still renders identically on `/`, `/ledger`, `/websh`, `/papers/foo`. **Note:** A4 is a structural change only — `extra_actions` is `None` at every call site until F1, so the live exercise of the slot's reactive evaluation lands at F1, not here.

### A5. Commit point

```
feat(infra): add hashchange dispatch + path-shape helpers + SiteChrome extra_actions slot
```

Phase A complete.

---

## Phase B — `MempoolEditor` component (new, unused)

Goal: build the un-modal'd compose form. After Phase B the component is mountable but no caller references it.

### B1. `src/components/mempool/editor.rs`

Create a new file with the `MempoolEditor` component. The body is a near-copy of the current `ComposeModal` body (`compose.rs:380-693`), with these changes:

- **Drop the modal frame**: no `<Show when=open.is_some()>`, no `<div class=css::backdrop>`, no `<div class=css::panel>`, no `<header>`/`<button class=css::close>`. The form mounts directly.
- **Replace `(open, set_open)` signals with the prop `mode: ComposeMode`**: seed once on mount, no Effect-driven seeding loop.
- **Replace `on_saved: Callback<()>` with `on_saved: Callback<VirtualPath>`**: pass the resolved canonical save path back to the page.
- **Add `on_cancel: Callback<()>`**: triggered by the Cancel button or Esc key.

Skeleton:

```rust
#[component]
pub fn MempoolEditor(
    mode: ComposeMode,
    #[prop(into)] on_saved: Callback<VirtualPath>,
    #[prop(into)] on_cancel: Callback<()>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let initial = derive_form_from_mode(&mode, &iso_today());
    let form = RwSignal::new(initial.clone());
    let errors = RwSignal::new(validate_form(&initial));
    let save_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);

    // ... reuse the field-input handlers, save handler, keyboard handler from compose.rs
    // adapted to call on_saved(saved_path) on Ok and on_cancel(()) on cancel/Esc.

    view! { <div class=css::editor /* ... form rows ... */ /> }
}
```

The save handler:

```rust
let try_save: Callback<()> = Callback::new(move |_| {
    if saving.get_untracked() { return; }
    let snapshot = form.get_untracked();
    let errs = validate_form(&snapshot);
    if !errs.is_empty() {
        errors.set(errs);
        save_error.set(Some("fix the highlighted fields before saving".into()));
        return;
    }
    let saved_path = save_path_for(&mode, &snapshot);
    save_error.set(None);
    saving.set(true);
    let ctx_clone = ctx.clone();
    let mode_clone = mode.clone();
    spawn_local(async move {
        let result = save_compose(ctx_clone, mode_clone, snapshot).await;
        saving.set(false);
        match result {
            Ok(()) => on_saved.run(saved_path),
            Err(message) => save_error.set(Some(message)),
        }
    });
});
```

Note: `save_path_for` is computed *before* the await, so the saved path is known synchronously and can be handed back without re-deriving from the form post-save.

### B2. `src/components/mempool/editor.module.css`

Copy the field/row/label/input/select/textarea/footer/error rules from `compose.module.css`. Drop `backdrop`, `panel`, `close`, modal-shaped `header`. Add a top-level `.editor` rule that gives the form page-level breathing room (e.g., `padding`, `max-width`).

### B3. Re-export from `mempool/mod.rs`

Add `pub mod editor;` and `pub use editor::MempoolEditor;`. Keep all existing re-exports for now.

### B4. Verification

`cargo check --target wasm32-unknown-unknown --lib`. No new tests yet (component-level Leptos tests are out of scope; pure helpers are already tested).

### B5. Commit point

```
feat(mempool): MempoolEditor un-modal'd compose component
```

---

## Phase C — `MempoolEditorPage` + router branches

Goal: wire the new pages so `/#/new` and `/#/edit/...` work end-to-end. After Phase C the editor is reachable, but the existing modals still exist (unused for new flows but mounted for old click paths).

### C1. `src/components/mempool_editor_page.rs`

Create the page wrapper. Public surface:

```rust
#[derive(Clone, Debug)]
pub enum MempoolEditorPageMode {
    New,
    Edit { request_path: String },
}

#[component]
pub fn MempoolEditorPage(mode: MempoolEditorPageMode) -> impl IntoView { /* ... */ }
```

Inside:

1. Read `ctx.runtime_state` for `author_mode`. Compute as `Memo`.
2. **Effect-based redirect** (per design §7): on first run, if `!author_mode`, call `replace_request_path(redirect_target)`.
3. **Render-time `<Show when=author_mode fallback=redirect_placeholder>`** wraps the body.
4. For `New`: build `ComposeMode::New { default_category: None }` and mount `<MempoolEditor mode=... on_saved=... on_cancel=... />`. **Intentional regression** vs the old modal flow: `+ compose` from a category-filtered ledger no longer pre-fills the dropdown. Per user direction (Korean session, "category 는 그 안에서 정하는게 나을 것 같고") and design §3 ("`/#/new` — Author-mode only"), the new flow does not propagate filter state via URL params; the user picks the category in the form. Document at F2 in the master-plan decision log.
5. For `Edit`: validate path-shape (§8 table). If any check fails, render an error frame. Otherwise wrap the body fetch in `LocalResource::new`, then mount `MempoolEditor` once the body resolves.
6. Navigation callbacks:
   - `on_saved(path)` → `push_request_path(&content_route_for_path(path.as_str()))`.
   - `on_cancel()` →
     - New mode: `push_request_path("/ledger")`.
     - Edit mode: `push_request_path(&content_route_for_path(<resolved canonical path>))`.

Pure helper for §8 row checks (extract for testability):

```rust
pub(crate) enum EditPathCheck {
    Ok { canonical: VirtualPath },
    NotMempool,
    NotFound,
    NotEditable, // wrong kind
    NotMarkdown,
}

pub(crate) fn check_edit_path(fs: &GlobalFs, request_path: &str) -> EditPathCheck { /* ... */ }
```

Unit-test `check_edit_path` with the 4 failure cases + happy path.

The error-frame component is small and local to this module (or extracted if shared).

### C2. `src/components/mod.rs`

Add `pub mod mempool_editor_page;` (keep `pub mod reader;` for now — deleted in Phase E).

### C3. Router branches

**File:** `src/components/router.rs`

Inside `RouterView` body, before the existing FS-resolved match arm, add (matching the existing `if _raw_request.with(is_builtin_home_route)` style):

```rust
if _raw_request.with(is_new_request_path) {
    return view! {
        <MempoolEditorPage mode=MempoolEditorPageMode::New />
    }.into_any();
}
if let Some(rest) = _raw_request.with(|r| edit_request_path_inner(r).map(str::to_string)) {
    return view! {
        <MempoolEditorPage mode=MempoolEditorPageMode::Edit { request_path: rest } />
    }.into_any();
}
```

Imports: `is_new_request_path`, `edit_request_path_inner` from `crate::core::engine::routing`; `MempoolEditorPage` and `MempoolEditorPageMode` from `crate::components::mempool_editor_page`.

### C4. Verification

`cargo check --target wasm32-unknown-unknown --lib`; `cargo test --lib` (new `check_edit_path` tests pass).

`trunk serve` smoke:

- As author (token in session), visit `/#/new` → empty form renders.
- As author, visit `/#/edit/mempool/writing/<existing-slug>` → form seeded with existing source.
- As author, visit `/#/edit/papers/foo` → "not editable" frame.
- As author, visit `/#/edit/mempool/writing/does-not-exist` → "no such mempool entry" frame.
- As non-author (clear token), visit `/#/new` → URL replaces to `/#/ledger` and ledger renders (no stuck "redirecting…").
- Click an item in the mempool list: still triggers the existing modal (unchanged in this phase). That stays for Phase D.

### C5. Commit point

```
feat(mempool): MempoolEditorPage + /new and /edit/<path> routes
```

### C6. Interim reviewer pass

After C5, run the `superpowers:code-reviewer` subagent on the cumulative diff (Phases A + B + C). This is the smallest commit at which the editor is end-to-end verifiable as a real page; reviewer can drive it live and catch any extraction defect from B1 before D/E/F build on it. Address HIGH/CRITICAL findings before proceeding to Phase D.

---

## Phase D — Mempool item rewiring + LedgerPage cleanup

Goal: switch mempool item clicks and `+ compose` to URL navigation. After Phase D the modal mounts in `LedgerPage` are gone and the modal components are dead code (deleted in Phase E).

The e2e suite asserts both behaviors Phase D inverts (`tests/e2e/mempool.spec.js:34-46`); the e2e update is part of this same phase, not a follow-up.

### D1. `Mempool` component → URL anchors

**File:** `src/components/mempool/component.rs`

- Drop the `on_select: Callback<MempoolEntry>` and `on_compose: Callback<()>` props from `Mempool`.
- In `MempoolItem`: replace the outer `<div tabindex="0" role="button" on:click=... on:keydown=...>` with `<a href=content_href_for_path(entry.path.as_str()) class=item_class>`. Drop the `on_click` prop. Anchor inherits keyboard accessibility from the link semantics. Keep `tabindex` only if the visual focus styling depends on it (it usually doesn't with `<a>`).
- In the header: replace `<button class=css::mpCompose on:click=...>` with `<a class=css::mpCompose href="/#/new" aria-label="Compose new mempool entry">`. Existing `<Show when=author_mode>` gating stays.
- Update CSS to ensure the `<a>` looks identical to the `<button>` (no underline, same border, same hover) — should already match `.mpCompose` rules; spot-check.

### D2. `LedgerPage` cleanup

**File:** `src/components/ledger_page.rs`

Remove:

- `let (preview_open, set_preview_open) = signal(None::<VirtualPath>);`
- `let (compose_open, set_compose_open) = signal(None::<ComposeMode>);`
- `let mempool_refresh = RwSignal::new(0u32);` and the `_ = mempool_refresh.get();` read in the LocalResource closure.
- `let on_mempool_select = ...`, `let on_compose_new = ...`, `let on_compose_saved = ...` Callbacks.
- The `<Mempool ... on_select=... author_mode=author_mode on_compose=... />` props `on_select` and `on_compose`.
- The `<MempoolPreviewModal .../>` and `<ComposeModal .../>` mounts at the bottom of the surface.
- The unused `pub use` from the `use crate::components::mempool::{...}` import: drop `ComposeModal`, `ComposeMode`, `MempoolEntry`, `MempoolPreviewModal` (none are referenced after the cleanup).
- `filter_category_from_route` if unused after removing `on_compose_new`. Verify with `cargo check`.

The `Mempool` invocation simplifies to:

```rust
<Mempool
    model=mempool_model
    author_mode=author_mode
/>
```

(`author_mode` may be unused inside the Mempool component if the only gate it controls is the compose link, which now has its own `<Show>`. If so, drop the prop too — cleanup pass.)

### D3. Update `tests/e2e/mempool.spec.js`

The current `'clicking a row opens the modal preview without URL change'` test (lines 33-46) asserts URL stays unchanged and `[aria-label="Close preview"]` becomes visible. Both invariants flip in Phase D. Rewrite the test to match URL nav:

```js
test('clicking a row navigates to the entry view', async ({ page }) => {
  await page.goto(`${baseUrl}/#/ledger`);
  const initialHash = await page.evaluate(() => window.location.hash);
  const firstRow = page
    .locator('section[aria-label="Mempool — pending entries"] a')
    .first();
  const href = await firstRow.getAttribute('href');
  expect(href).toMatch(/^\/#\/(mempool|writing|papers|projects|talks|misc)\//);
  await firstRow.click();
  const afterClickHash = await page.evaluate(() => window.location.hash);
  expect(afterClickHash).not.toBe(initialHash);
  // No more modal close button — confirm URL change is sufficient
  await expect(page.locator('[aria-label="Close preview"]')).toHaveCount(0);
});
```

The selector update from `[role="button"]` (D1 anchor swap drops `role="button"` since `<a>` carries link semantics natively) is reflected in the new selector `... a`.

Also update line 25 (`const itemKinds = await mempool.locator('[role="button"] [data-kind]')...`) to `mempool.locator('a [data-kind]')`.

### D4. Verification

`cargo check --target wasm32-unknown-unknown --lib`; `cargo test --lib`.

`trunk serve` smoke:

- Click a mempool item: URL changes to `/#/<path>`, RendererPage renders. No modal.
- Confirm `aria-label="Mempool — pending entries"` still on the section (selector parity).
- Click `+ compose` (as author): URL changes to `/#/new`, MempoolEditorPage renders. No modal.
- Save in `/#/new`: URL changes to view URL, content rendered. No modal.

E2E (post-trunk-serve at port 4173 per CLAUDE.md):

```bash
WEBSH_E2E_BASE_URL=http://127.0.0.1:4173 NODE_PATH=target/qa/node_modules \
  target/qa/node_modules/.bin/playwright test tests/e2e/mempool.spec.js \
  --reporter=line --workers=1
```

Must pass.

### D5. Commit point

```
refactor(mempool): mempool list items navigate by URL, drop modal mounts
```

---

## Phase E — Deletions + Reader removal

Goal: remove the dead modal components, `Reader`, and their CSS.

### E1. Delete files

- `src/components/mempool/preview.rs`
- `src/components/mempool/preview.module.css`
- `src/components/reader/mod.rs`
- `src/components/reader/reader.module.css`

Remove the `reader` directory entirely.

### E2. Delete `ComposeModal` component (keep helpers)

**File:** `src/components/mempool/compose.rs`

- Delete the `#[component] pub fn ComposeModal(...) -> impl IntoView { ... }` (lines 380-693 — the entire component definition).
- Delete the `stylance::import_crate_style!(css, "src/components/mempool/compose.module.css");` import. The compose-modal CSS is copied/adapted in `editor.module.css` (B2); helpers in `compose.rs:1-378` reference no CSS.
- Delete `src/components/mempool/compose.module.css` outright. Verify with `grep -r compose.module.css src/` returns empty before deletion.

### E3. Update `mod.rs` files

**File:** `src/components/mempool/mod.rs`

- Remove `mod preview;`.
- Remove `pub use preview::MempoolPreviewModal;`.
- Remove `ComposeModal` from `pub use compose::{...}` (other exports stay).

**File:** `src/components/mod.rs`

- Remove `pub mod reader;` (line 26 area).
- Remove the doc-comment line referencing reader (if any).

### E4. Verification

`cargo check --target wasm32-unknown-unknown --lib`; `cargo test --lib`. Both must be green — these are pure deletions that should not affect anything if Phase D removed all references correctly. If any reference remains, `cargo check` names it; fix that reference and re-run.

`trunk build --release` — confirms CSS bundling still works.

`trunk serve` smoke (full walkthrough from design §11 verification plan steps 1–15).

### E5. Commit point

```
chore(mempool): delete Reader, MempoolPreviewModal, ComposeModal component
```

---

## Phase F — RendererPage edit affordance + master plan updates

Goal: surface the edit link on view pages and lock in the design's documentation changes.

### F1. RendererPage edit affordance

**File:** `src/components/renderer_page.rs`

Add the `edit_visible` / `edit_href` Memos and pass `extra_actions` to `SiteChrome`, per design §12.2. CSS rule for `.editLink` in `renderer_page.module.css` (or add to an existing chrome-action class if one applies).

Verification: `trunk serve`, as author visit `/#/papers/foo` → no edit link; visit `/#/mempool/writing/<existing-slug>` → edit link visible in chrome; click → `/#/edit/...` resolves.

### F2. Master plan updates

**File:** `docs/superpowers/specs/2026-04-28-mempool-master.md`

- Edit anchor row A5 inline to mark it dropped: replace the description with "**Dropped 2026-04-29.** Mempool items use URL navigation; the original URL-hiding rationale conflicted with A1's public-by-design framing. See Phase 6 design §2."
- Add the §10 decision-log row from Phase 6 design §9 (long-form).
- Add a "reserved URL prefixes" note alongside §3, listing `/`, `/ledger`, `/websh`, `/explorer`, `/new`, `/edit/`.
- Mark Phase 6 in §4 as Complete (with the design + plan doc links).

### F3. Commit point

```
feat(mempool): edit affordance on view pages + Phase 6 docs
```

---

## Final verification

Before declaring Phase 6 complete:

```bash
cargo check --target wasm32-unknown-unknown --lib
cargo test --lib
cargo build --bin websh-cli   # sanity, no expected change
trunk build --release
```

Live walkthrough (all 15 steps from design §11). Capture any failures as new TODOs (not regressions of Phase 6's spec).

Reviewer agent on the diff after F3: code-reviewer subagent for HIGH/CRITICAL severity issues. Address before push.

## Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| `replace_state_with_url` + synthetic `hashchange` differs across browsers | Low | Phase A2 verifies in Chrome via `trunk serve`; Safari/Firefox parity is standard for this Web API. |
| `ChildrenFn` clone semantics surprise in `SiteChrome` | Low | Only one new caller (RendererPage); test live. |
| `MempoolEditor` form-input handlers diverge from extracted `ComposeModal` body and miss validation | Medium | Phase B is a near-mechanical copy; existing helper unit tests still cover validation. Run `cargo test --lib` after B. |
| LedgerPage cleanup leaves dangling unused symbols | Low | `cargo check` catches unused imports; re-run after D2 to clean. |
| Mempool body fetch under `/edit/...` hits raw.githubusercontent.com cache (existing quirk) | Medium | Out of scope for Phase 6 per design §10; fix under separate ticket if QA reveals stale-edit. |

## Total scope estimate

Per design §13: ~ -650 net lines. Six commits, one per phase, plus optional phase-internal commits if a single phase's diff feels too large to review in one shot. Expected to land in one focused implementation session.
