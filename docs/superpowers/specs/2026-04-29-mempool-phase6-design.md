# Phase 6 — Reader-Unified Mempool UI (No Modals)

**Status:** Design (v3 — addresses Phase 6 design-review findings, two passes)
**Date:** 2026-04-29
**Master plan:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Predecessor designs:**
- Phase 2 (mempool authoring modals — `2026-04-28-mempool-phase2-design.md`)
- Phase 5 (CLI promote pivot — `2026-04-28-mempool-phase5-design.md`)

---

## 1. Problem

Mempool authoring currently lives in two modal surfaces stacked on `LedgerPage`:

- `MempoolPreviewModal` — non-author click → modal-wrapped `Reader`
- `ComposeModal` — author click + `+ compose` button → modal-wrapped form

This produces three coexisting paradigms for "view a mempool entry": clicking from the chain (URL nav → `RendererPage`), clicking from mempool list as non-author (modal), clicking from mempool list as author (compose-edit modal). The user's request: collapse to one. Mempool items navigate by URL; viewing/editing/creating all happen on a page, not in modals.

## 2. Anchor Revision (A5)

The master plan currently states:

> **A5** | Mempool item click → modal preview, not URL navigation | Avoids exposing `/mempool/...` paths in the URL bar; matches the `ledger.html` interaction model

A5's justification — *"avoid exposing `/mempool/...` paths in the URL bar"* — derives from a privacy framing. But:

- **A1** (the foundational anchor) declares the mempool repo *public*: "pending content is naturally public ('pending tx' in a public mempool)".
- A `/mempool/...` URL leaks no information that anyone with the public repo URL can't already read.
- Hiding mempool paths in the URL bar costs UX (no bookmarking, no sharing, no browser-back during edit, no refresh-during-edit) for zero confidentiality benefit.

### Alternatives considered

| Option | Description | Trade |
|---|---|---|
| (a) **Drop A5** | Mempool URLs exposed; full URL-driven flows | Simplest model; matches A1's public-by-design framing; loses the cosmetic URL-hiding |
| (b) Keep A5, reuse component | Click does in-page swap (no URL change) but renders the same component; needs a parent-owned `view_or_edit` signal threaded through | Preserves A5's URL-hiding cosmetic; introduces stateful parent that stays out of sync with URL; refresh-during-edit loses state; browser-back behaves unexpectedly |

**Decision:** (a). The URL-hiding rationale conflicts with A1 and provides no real privacy benefit. Drop A5 entirely; mempool participates in the same URL model as the chain. Decision-log entry added (§9).

## 3. URL Surface

Three URL-distinct flows, all hash-routed:

| URL | Page | Component | When |
|---|---|---|---|
| `/#/<path>` (e.g. `/#/mempool/writing/foo`) | `RendererPage` | view | All viewers, all paths under `/` (canonical content + mempool) |
| `/#/edit/<path>` (e.g. `/#/edit/mempool/writing/foo`) | new `MempoolEditorPage` | `MempoolEditor` (Edit mode) | Author-mode + path under `/mempool/` only; otherwise rejected (§7, §8) |
| `/#/new` | new `MempoolEditorPage` | `MempoolEditor` (New mode) | Author-mode only; non-author redirected to ledger |

URL-distinct edit was preferred over a Reader-internal toggle because:

- **Refresh during edit preserves state** (URL is the truth, not a transient signal).
- **Browser back from edit goes to view** without custom history glue.
- **Save = navigate** to the view URL; no manual `LocalResource` invalidation needed (route memo recomputes automatically when the URL changes; `RendererPage` re-runs its fetch).
- **Three pages with one entry path each** is a smaller mental model than one page with three modes plus a parent-owned mode signal.

Path-collision check: `content/` has no `new/` or `edit/` directories or files (`top-level new.md` does not exist either; `now.toml` is the only shape collision and does not match either `new` or `edit`). Mempool repo has no current `new/` or `edit/` namespace either; if a future mempool entry happened to land at `/mempool/new/` or `/mempool/edit/...`, the rejection rules in §8 keep the editor route from accidentally consuming the request — it would render a "not editable" frame and the user can drop the `/edit/` prefix to view.

**Reserved URL prefixes.** Phase 6 permanently reserves `/new` and `/edit/` at the URL layer. Combined with the existing reserved set (`/`, `/ledger`, `/websh`, `/explorer`), the master plan should record: *content repo (`/site`) and mempool repo MUST NOT introduce files or directories that produce one of these top-level URL segments.* Anchor-or-note added to master plan §3 (alongside A1–A8) at phase-completion time. Until then, treat this as a Phase 6 acceptance constraint.

