# Phase 7 — Implementation Plan

**Design:** [`docs/superpowers/specs/2026-04-29-mempool-phase7-design.md`](../specs/2026-04-29-mempool-phase7-design.md) (v3)
**Date:** 2026-04-29

Single phase, executed in **six ordered steps** with one commit each. Each step keeps `cargo check --target wasm32-unknown-unknown --lib` and `cargo test --lib` green.

(Plan v2 — split former Step 3 into 3a/3b per plan-reviewer pass; tightened collision-check ownership; added belt-and-suspenders `LocalResource::refetch()` to the post-save invalidation; promoted `.editLink` CSS deletion to its own checklist item; added two risks the reviewer flagged.)

---

## Step 1 — `RawMempoolMeta.category` extension

**File:** `src/components/mempool/parse.rs`

- Add `pub category: Option<String>` to `RawMempoolMeta` (line 117-124 area).
- In `parse_mempool_frontmatter`'s match (line 145-151 area), add `"category" => meta.category = Some(value.to_string()),`.
- Tests:
  - Extend `parses_full_frontmatter` (line 184) to include `category: writing` in the input and assert `meta.category.as_deref() == Some("writing")`.
  - New test `parses_category_when_present` covering minimal frontmatter with `category` only.
  - New test `category_absent_returns_none` covering frontmatter with no `category` key.

**Verification:** `cargo test --lib parse` (existing parse tests pass + 2 new); `cargo check --target wasm32-unknown-unknown --lib` clean.

**Commit:** `feat(mempool): RawMempoolMeta carries category from frontmatter`

---

## Step 2 — Pure helpers: `save_raw`, `placeholder_frontmatter`, `derive_new_path`

**File:** `src/components/mempool/compose.rs`

