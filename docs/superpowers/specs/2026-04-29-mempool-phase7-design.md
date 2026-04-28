# Phase 7 — Single-Component View/Edit Reader

**Status:** Design v3 (closes reviewer passes 1 & 2)
**Date:** 2026-04-29
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Supersedes:** Phase 6 D–F structures (kept Phase 6 A–C infrastructure where still load-bearing, see §5)

---

## 1. Problem

Phase 6 produced a structured compose form (`MempoolEditor`) on a dedicated page (`MempoolEditorPage`) reached via `/#/edit/<path>` and `/#/new`. The user wants HackMD-style: one component, view ↔ edit toggle, raw-markdown editing including frontmatter, URL unchanged across the toggle.

## 2. Model

Two states: **View** | **Edit**. State is local to `RendererPage`; the URL does not change when toggling.

| URL | Reader-side initial state |
|---|---|
| `/#/<path>` | View. If `path.starts_with("/mempool/")` and author-mode, an "edit" button in the chrome flips to Edit. |
| `/#/new` | Edit, with the textarea pre-filled from `placeholder_frontmatter()`. Toggle to View renders the current textarea content. |
| All other paths | View, no edit affordance. |

Edit mode renders a single `<textarea>` bound to `draft_body: RwSignal<String>`. View mode renders `draft_body` through `render_markdown` so an unsaved draft preview matches what View shows (the literal `---` frontmatter fence appears in the preview — by design; the placeholder is a visible reminder of the metadata being authored). Toggling to Edit on an existing entry copies the current source body — *including frontmatter* — into `draft_body`. Saving an existing entry leaves the URL unchanged and re-fetches; saving a new entry navigates to the resolved view URL. **Navigation away from the page (back button, chrome link, hash change) discards an in-flight Edit; there is no dirty-state guard in V1.** Navigating between two different entries via URL change resets `mode` to View and clears any draft from the previous entry.

## 3. URL surface delta vs Phase 6

- Drop `/#/edit/<path>`. The route, `edit_request_path_inner`, the `MempoolEditorPage` Edit branch, and the chrome `<a href="/#/edit/...">` are all gone.
- Keep `/#/new`. The router branches `is_new_request_path` to `RendererPage` with a new-draft hint; `MempoolEditorPage` is deleted entirely.
- A9 (reserved URL prefixes) updates: drop `/edit/` from the list. `/new` stays reserved.

## 4. Component plan

### Modify