### 3.1 Component-name disambiguation

The codebase already has `src/components/editor/` containing `EditModal` (a generic text-edit modal). Phase 6 adds a *mempool-specific* page wrapper and inner form component. To avoid cognitive collision:

- Top-level page: `src/components/mempool_editor_page.rs` (not `editor_page.rs`).
- Inner form component: `src/components/mempool/editor.rs` exporting `MempoolEditor`.
- The existing `components::editor::EditModal` is unaffected and stays as-is.

Both new identifiers carry the `mempool` qualifier so `EditorPage` / `Editor` symbols cannot be confused with the generic editor.

## 4. Component Plan

### 4.1 Delete

| File / item | Lines | Reason |
|---|---|---|
| `src/components/mempool/preview.rs` | ~95 | Modal preview deprecated. |
| `src/components/mempool/preview.module.css` | ~35 | Same. |
| `src/components/reader/mod.rs` | 442 | After `MempoolPreviewModal` removal, `Reader` has zero non-modal call sites (verified by grep — only `mempool/preview.rs:9` references it). `RendererPage` is the actual content viewer. |
| `src/components/reader/reader.module.css` | 520 | Same. |
| `pub mod reader;` line in `src/components/mod.rs:26` | 1 | Module declaration must be removed to keep `cargo check` clean. |
| `ComposeModal` Leptos component (in `compose.rs:380–693`) | ~315 | Author flow moves to a page. Helpers (`ComposeForm`, `ComposeMode`, `validate_form`, `save_compose`, `apply_commit_outcome`, etc., lines 1–378) stay. |
| `ReaderMode` enum + all references | ~10 | Disappears with Reader. |
| `pub use preview::MempoolPreviewModal;` and `pub use compose::{..., ComposeModal, ...}` in `src/components/mempool/mod.rs` | 2 | Symbol re-exports for deleted items. |

### 4.2 Add

| File | Purpose |
|---|---|
| `src/components/mempool/editor.rs` | `MempoolEditor` component. Renders the un-modal'd compose form (extracted from current `ComposeModal` body). Props: `mode: ComposeMode`, `on_saved: Callback<VirtualPath>`, `on_cancel: Callback<()>`. Calls `save_compose` on submit. |
| `src/components/mempool/editor.module.css` | Page-level layout styles. Largely copy-adapt of `compose.module.css` form rules; drop the modal-frame rules (`backdrop`, `panel`, `close`, modal-shaped `header`). |
| `src/components/mempool_editor_page.rs` | `MempoolEditorPage` page wrapper for the `/edit/...` and `/new` routes. Owns: `SiteChrome` at top, source-body fetch (Edit mode only — `LocalResource::new`), author-mode gating, navigation callbacks. Mounts `MempoolEditor` once mode is resolved. Render-time gate **and** `Effect`-based redirect for non-author requests, per §7. The `MempoolEditorPageMode` enum (`New` \| `Edit { request_path: String }`) is defined here, not in the router. |
| Edit-affordance link inside existing `RendererPage` chrome | Mounts a single `<a href="/#/edit/<path>">edit</a>` in the existing `SiteChromeActions` slot when `author_mode && path.starts_with("/mempool/")`. See §12 for full spec. |

### 4.3 Modify