- Add `pub async fn save_raw(ctx: AppContext, path: VirtualPath, body: String, message: String, is_new: bool) -> Result<(), String>` that:
  - Resolves backend via `ctx.backend_for_mount_root(&mempool_root())` (matching `save_compose`'s backend resolution).
  - Resolves token via `github_token_for_commit()`.
  - Resolves `expected_head` via `ctx.remote_head_for_path(&mempool_root())`.
  - Builds `ChangeSet` with `ChangeType::CreateFile { content: body, meta: FileMetadata::default() }` if `is_new`, else `ChangeType::UpdateFile { content: body, description: None }`.
  - For `is_new`, runs the collision check (`ctx.view_global_fs.with_untracked(|fs| fs.exists(&path))` → "draft already exists at <path>"). **`save_raw` is the *only* place this check lives**; the page does not duplicate it. Mirrors the design-§6 split: pure helper handles the I/O contract, page surfaces the error string in `save_error`.
  - Calls `commit_backend(...)`, on error returns `Err(error.to_string())`.
  - Calls `apply_commit_outcome(&ctx, &mempool_root(), &outcome).await`.
  - Calls `reload_runtime().await` and `apply_runtime_load`; reload errors log via `leptos::logging::warn!` but do not poison success.
  - Returns `Ok(())`.

**File:** new `src/components/mempool/draft.rs` (new module — keeps `compose.rs` focused on form-based path)

Module purpose: pure helpers for raw-markdown new-draft path derivation. Lives next to `compose.rs` so the module shape stays clean.

```rust
//! Pure helpers for the /new raw-markdown compose flow.

use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::models::VirtualPath;

use super::parse::parse_mempool_frontmatter;
use super::serialize::slug_from_title;

const TITLE_RESERVED: &[char] = &['"', '\\', '\n', '\r', ':'];

/// YAML frontmatter placeholder for the /new compose flow. The `today`
/// argument is injected so unit tests are deterministic; the page passes
/// `format_date_iso(current_timestamp() / 1000)`.
pub fn placeholder_frontmatter(today: &str) -> String {
    let category = LEDGER_CATEGORIES[0]; // "writing"
    format!(
        "---\n\
         title: \"\"\n\
         category: {category}\n\
         status: draft\n\
         modified: {today}\n\
         ---\n\n"
    )
}

/// Parse `raw_body`'s frontmatter and derive the canonical save path for a
/// new mempool draft. Returns the human-readable error string the page
/// surfaces in `save_error`. Contract: title required, no `TITLE_RESERVED`
/// chars; category required and ∈ `LEDGER_CATEGORIES`; explicit `slug:`
/// is ignored (slug is derived from title).
pub fn derive_new_path(raw_body: &str) -> Result<VirtualPath, String> {
    let meta = parse_mempool_frontmatter(raw_body)
        .ok_or_else(|| "frontmatter is missing the leading `---` fence".to_string())?;
    let title = meta
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "title is required".to_string())?;
    if title.chars().any(|c| TITLE_RESERVED.contains(&c)) {
        return Err("title cannot contain \" \\ : or newlines".to_string());
    }
    let category = meta
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "category is required".to_string())?;
    if !LEDGER_CATEGORIES.contains(&category) {
        return Err(format!(
            "category must be one of: {}",
            LEDGER_CATEGORIES.join(", ")
        ));
    }
    let slug = slug_from_title(title);
    if slug.is_empty() {
        return Err("title must produce a non-empty slug".to_string());
    }
    VirtualPath::from_absolute(format!("/mempool/{category}/{slug}.md"))
        .map_err(|error| format!("cannot build path: {error}"))
}
```

**File:** `src/components/mempool/mod.rs`

- Add `mod draft;`
- Add `pub use draft::{derive_new_path, placeholder_frontmatter};`
- Add `pub use compose::save_raw;`

Tests (in `draft.rs` `#[cfg(test)] mod tests`):
- Happy path with title + category → expected `/mempool/writing/foo.md`.
- Reject empty title.
- Reject title with each `TITLE_RESERVED` char.
- Reject missing category.
- Reject category not in `LEDGER_CATEGORIES` (e.g., "blog").
- Reject malformed frontmatter (no `---` fence).
- `slug:` key in frontmatter ignored — derived slug differs.
- `placeholder_frontmatter("2026-04-29")` round-trips through `parse_mempool_frontmatter` and yields `category == LEDGER_CATEGORIES[0]`.

**Verification:** `cargo test --lib draft` passes new tests; `cargo check --target wasm32-unknown-unknown --lib` clean. `save_raw` has no UI caller yet — Step 3 adds it.

**Commit:** `feat(mempool): save_raw, derive_new_path, placeholder_frontmatter helpers`

---

## Step 3a — `RendererPage` view/edit toggle (existing mempool entries)

Scope: add Edit/View toggle for existing mempool paths only. `/new` continues to mount `MempoolEditorPage` (unchanged in 3a — migrated in 3b). Step 3a is verifiable in isolation: existing mempool entries can be edited; non-mempool paths unaffected.

**File:** `src/components/renderer_page.rs`

- Add module-level `enum ReaderMode { View, Edit }` (derive `Clone, Copy, Debug, PartialEq, Eq`).
- Inside `RendererPage`:
  - `let mode = RwSignal::new(ReaderMode::View);`
  - `let draft_body = RwSignal::new(String::new());`
  - `let save_error = RwSignal::new(None::<String>);`
  - `let saving = RwSignal::new(false);`
  - `let refetch_epoch = RwSignal::new(0u32);`
  - **Path-change reset:** `Effect::new(move |_| { let _ = canonical_path.get(); mode.set(ReaderMode::View); save_error.set(None); });`. Defensive against any case where the page is *not* re-mounted across path navigation (Leptos's `into_any()` boundary makes identity-keep possible).
  - LocalResource source closure: `move || { let frame = route.get(); let _ = refetch_epoch.get(); ... }`. The `let _ = refetch_epoch.get()` registers the source closure as a subscriber so post-save bumps trigger a re-fetch. As belt-and-suspenders, the save success branch *also* calls `content.refetch()` directly — see Save handler below.
  - `loaded_body = Signal::derive(move || match resource.get() { Some(Ok(RendererContent::Html(rendered))) => rendered.source.clone(), ... _ => String::new() })`. The exact extraction depends on `RendererContent`'s shape; for markdown the raw source is read inside `load_renderer_content` and rendered to HTML before discarding the source. **Adjustment:** `RendererContent::Html` currently carries `RenderedMarkdown` (no raw source). Step 3a needs to either (a) keep a parallel `raw_source: Signal<String>` from a second `LocalResource` that calls `read_text` on canonical_path, or (b) thread the raw source through `RendererContent`. Option (a) is the smaller diff: add `let raw_source = LocalResource::new(...);` reading `read_text(&canonical_path.get()).await` for markdown paths only.
  - On toggle to Edit (existing): `draft_body.set(raw_source.get().unwrap_or_default()); mode.set(Edit);`.
  - Save handler (existing only — `/new` branch added in 3b):
    ```rust
    let on_save = move |_| {
        if saving.get_untracked() { return; }
        saving.set(true);
        let body = draft_body.get_untracked();
        let path = canonical_path.get_untracked();
        let rel = path.as_str().trim_start_matches("/mempool/").trim_end_matches(".md");
        let message = format!("mempool: edit {rel}");
        let ctx_clone = ctx.clone();
        spawn_local(async move {
            let result = save_raw(ctx_clone, path, body, message, false).await;
            saving.set(false);
            match result {
                Ok(()) => {
                    save_error.set(None);
                    mode.set(ReaderMode::View);
                    refetch_epoch.update(|n| *n += 1);
                    content.refetch();  // belt-and-suspenders
                }
                Err(message) => save_error.set(Some(message)),
            }
        });
    };
    ```
  - Cancel handler (existing only): `draft_body.set(raw_source.get().unwrap_or_default()); mode.set(View); save_error.set(None);`.
  - Build `extra_actions` `ChildrenFn`:
    ```rust
    let edit_visible = Memo::new(move |_| {
        author_mode.get() && canonical_path.get().as_str().starts_with("/mempool/")
    });
    let extra_actions: ChildrenFn = Arc::new(move || {
        view! {
            <Show when=move || mode.get() == ReaderMode::View && edit_visible.get()>
                <button class=css::editButton on:click=on_toggle_edit>"edit"</button>
            </Show>
            <Show when=move || mode.get() == ReaderMode::Edit>
                <button class=css::cancelButton on:click=on_cancel
                    prop:disabled=move || saving.get()>"cancel"</button>
                <button class=css::saveButton on:click=on_save
                    prop:disabled=move || saving.get()>
                    {move || if saving.get() { "Saving…" } else { "Save" }}
                </button>
            </Show>
        }
        .into_any()
    });
    ```
  - View body branches on `mode`:
    - `ReaderMode::View`: existing rendered output (unchanged from current `RendererPage`).
    - `ReaderMode::Edit`: `<textarea class=css::editorTextarea prop:value=move || draft_body.get() on:input=move |ev| draft_body.set(event_target_value(&ev))>` plus `save_error` rendered as a banner above.

**File:** `src/components/renderer_page.module.css`

- **Delete** `.editLink`, `.editLink:hover`, `.editLink:focus-visible` (lines 203-223 in the existing module).
- Add `.editButton`, `.saveButton`, `.cancelButton` (transparent backgrounds, monospace, accent color for save). Add `.editorTextarea` (full-width, min-height ~60vh, monospace), `.errorBanner` (red border, padding, top of main).

**Verification:** `cargo check --target wasm32-unknown-unknown --lib`; `cargo test --lib`. **Live walk-through (proves refetch_epoch works):** edit an existing mempool entry → save → mode flips to View, just-saved bytes render. Toggle Edit again → textarea seeded with just-saved bytes (not pre-save). If View shows stale bytes after save, refetch_epoch tracking is broken; abort and investigate.

**Commit:** `feat(mempool): RendererPage view/edit toggle for existing mempool entries`

---

## Step 3b — `/new` compose flow

Scope: switch `/new` from `MempoolEditorPage` to `RendererPage`; add the new-draft short-circuit, placeholder seed, redirect Effect, save handler's `is_new` branch.

**File:** `src/components/router.rs` — change the `/new` branch to mount `RendererPage` with the synthetic `RouteFrame` from design §4 (not `MempoolEditorPage`). `/edit/<rest>` branch stays for now (removed in Step 4).

```rust
if _raw_request.with(is_new_request_path) {
    return view! {
        <RendererPage route=Memo::new(move |_| new_compose_frame()) />
    }.into_any();
}
```

Add `fn new_compose_frame() -> RouteFrame { /* synthetic frame per design §4 */ }` adjacent to `ledger_filter_frame`.

**File:** `src/components/renderer_page.rs`

- Compute `is_new_route = Memo::new(move |_| route.get().request.url_path == "/new")`.
- Construction-time seed (per plan-reviewer suggestion — no Effect, no StoredValue guard):
  ```rust
  let initial_draft = if is_new_route.get_untracked() {
      placeholder_frontmatter(&iso_today())
  } else {
      String::new()
  };
  let draft_body = RwSignal::new(initial_draft);
  let mode = RwSignal::new(if is_new_route.get_untracked() { ReaderMode::Edit } else { ReaderMode::View });
  ```
  (This replaces the `RwSignal::new(String::new())` and `RwSignal::new(View)` lines from 3a.)
- Update `edit_visible`:
  ```rust
  let edit_visible = Memo::new(move |_| {
      author_mode.get()
          && (canonical_path.get().as_str().starts_with("/mempool/") || is_new_route.get())
  });
  ```
- Author-mode redirect Effect:
  ```rust
  Effect::new(move |_| {
      if is_new_route.get() && !author_mode.get() {
          replace_request_path("/ledger");
      }
  });
  ```
- Skip the LocalResource fetch for `/new`: in the source closure, early-return a sentinel (`if is_new_route.get() { return None; }` and adjust the resource type to `Option<Result<RendererContent, String>>`); or simpler, keep the resource shape but ignore its result in the View branch when `is_new_route.get()`.
- View body adjustment: `ReaderMode::View && is_new_route` → render `draft_body` through `render_markdown` (live preview of the unsaved draft, frontmatter fence visible per design).
- AttestationSigFooter: wrap in `<Show when=move || !is_new_route.get()>`.
- Save handler — extend with the `is_new` branch:
  ```rust
  if is_new_route.get_untracked() {
      let body = draft_body.get_untracked();
      match derive_new_path(&body) {
          Ok(target) => {
              let rel = target.as_str().trim_start_matches("/mempool/").trim_end_matches(".md");
              let message = format!("mempool: add {rel}");
              let ctx_clone = ctx.clone();
              saving.set(true);
              spawn_local(async move {
                  let result = save_raw(ctx_clone, target.clone(), body, message, true).await;
                  saving.set(false);
                  match result {
                      Ok(()) => {
                          save_error.set(None);
                          push_request_path(&content_route_for_path(target.as_str()));
                      }
                      Err(message) => save_error.set(Some(message)),
                  }
              });
          }
          Err(message) => save_error.set(Some(message)),
      }
      return;
  }
  ```
- Cancel handler — extend:
  ```rust
  if is_new_route.get_untracked() {
      replace_request_path("/ledger");
      return;
  }
  ```

**Verification:** `cargo check --target wasm32-unknown-unknown --lib`; `cargo test --lib`. **Live walk-through:**
- `/#/new` (author): Edit mode with placeholder frontmatter. View toggle → renders draft (frontmatter fence visible).
- `/#/new` (non-author, simulate by closing token): URL replaces to `/#/ledger`.
- `/#/new` Save with valid frontmatter → URL navigates to view URL of new entry.
- `/#/new` Save with empty title → save_error banner; stays in Edit.
- `/#/new` Save with category not in `LEDGER_CATEGORIES` → save_error banner; stays in Edit.
- `/#/new` Cancel → URL replaces to `/#/ledger`.

**Commit:** `feat(mempool): /new compose route mounts RendererPage with placeholder frontmatter`

---

## Step 4 — Router cleanup (`/edit/<path>` route)

**File:** `src/components/router.rs`

- Drop the `if let Some(rest) = ... edit_request_path_inner(...)` branch (Phase 6 §C3 addition).
- Drop `MempoolEditorPage` and `MempoolEditorPageMode::Edit` from imports (the `/new` branch was migrated to `RendererPage` in Step 3b).
- Drop `edit_request_path_inner` from imports.

**File:** `src/core/engine/routing.rs`

- Delete `pub fn edit_request_path_inner(...)`.
- Delete the two `edit_request_path_inner_*` test functions.
- `is_new_request_path` and its tests stay.

**File:** `src/core/engine/mod.rs`

- Remove `edit_request_path_inner` from the `pub use routing::{...}` re-export.

**Verification:** `cargo test --lib` (498 → 496 since 2 edit_request_path tests are removed; 2 parse.rs tests + ~8 draft.rs tests added in steps 1-2 push it back up); `cargo check --target wasm32-unknown-unknown --lib` clean.

**Commit:** `refactor(router): drop /edit/<path> route and edit_request_path_inner helper`

---

## Step 5 — Delete `MempoolEditor`, `MempoolEditorPage` + master plan + tests + docs

**File deletions:**

- `src/components/mempool/editor.rs`
- `src/components/mempool/editor.module.css`
- `src/components/mempool_editor_page.rs`
- `src/components/mempool_editor_page.module.css`

**File modifications:**

- `src/components/mempool/mod.rs` — drop `mod editor;` and `pub use editor::MempoolEditor;`.
- `src/components/mod.rs` — drop `pub mod mempool_editor_page;` and the editor doc-comment line.

**Master plan updates** (`docs/superpowers/specs/2026-04-28-mempool-master.md`):

- §3 A9: edit the list and the justification paragraph to drop `/edit/`. After: `/`, `/ledger`, `/websh`, `/explorer`, `/new`. Justification: "`/new` is claimed by the URL-driven mempool authoring flow; …".
- §4 phase table: add Phase 7 row.
- §6 document index: add Phase 7 design + plan rows.
- §10 decision log: append the entry from Phase 7 design §9.

**E2E test:** `tests/e2e/mempool.spec.js` — no change needed (Phase 6 D3 already updated for URL navigation).

**Verification:** `cargo test --lib` clean (no orphans), `cargo check --target wasm32-unknown-unknown --lib` clean. Run the design §11/§8 manual walk-through:
- View an existing mempool entry, toggle Edit → textarea shows raw markdown including frontmatter.
- Edit, hit Save → mode flips to View, just-saved bytes render (proves refetch-invalidation works).
- Toggle Edit again → `draft_body` reflects the saved bytes (proves loaded_body re-derives).
- Visit `/#/new` (author-mode) → Edit mode with placeholder frontmatter.
- Edit title/category, hit Save → URL navigates to view URL of new entry.
- Visit `/#/new` (non-author) → URL replaces to `/#/ledger`.

**Commit:** `chore(mempool): delete MempoolEditor + MempoolEditorPage, master plan Phase 7`

---

## Final reviewer pass

After Step 5: invoke `superpowers:code-reviewer` on the cumulative diff (Steps 1-5). Address HIGH/CRITICAL findings before declaring complete.

## Risk register

| Risk | Likelihood | Mitigation |
|---|---|---|
| `LocalResource` source closure doesn't pick up `refetch_epoch.get()` (signal tracking unavailable in resource source closures) | Medium — Leptos 0.8's reactive primitives generally track in source closures, but unconfirmed for `LocalResource` specifically | Belt-and-suspenders: also call `content.refetch()` directly after a successful save. Either mechanism alone forces the refetch; both together makes the refresh deterministic. Verified live in Step 3a's walk-through. |
| `RendererPage` *not* re-mounted across content-path navigation (Leptos `into_any()` boundary may keep component identity) | Medium | `Effect::new` resets `mode = View` and `save_error = None` whenever `canonical_path` changes (see Step 3a). |
| `/new` Effect re-fires on every reactive change (e.g., theme flip) | Low | Effect reads only `is_new_route` and `author_mode`; both are stable across UI flicks. |
| Construction-time seed for `/new` placeholder fires twice if `RendererPage` re-mounts | Low | Construction is per-mount; re-mount is the correct trigger for re-seeding. The path-change Effect resets `mode` but not `draft_body` — for cross-mount /new visits the seed is fresh per mount. |
| Master plan A9 paragraph rewrite missed | Medium | Step 5 lists both list and paragraph edits explicitly. |
| `content_route_for_path("/mempool/writing/foo.md")` strips the `.md` and yields the wrong navigation target | Low | `content_routes.rs:12` already strips `.md` reader extensions for the chain blocks; the new entry's view URL is the same shape as any other content path. Confirmed by reading the helper's contract. |

## Total scope estimate

| Bucket | Lines |
|---|---|
| Deletions | `mempool_editor_page.rs` (389), `mempool_editor_page.module.css` (52), `mempool/editor.rs` (298), `mempool/editor.module.css` (121), `edit_request_path_inner` + 2 tests (~22), router /edit branch (~7), `RendererPage` Phase 6 F1 edit-link (~25), `.editLink` CSS (~21) | ~935 |
| Additions | `RawMempoolMeta.category` field + parse update + 2 tests (~15), `save_raw` (~55), `draft.rs` (~110 incl. ~8 tests), `RendererPage` mode/draft/save/cancel/extra_actions/textarea/raw_source (~210), CSS rules (~55), router synthetic frame helper (~10), master plan (~20) | ~475 |
| Net | | ~ -460 |

Single-phase fits comfortably. Six commits, each <300 lines.