- `src/components/renderer_page.rs` — add `mode: RwSignal<ReaderMode>` (default `View`), `draft_body: RwSignal<String>`, `save_error: RwSignal<Option<String>>`, `refetch_epoch: RwSignal<u32>` (post-save invalidation, see §6), edit toggle handler, save and cancel handlers. Edit affordance in chrome is a `<button on:click=toggle>` (URL doesn't change, anchor was wrong). The chrome `extra_actions` slot swaps content based on `mode`: in View, it renders the Edit button (gated by `edit_visible = author_mode && (path.starts_with("/mempool/") || url_path == "/new")` — the `/new` clause is load-bearing because after the user previews a `/new` draft by toggling to View, they need an Edit button to return to authoring); in Edit, it renders Save + Cancel. `save_error`, when `Some`, renders inline at the top of `<main>`. Synthetic `/new` short-circuits: the `LocalResource` source closure returns early when `url_path == "/new"` (no fetch), the AttestationSigFooter is hidden, and the page mounts directly into Edit mode with `draft_body` pre-filled from `placeholder_frontmatter(today)`. The `mode` RwSignal is created once per `RendererPage` mount and survives chrome re-renders (the `extra_actions` `ChildrenFn` captures it once); navigating between entries via URL change re-mounts the page and resets `mode` to View.
- `src/components/router.rs` — drop the `/#/edit/<rest>` branch and its imports. The `/#/new` branch builds a synthetic `RouteFrame { request: RouteRequest::new("/new"), resolution: { request_path: "/new", surface: Content, node_path: VirtualPath::root(), kind: ResolvedKind::Document, params: BTreeMap::new() }, intent: RenderIntent::DocumentReader { node_path: VirtualPath::root() } }` and mounts `RendererPage`. `RendererPage` ignores `intent` and `node_path` when `url_path == "/new"` (Edit-mode short-circuit, see above). The choice of `VirtualPath::root()` is sentinel — the page never reads it for `/new`.
- `src/core/engine/routing.rs` — drop `edit_request_path_inner` and its tests. `is_new_request_path` stays.
- `src/core/engine/mod.rs` — drop the re-export of `edit_request_path_inner`.
- `src/components/mempool/parse.rs` — extend `RawMempoolMeta` with `category: Option<String>`. The new-save path-derivation needs it (existing reads were positional via `category_for_mempool_path`, which doesn't apply to a draft that has no path yet).
- `src/components/mempool/compose.rs` — add a `pub async fn save_raw(ctx, path, raw_body, message)` helper that wraps `commit_backend` + `apply_commit_outcome` + `reload_runtime`. Existing `save_compose` (form-based) stays for the CLI `mempool add`.
- `src/components/mempool/mod.rs` — drop the `MempoolEditor` re-export and the `mod editor;` line; export `save_raw`.
- `src/components/mod.rs` — drop the `mempool_editor_page` module.
- `docs/superpowers/specs/2026-04-28-mempool-master.md` — A9 reserved-prefix list edit, Phase 7 row in §4, decision-log entry.

### Delete

- `src/components/mempool/editor.rs` + `editor.module.css`
- `src/components/mempool_editor_page.rs` + `mempool_editor_page.module.css`

### New

- `src/components/renderer_page.rs` gains a `placeholder_frontmatter(today: &str) -> String` helper and a `derive_new_path(raw_body: &str) -> Result<VirtualPath, String>` helper. Both pure, unit-tested.
  - `placeholder_frontmatter` uses `category: <LEDGER_CATEGORIES[0]>` (currently `"writing"`) so the placeholder's category and `derive_form_from_mode`'s default category stay in sync. Tests assert this constant.
  - `derive_new_path` contract: parses frontmatter via `parse_mempool_frontmatter`; requires non-empty `title` whose characters do **not** include `TITLE_RESERVED` (`"`, `\\`, `\n`, `\r`, `:` — same set `validate_form` enforces, since `parse_mempool_frontmatter`'s naive `:`-split would corrupt such titles); requires `category` ∈ `LEDGER_CATEGORIES` (closed set; not free-form); ignores any explicit `slug:` key (slug is derived from title only); returns `VirtualPath::from_absolute("/mempool/<category>/<slug_from_title(title)>.md")`. Errors are human-readable strings rendered in `save_error`.

## 5. Phase 6 retention

| Item | Status |
|---|---|
| `dispatch_hashchange` + `replace_request_path` patch | Keep — used by the `/#/new` author-mode redirect to avoid a back-button loop. |
| `is_new_request_path` | Keep. |
| `SiteChrome.extra_actions` slot | Keep — hosts the edit toggle button. |
| `Mempool` item `<a href>` swap + `+ compose` `<a href="/#/new">` | Keep. `+ compose` semantics unchanged. |
| `LedgerPage` modal cleanup | Keep. |
| `RendererPage` Phase 6 F1 edit-link `<a href="/#/edit/...">` | **Remove** — replaced by an in-page `<button on:click=toggle>`. |
| `edit_request_path_inner` | **Remove.** |
| `MempoolEditor` / `MempoolEditorPage` | **Remove.** Confirmed: the only `save_compose` callers post-Phase-7 are the CLI `mempool add` subcommand (`src/cli/mempool.rs`) and `MempoolEditor` (deleted), so `save_compose` stays for the CLI. |
| Master-plan A5-dropped, A9 reserved-prefix list | A5 stays dropped. A9 list **and** justification paragraph both edit to drop `/edit/`. |

## 6. Save flow

`save_raw` signature: `pub async fn save_raw(ctx: AppContext, path: VirtualPath, body: String, message: String, is_new: bool) -> Result<(), String>`. Mirrors `save_compose`'s shape — commit error returns `Err`; `reload_runtime` failure logs via `leptos::logging::warn!` but does not poison a successful commit. `is_new` switches the change between `ChangeType::CreateFile` and `ChangeType::UpdateFile`.

```
EXISTING EDIT (mode = Edit at /#/<existing-path>)
  user hits Save
  → save_raw(ctx, canonical_path, draft_body.get(), "mempool: edit <rel>", false).await
  → on Ok: save_error.set(None); mode = View; refetch_epoch.update(|n| *n += 1)
    [the LocalResource source closure reads refetch_epoch, so the bump forces a refetch
     against the now-updated view_global_fs — see "Refetch invalidation" below]
  → on Err: save_error.set(Some(message)); stay in Edit

CANCEL EXISTING
  → draft_body.set(loaded_body); save_error.set(None); mode = View

NEW (mode = Edit at /#/new)
  user hits Save
  → derive_new_path(draft_body.get()) → target VirtualPath (or error)
  → if collision (fs.exists(target)): save_error = "draft already exists at <path>"
  → save_raw(ctx, target, draft_body.get(), "mempool: add <rel>", true).await
  → on Ok: push_request_path(&content_route_for_path(target.as_str()))
    [push, not replace: the saved entry is a real navigation destination — back-button
     should return the user to /#/new for re-authoring, which lands them in a fresh
     Edit. Asymmetric with cancel-new below by design.]
  → on Err: save_error.set(Some(message)); stay in Edit

CANCEL NEW
  → replace_request_path("/ledger")
    [replace, not push: the cancelled draft URL is a transient compose attempt;
     the user shouldn't be able to back-button into it. Asymmetric with save-new
     above because save creates persistent state, cancel doesn't.]
```

**Refetch invalidation.** The `RendererPage` LocalResource's source closure reads `route.get()` (a `Memo<RouteFrame>`) and `refetch_epoch.get()`. After an existing-entry save, the path/kind/intent are unchanged so the route memo's `PartialEq` returns equal and won't fire — but bumping `refetch_epoch` produces a fresh source-closure run, which sees the post-save `view_global_fs` and re-fetches via `read_text`. `apply_runtime_load`'s effect on `view_global_fs` is what makes the next read return new bytes; the epoch is what triggers the read. This is the explicit-trigger pattern; the implicit "memo-chain refresh" assumed in design v2 was unsound.

`save_raw` does not validate frontmatter (raw bytes pass through). For existing-edit saves, runtime parses are forgiving and fall back to defaults if frontmatter breaks. For new saves, `derive_new_path` is the only validation gate; it runs *before* the commit so a bad frontmatter never reaches the wire.

Author-mode redirect for `/#/new`: an `Effect` inside `RendererPage` runs whenever the route memo changes. The Effect is gated by `is_new_request_path(&route.request)` — for canonical paths it's a no-op; for `/#/new` and non-author it calls `replace_request_path("/ledger")`. The Phase 6 §7.1 synthetic-hashchange dispatch ensures the router actually re-routes (without it, the URL would replace but `RendererPage` would stay mounted in Edit).

## 7. Frontmatter placeholder

```yaml
---
title: ""
category: writing
status: draft
modified: <today YYYY-MM-DD>
---

```

Plus a trailing blank line so the cursor lands in the body. User changes `title` and (if needed) `category` before saving.

## 8. Tests

- `derive_new_path` happy path (title + category → `/mempool/<category>/<slug>.md`).
- `derive_new_path` rejects missing title.
- `derive_new_path` rejects title containing any `TITLE_RESERVED` char (`"`, `\\`, `\n`, `\r`, `:`).
- `derive_new_path` rejects missing category.
- `derive_new_path` rejects category not in `LEDGER_CATEGORIES`.
- `derive_new_path` rejects malformed frontmatter (no `---` fence).
- `derive_new_path` ignores explicit `slug:` key in favor of `slug_from_title(title)`.
- `placeholder_frontmatter` round-trips through `parse_mempool_frontmatter` and yields `category == LEDGER_CATEGORIES[0]`.
- `parse.rs` tests are extended: a new test asserts `RawMempoolMeta.category` reads `"writing"` from a frontmatter that includes `category: writing`.
- Existing tests survive: `routing.rs` keeps `is_new_request_path` tests; the `edit_request_path_inner` tests are removed alongside the helper.

A live walk-through (manual, post-impl) covers the refetch-invalidation flow: edit an existing mempool entry → save → toggle Edit again → confirm `draft_body` reflects the just-saved bytes (not pre-save).

## 9. Decision-log entry (to append at phase-completion)

| Date | Decision | Reference |
|---|---|---|
| 2026-04-29 | Phase 7 — Phase 6's compose form + `/edit/<path>` route replaced with a single-component `RendererPage` view/edit toggle. URL unchanged across the toggle; raw-markdown editing including frontmatter; `/#/new` opens the same component in Edit with placeholder frontmatter. `MempoolEditor`, `MempoolEditorPage`, `edit_request_path_inner` deleted. A9 reserved-prefix list narrows by dropping `/edit/`. | §3, §4 |

## 10. Out of scope

- Live split-pane preview. The toggle flip is enough for V1.
- Frontmatter validation surfaced in the UI for existing-edit saves. Mempool's existing parser is forgiving; runtime renders fall back to defaults.
- Renaming `RendererPage` → `ReaderPage`. Cosmetic; user can request later.