| File | Change |
|---|---|
| `src/components/mempool/component.rs` | `MempoolItem`: `<div on:click>` → `<a href={content_href_for_path(entry.path.as_str())}>`. Drop `on_click` callback parameter. `Mempool` header: `+ compose` `<button on:click>` → `<a href="/#/new">`. Drop `on_select` and `on_compose` props from `Mempool`. |
| `src/components/ledger_page.rs` | Drop `preview_open`, `compose_open` signals; drop `on_mempool_select`, `on_compose_new`, `on_compose_saved`, `mempool_refresh`. Drop `<MempoolPreviewModal>` and `<ComposeModal>` from the view. `Mempool` is rendered with no callbacks. |
| `src/components/router.rs` | Add two new branches before the FS-resolved route block: (a) `/new` → mount `MempoolEditorPage` in New mode; (b) `/edit/<rest>` → mount `MempoolEditorPage` in Edit mode with the path-after-`/edit/` carried in the props. |
| `src/core/engine/routing.rs` | Add pure path-shape helpers `is_new_request_path(req)`, `edit_request_path_inner(req) -> Option<&str>`. Unit-tested without Leptos. **Also modify `replace_request_path` to dispatch a synthetic `hashchange` event after `dom::replace_hash`** (per §7.1). |
| `src/utils/dom.rs` | Add a `dispatch_hashchange()` helper (per §7.1). |
| `src/components/chrome/mod.rs` | Extend `SiteChrome` with an `extra_actions: Option<ChildrenFn>` prop (per §12.1). |
| `src/components/mempool/mod.rs` | Drop `pub use` of `MempoolPreviewModal` and `ComposeModal`. Add `pub use editor::MempoolEditor`. |
| `src/components/mempool/compose.rs` | Delete `ComposeModal` component definition (lines 380–693). Keep all helpers. Move `import_crate_style!` import to `editor.rs` if `compose.module.css` is no longer referenced; otherwise the helpers stay file-local. |
| `src/components/mod.rs` | Remove `pub mod reader;`. Add `pub mod mempool_editor_page;`. |
| `src/components/renderer_page.rs` | Mount the edit-affordance link from §12 inside the existing chrome. Hide it for non-author or non-mempool paths via `<Show when=...>`. |

`apply_commit_outcome` (currently `pub(super) async fn` at `compose.rs:337`) stays in `compose.rs` and remains available to `editor.rs` via the existing `super::` visibility (both are in the same `mempool` module).

## 5. Routing

`router.rs` gains two branches before the FS-resolved arm. The `is_*` checks live in `core/engine/routing.rs` (per the project's existing convention of keeping path-shape helpers off the component) and are unit-testable without a Leptos runtime:

```rust
// in core/engine/routing.rs
pub fn is_new_request_path(req: &RouteRequest) -> bool {
    req.url_path.trim_matches('/') == "new"
}

pub fn edit_request_path_inner(req: &RouteRequest) -> Option<&str> {
    req.url_path.strip_prefix("/edit/")
}
```

```rust
// in router.rs RouterView body, before the existing FS-resolved match:
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

`MempoolEditorPageMode::Edit { request_path: String }` carries the URL-extracted suffix (e.g., `mempool/writing/foo`). `MempoolEditorPage` resolves it to a canonical `VirtualPath` via `fs.resolve_route` and runs the §8 acceptance checks before fetching the body.

## 6. Save / Cancel Flow

```
NEW (/#/new)
  user submits form
  → MempoolEditor calls save_compose(ctx, ComposeMode::New, form)
  → save_compose validates → builds ChangeSet → commits → reload_runtime → apply_runtime_load
  → on Ok: derive saved_path = target_path(&form); on_saved(saved_path) fires
  → MempoolEditorPage calls push_request_path(&content_route_for_path(saved_path.as_str()))
  → router resolves new URL → RendererPage mounts → fetches content → user sees the new entry

NEW cancel
  → on_cancel fires → MempoolEditorPage calls push_request_path("/ledger")

EDIT (/#/edit/mempool/writing/foo)
  → MempoolEditorPage resolves request_path → VirtualPath via fs.resolve_route(...)?.node_path
  → fetches body via ctx.read_text(&path).await (wrapped in LocalResource for Suspense)
  → builds ComposeMode::Edit { path, body } and seeds MempoolEditor's form
  user submits form
  → save_compose(ctx, ComposeMode::Edit { path, body: source }, form)
  → as above; on Ok: on_saved(path) where path is the existing canonical path
  → MempoolEditorPage calls push_request_path(&content_route_for_path(path.as_str()))

EDIT cancel
  → on_cancel fires → MempoolEditorPage calls push_request_path(&content_route_for_path(path.as_str()))
```

Navigation uses the existing `core::engine::routing::push_request_path` helper (which under the hood calls `dom::set_hash`). The non-author redirects in §7 use `replace_request_path` (after the §7.1 fix) so the back button does not re-enter the editor route in a loop.

**Reactive ordering correctness.** `save_compose` awaits `crate::core::runtime::reload_runtime()` and then synchronously calls `ctx.apply_runtime_load(load)` (which `set`s `global_fs` and friends) before returning `Ok(())`. `on_saved` runs after the `Ok`, so by the time `MempoolEditorPage` calls `push_request_path`, `view_global_fs` already reflects the new state. The route memo's next read sees the updated FS and the destination URL resolves cleanly.

## 7. Author-Mode Gating

`author_mode = ctx.runtime_state.with(|rs| rs.github_token_present)` — synchronously available post-boot, same source as `LedgerPage` today.

`MempoolEditorPage` gates twice (defense in depth):

**Render-time gate** — the editor body is wrapped in `<Show when=author_mode fallback=...>`. The fallback is a brief `<Suspense>`-shaped placeholder ("redirecting…"). Non-author users *never* paint the form for even one frame.

**Effect-based redirect** — fires at most once per route entry:

```rust
Effect::new(move |_| {
    if !author_mode.get() {
        let target = match mode {
            MempoolEditorPageMode::New => "/ledger".to_string(),
            MempoolEditorPageMode::Edit { ref request_path } =>
                content_route_for_path(request_path),
        };
        replace_request_path(&target);
    }
});
```

Using `replace_request_path` (not `push`) means the back button doesn't trap the user in `view → /edit → redirect-back-to-view → /edit again`.

**Why both gates?** The render-time `Show` prevents flicker. The Effect actually changes the URL. Doing only the Effect would render the form for one frame; doing only the `Show` would leave the user stuck on a "redirecting…" placeholder.

The `+ compose` link in `Mempool` is still gated on `author_mode` (existing `<Show when=...>`). Non-authors don't see the link, so the only paths to `/#/new` and `/#/edit/...` are URL typing and bookmarks — both correctly hit the redirect.

### 7.1 `replace_request_path` must dispatch a synthetic `hashchange` event

The current `replace_request_path` (`/Users/wonj/Projects/websh/src/core/engine/routing.rs:125-126`) calls `dom::replace_hash`, which calls `history.replace_state_with_url`. Per the HTML spec, `replace_state_with_url` does **not** fire `hashchange`. The router (`/Users/wonj/Projects/websh/src/components/router.rs:55-65`) only updates `_raw_request` on `hashchange`. Result if unmodified: the URL bar reads `/ledger`, but `MempoolEditorPage` stays mounted, the `<Show>` keeps showing "redirecting…", and the user is stuck.

**Fix (mandatory pre-implementation):** modify `replace_request_path` (or `dom::replace_hash`) to dispatch a synthetic `hashchange` event after `replace_state_with_url`:

```rust
pub fn replace_request_path(path: &str) {
    dom::replace_hash(&format!("#{}", normalize_request_path(path)));
    dom::dispatch_hashchange();
}

// new helper in src/utils/dom.rs:
pub fn dispatch_hashchange() {
    if let Some(window) = window() {
        if let Ok(event) = web_sys::Event::new("hashchange") {
            let _ = window.dispatch_event(&event);
        }
    }
}
```

Backwards compatibility: the only existing caller of `replace_request_path` is `RouteRequest::replace()`, which is only invoked from `terminal/shell.rs:112` for shell-surface URL normalization (rewriting an already-resolved request to its canonical shape). Adding a synthetic `hashchange` causes the route memo to re-run with the canonical url_path, which `resolve_route` resolves to the same `Shell` frame — no behavioral change other than `_raw_request` matching the URL bar (a strict improvement). The `if frame.request.url_path != canonical` guard ensures the Effect does not re-fire after normalization, so no infinite loop.

**Why not `push_request_path` instead?** Push pollutes history. The user's back-button sequence after `/foo → /new (auth fails) → /ledger (push)` becomes a `/new ↔ /ledger` oscillation when pressing back, since each /new mount fires its redirect. Replace + dispatch is the only correct shape.

## 8. Loading and Error States

`MempoolEditorPage` (Edit mode) goes through these checks in order, each producing a render branch. The first three checks are URL-shape checks (sync, no fs read); the remaining three depend on resolved fs state. The body fetch is wrapped in `LocalResource::new(...)` to integrate with `<Suspense>`, matching the `RendererPage` pattern (`/Users/wonj/Projects/websh/src/components/renderer_page.rs:42`).

| Check | Failure render |
|---|---|
| Author-mode | (handled in §7 — render `<Show fallback>` + redirect) |
| `request_path` (the URL suffix after `/edit/`) starts with `mempool/` | Error frame: "this URL is not editable — only `/mempool/...` paths can be edited." Link: back to `/#/ledger`. |
| `fs.resolve_route(&request)` returns `Some(_)` | Error frame: "no such mempool entry: `/mempool/...`." Link: back to `/#/ledger`. |
| Resolved kind is `ResolvedKind::Page` or `ResolvedKind::Document` (not `Directory` / `App` / `Asset` / `Redirect`) | Error frame: "this is not a markdown entry." Link: back to view URL (`content_route_for_path` of resolved canonical path). |
| Path ends in `.md` (markdown only — mempool entries are markdown by design) | Same error frame as above. |
| `ctx.read_text(&path).await` succeeds | Error frame: "could not load source: `<message>`." Link: back to view URL. Logged via `leptos::logging::warn!`. |
| Body parses (frontmatter is optional; absence is fine) | (no failure — `parse_mempool_frontmatter` returns `Default::default()` on missing) |

While the body fetch is pending: `<Suspense>` fallback shows "Loading editor…" — same loading shell pattern as `RendererPage`.

`MempoolEditorPage` (New mode) has no async dependency; it renders the form immediately after the §7 author-mode gate.

## 9. Decision Log Additions

To append to the master plan §10 at phase-completion time. Long-form entries match the surrounding style (Phase 5's row was authored after live-QA):

| Date | Decision | Reference |
|---|---|---|
| 2026-04-29 | A5 dropped — mempool items use URL navigation; mempool paths exposed in URL bar. Rationale: original A5 hid mempool paths to avoid URL exposure, but A1 already declares the mempool repo public, so the privacy framing was incoherent. URL-driven flows enable bookmarking, refresh-during-edit, and browser-back from edit. Anchor edited inline; entry preserved in §10 for future archaeology. | §3 A5 |
| 2026-04-29 | Phase 6 — modal-free authoring. Three URL-distinct flows replace the two modals: `/#/<path>` (view, unchanged), `/#/edit/<path>` (edit), `/#/new` (compose). `MempoolEditorPage` (router-mounted) hosts both edit and new; `MempoolEditor` (inner) is the un-modal'd compose form. `Reader` deleted (only consumer was `MempoolPreviewModal`); `MempoolPreviewModal` deleted; `ComposeModal` component deleted (helpers retained). Edit affordance from a viewed mempool entry surfaces via the existing `SiteChromeActions` slot on `RendererPage`, gated on author-mode + `/mempool/` path. Net diff: ~ -550 lines (Reader is the bulk). Reviewer findings closed pre-implementation: design pass (4 HIGH + 4 MEDIUM + 4 NIT). | §4 |

## 10. Open Questions / Out of Scope

- **Refresh button on view page after manual mempool changes elsewhere.** Out of scope; `LocalResource` re-fetches on FS state change which already happens after compose-save in this session. Cross-session staleness is the same problem as for canonical content and not specific to this phase.
- **Mempool body fetch path** (raw.githubusercontent.com vs authenticated `api.github.com/contents`). Pre-existing quirk noted in master plan; not addressed here.
- **`modified:` vs `date:` frontmatter divergence.** Pre-existing; promote-time concern, not editor concern.

## 11. Verification Plan

After implementation:

- `cargo check --target wasm32-unknown-unknown --lib`
- `cargo test --lib` (existing compose helper tests at `compose.rs:695–922` still pass intact; existing `tests/mempool_compose.rs` is helper-level — verified — and survives. Add unit tests for `is_new_request_path`, `edit_request_path_inner`, and any pure helpers extracted from `MempoolEditorPage` (e.g., the §8 acceptance check.))
- `cargo build --bin websh-cli` (no expected change, sanity)
- `trunk build --release` succeeds; `trunk serve` runs.
- Manual browser walkthrough:
  1. As non-author: click mempool item → URL changes to `/#/<path>` → renders in `RendererPage` (view).
  2. As non-author: visit `/#/new` → redirected to `/#/ledger` (back button does not return to `/#/new`).
  3. As non-author: visit `/#/edit/...` → redirected to view URL.
  4. As author: click mempool item → URL changes → view in `RendererPage`. Edit affordance visible in chrome.
  5. As author: from view, click "edit" → `/#/edit/<path>` → form seeded → edit → save → navigates to view URL → renders updated entry.
  6. As author: `+ compose` → `/#/new` → empty form with category dropdown → fill → save → URL changes to view URL → renders saved entry.
  7. As author: visit `/#/edit/<path>` directly → form seeded from source → edit → save → navigates to view URL → renders updated entry.
  8. Cancel from `/#/new` → navigates to `/#/ledger`.
  9. Cancel from `/#/edit/...` → navigates to view URL.
  10. Browser back from `/#/edit/...` after entering it → back to previous URL.
  11. As author, visit `/#/edit/papers/foo` (canonical, not mempool) → §8 rejection frame.
  12. As author, visit `/#/edit/mempool/writing/does-not-exist` → §8 rejection frame.
  13. As author, visit `/#/edit/mempool/writing` (directory, no slug) → §8 rejection frame.
  14. With page mounted at `/#/new` (Effect redirect already fired), confirm session is interactive on the redirected page (no stuck "redirecting…" placeholder); covers §7.1 hashchange dispatch.
  15. Author toggles token mid-session: visit `/#/new` while logged in (form renders), then close auth (token cleared) elsewhere, then visit `/#/new` again → redirect fires; covers §7's `<Show>` + Effect interaction across sessions.

## 12. Edit Affordance on `RendererPage`

The author needs an entry point from view → edit. The mempool list's items are now `<a href={view URL}>` (no edit context); the chain blocks have nothing to do with mempool. So the only place to surface "edit" is the view page itself.

**Decision:** mount a single `<a>` in the existing `SiteChromeActions` slot (`src/components/chrome/mod.rs:187`). This slot exists *for* page-action affordances; using it is filling a slot, not adding chrome.

### 12.1 `SiteChrome` API extension (mandatory)

The current `SiteChrome` (`/Users/wonj/Projects/websh/src/components/chrome/mod.rs:65-128`) takes only `route: Memo<RouteFrame>` — it does not accept children, and `SiteChromeActions` is hardcoded inside the component to nav + palette-picker. Phase 6 must extend the signature with an optional extra-actions slot:

```rust
#[component]
pub fn SiteChrome(
    route: Memo<RouteFrame>,
    #[prop(optional, into)] extra_actions: Option<ChildrenFn>,
) -> impl IntoView {
    // ...
    <SiteChromeActions>
        <SiteChromeNav>...</SiteChromeNav>
        <SiteChromeDivider />
        <SiteChromePalettePicker theme=theme />
        {extra_actions.map(|c| c())}
    </SiteChromeActions>
    // ...
}
```

`ChildrenFn` (clonable) is required because `SiteChromeActions` may rerun under reactive updates. Existing call sites (LedgerPage, RendererPage's existing mount, HomePage, Shell) keep working unchanged — `extra_actions` defaults to `None`.

### 12.2 Edit-link mount in `RendererPage`

Pseudocode:

```rust
let canonical_path = Memo::new(move |_| route.get().resolution.node_path.clone());
let author_mode = Memo::new(move |_|
    ctx.runtime_state.with(|rs| rs.github_token_present));
let edit_visible = Memo::new(move |_|
    author_mode.get() && canonical_path.get().as_str().starts_with("/mempool/"));
let edit_href = Memo::new(move |_|
    format!("/#/edit{}", canonical_path.get().as_str()));

let extra_actions: ChildrenFn = ChildrenFn::from(move || view! {
    <Show when=move || edit_visible.get()>
        <a
            href=move || edit_href.get()
            class=css::editLink
            aria-label="Edit this mempool entry"
        >"edit"</a>
    </Show>
}.into_any());

view! {
    <div class=css::surface>
        <SiteChrome route=route extra_actions=extra_actions />
        // ... existing main / footer
    </div>
}
```

`css::editLink` style: small monospace link, color `var(--ledger-accent)`, hover underline — match the `+ compose` button's compact styling. Define in `renderer_page.module.css`.

## 13. Scope Estimate

| Bucket | Lines |
|---|---|
| Deletions | `preview.rs` (~95), `preview.module.css` (~35), `reader/mod.rs` (442), `reader/reader.module.css` (520), `pub mod reader;` (1), `ComposeModal` body (~315), pub-uses (2), `ReaderMode` references (~10) | ~1420 |
| Additions | `mempool/editor.rs` (~280), `mempool/editor.module.css` (~150), `mempool_editor_page.rs` (~180), `routing.rs` helpers + tests (~40), `router.rs` patches (~30), `RendererPage` edit-link (~25), `Mempool` `<a>` rewiring (~15), `LedgerPage` cleanup (-60 net delete), test additions (~50) | ~770 |
| Net | | ~ -650 |

Test re-points: `tests/mempool_compose.rs` is helper-only (no `ComposeModal` references — verified) and survives. `tests/mempool_model.rs` tests the model — unaffected. `tests/e2e/mempool.spec.js` may need to be updated for the new URL flows; treat as a follow-up if existing assertions reference modal selectors.
