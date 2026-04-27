# Mempool — Phase 2 Design (Authoring: Compose & Edit)

**Date:** 2026-04-28
**Phase:** 2 of 3
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Phase 1:** [`2026-04-28-mempool-phase1-design.md`](./2026-04-28-mempool-phase1-design.md) — landed
**Depends on:** master plan §3 (architecture anchors), Phase 1 surface (Mempool, MempoolEntry, mempool_root, MempoolPreviewModal, ReaderMode)

This document refines the master plan into Phase 2 specifics. Phase 2 ships **author-mode write capability**: when a GitHub write token is present, the user can compose a new draft entry or edit an existing one, with both flows committing back to the mempool repo via the existing Phase 3a commit infrastructure.

## 1. Scope

In:

- Author-mode detection (`runtime_state.github_token_present` signal)
- Compose button in `LedgerFilterBar` (rendered into the Phase 1 reserved slot when author mode is active)
- Compose modal: frontmatter form (title, status, priority, tags, modified) + markdown body textarea
- Edit existing draft: clicking a mempool item in author mode opens the modal pre-filled with the file's current frontmatter and body
- Save flow: build a `ChangeSet`, commit to the mempool mount via `commit_backend`, refresh the `LocalResource` so the new/updated entry appears
- Frontmatter-form validation: required fields (`title`, `status`), date format
- Token gating: if no token, the buttons do not render

Out (covered by Phase 3 or master §7):

- Promote action and two-commit transaction (Phase 3)
- Delete from mempool (Phase 3 covers it via promotion's "delete from mempool repo" arm; standalone delete deferred)
- Daemon, three-tier model, image upload (V2 — master §7)

## 2. Anchor Decisions (Phase 2 specifics)

| # | Decision | Rationale |
|---|---|---|
| P2-1 | Author-mode signal = `runtime_state.github_token_present` from existing `RuntimeStateSnapshot` | Already wired through `AppContext`; no new auth surface needed |
| P2-2 | Compose & edit share one `ComposeModal` component with two modes (`Compose`, `Edit { existing_path, existing_body }`) | Avoids duplicating the form layout; one save path |
| P2-3 | Save uses `commit_backend` directly against the mempool mount's backend | Reuses Phase 3a infrastructure; no daemon |
| P2-4 | Compose preview defaults filename to `mempool/<category>/<slug>.md` from the title field | Author can override; sensible default reduces friction |
| P2-5 | After successful save, refetch `mempool_files` `LocalResource` (don't optimistically mutate) | Simpler: round-trip ensures we see what GitHub now has, including any normalization |
| P2-6 | Edit click in author mode replaces the preview-modal click handler — preview becomes editor | Less UI surface than two separate modals; "click any draft to work on it" |

### 2.1 Explicit non-decisions

- We do **not** add a "Delete" button in Phase 2. Promotion in Phase 3 implicitly removes from the mempool repo. Standalone delete is a future iteration.
- We do **not** validate frontmatter at parse time beyond what Phase 1 already does. Form-side validation is the new layer.
- We do **not** add markdown live preview. The `Reader` is reused as a *read-after-save* preview by closing the editor, not a live one.

## 3. Author-Mode Detection

`AppContext` already exposes `runtime_state: RwSignal<RuntimeStateSnapshot>` (per `src/core/runtime/state.rs`). The snapshot has `github_token_present: bool`.

In `LedgerPage`, derive a memo:

```rust
let author_mode = Memo::new(move |_| ctx.runtime_state.with(|rs| rs.github_token_present));
```

Pass it into `LedgerFilterBar` and the mempool item handler. When `author_mode.get() == true`:
- The compose button renders into `.filterBarSlot`.
- A click on a mempool item opens the *editor* modal instead of the read-only preview modal.

When `author_mode.get() == false`:
- The compose button is hidden.
- A click on a mempool item opens the read-only preview modal (Phase 1 behavior unchanged).

## 4. Compose / Edit Modal

### 4.1 Component surface

```rust
#[component]
pub fn ComposeModal(
    open: ReadSignal<Option<ComposeMode>>,
    set_open: WriteSignal<Option<ComposeMode>>,
    on_saved: Callback<()>,        // signals LedgerPage to refetch the LocalResource
) -> impl IntoView;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeMode {
    /// Author wants to create a new draft. Default category from filter, if any.
    New { default_category: Option<String> },
    /// Author wants to edit an existing draft. Path + current body for prefill.
    Edit { path: VirtualPath, body: String },
}
```

### 4.2 Form fields

| Field | Source on Edit | Source on New | Required |
|---|---|---|---|
| `title` | parsed from frontmatter | empty | yes |
| `category` | derived from `path` | from `default_category` or first `LEDGER_CATEGORIES` entry | yes |
| `slug` | derived from `path` filename | derived from title (kebab-case, ASCII) | yes |
| `status` | parsed | `draft` | yes |
| `priority` | parsed | none (omit field) | no |
| `modified` | parsed | today (`YYYY-MM-DD`) | yes |
| `tags` | parsed | empty | no |
| `body` | passed in via `ComposeMode::Edit` | empty | yes |

Saved file path: `<category>/<slug>.md` relative to the mempool repo root, i.e. canonical path `/mempool/<category>/<slug>.md`.

### 4.3 Save flow

```
1. Build the file body: serialize frontmatter to YAML, append body.
2. Build a ChangeSet:
     - On New: ChangeSet::add(canonical_path, body_bytes)
     - On Edit: ChangeSet::edit(canonical_path, body_bytes)
3. Resolve mempool backend via ctx.backend_for_path(&mempool_root()).
4. Resolve auth token via runtime::state::github_token_for_commit().
5. Resolve expected_head via ctx.remote_head_for_path(&mempool_root()).
6. Call commit_backend(backend, mempool_root(), changes, message, expected_head, token).
7. On success: invoke on_saved (LedgerPage refetches the LocalResource).
8. On failure: keep modal open, show error banner inside modal.
```

Commit message format:
- New: `"mempool: add <category>/<slug>"`
- Edit: `"mempool: edit <category>/<slug>"`

### 4.4 Validation

- `title`: trim non-empty
- `status`: must be `draft` or `review`
- `slug`: matches `[a-z0-9][a-z0-9-]*` after generation; if user-edited, re-validate
- `modified`: passes `iso_date_prefix` validation
- `category`: in `LEDGER_CATEGORIES`

Validation errors render inline next to each field; Save button is disabled while any field fails.

## 5. UX surface

### 5.1 Compose button

Sits in `.filterBarSlot` (Phase 1 reserved). Rendered conditionally on author mode. Visible label: small text `+ compose`, accent color. On click, opens `ComposeModal` with `ComposeMode::New { default_category: filter_category_or_none }`.

### 5.2 Click on mempool item

Author mode: opens `ComposeModal` with `ComposeMode::Edit { path, body }`. The body is fetched the first time the user clicks (separate `read_text` call); show modal in a "loading" state until the fetch resolves, then populate the form.

Non-author mode: opens `MempoolPreviewModal` (Phase 1 behavior, unchanged).

### 5.3 Modal layout

Reuses the existing `editor/modal.module.css` aesthetic for consistency.

```
┌──────────────────────────────────────────────────────┐
│ × Close                            (mode: New | Edit)│
├──────────────────────────────────────────────────────┤
│ Title:    [____________________________]             │
│ Category: [writing v]  Slug: [________]              │
│ Status:   [draft v]  Priority: [— v]                 │
│ Modified: [2026-04-28]  Tags: [_______________]      │
│                                                       │
│ ┌────────────────────────────────────────────────┐  │
│ │ # Title                                         │  │
│ │                                                  │  │
│ │ Body...                                          │  │
│ │                                                  │  │
│ └────────────────────────────────────────────────┘  │
│                                                       │
│              [ Cancel ]  [ Save (Cmd+S) ]            │
└──────────────────────────────────────────────────────┘
```

Keyboard:
- `Esc` → cancel (with confirm if dirty)
- `Cmd/Ctrl+S` → save

## 6. Component tree

```
LedgerPage
├── LedgerIdentifier
├── LedgerHeader
├── LedgerFilterBar
│   └── ComposeButton (Phase 2 adds; gated on author_mode)
├── Mempool
│   └── MempoolItem (×N) — click goes to Compose-Edit if author, preview otherwise
├── LedgerChain
├── MempoolPreviewModal (Phase 1, gated on non-author mode click)
└── ComposeModal (Phase 2 adds, gated on author mode)
```

## 7. Files

### 7.1 New files

| Path | Purpose |
|---|---|
| `src/components/mempool/compose.rs` | `ComposeModal` component, `ComposeMode` enum, save handler |
| `src/components/mempool/compose.module.css` | Modal styles (reuses editor modal idiom) |
| `src/components/mempool/serialize.rs` | Frontmatter→YAML serializer, slug derivation, file body composition |
| `tests/mempool_compose.rs` | Integration tests for serialize + change-set construction |

### 7.2 Modified files

| Path | Change |
|---|---|
| `src/components/mempool/mod.rs` | Re-export `ComposeModal`, `ComposeMode` |
| `src/components/ledger_page.rs` | Add `author_mode` memo, `compose_open` signal, route mempool click by author_mode, mount ComposeModal |
| `src/components/ledger_page.module.css` | `.composeButton` style for the slot |

### 7.3 Out of scope for this phase

- Anything in `src/cli/`, `src/crypto/` — phase 2 is purely runtime
- Phase 1 component code beyond the click-routing change in `ledger_page.rs`

## 8. Test Strategy

### 8.1 Unit tests

| Function | Cases |
|---|---|
| `slug_from_title` | typical, leading/trailing spaces, special chars stripped, double-dash collapsed, falls back to `untitled` for empty/all-symbol input |
| `serialize_mempool_frontmatter` | full roundtrip with `parse_mempool_frontmatter`, omits `priority`/`tags` when empty, preserves `modified` ISO format |
| `compose_to_change_set` | New mode → ChangeSet has one `Add` for the canonical path; Edit mode → one `Edit`. Body bytes are valid UTF-8 and start with `---\n` |
| `validate_compose_form` | required-field errors, status-enum errors, slug regex errors |

### 8.2 Integration tests

`tests/mempool_compose.rs`:

- Roundtrip: build a `ComposeModal` form payload programmatically, serialize, then `parse_mempool_frontmatter` back — assert equivalence
- ChangeSet shape: New mode produces one addition at `/mempool/<category>/<slug>.md`; Edit mode produces one edit at the existing path
- Validation: known-bad inputs flagged, known-good pass

### 8.3 Visual QA (manual)

- Token-less session: compose button hidden, mempool click opens read-only preview (Phase 1 behavior preserved)
- Token-present session: compose button visible
- Compose flow: click → modal with empty form → fill → Save → modal closes → mempool list refreshes with the new entry within ~2s
- Edit flow: click an existing item → modal pre-filled → edit body → Save → mempool list refreshes; click again → see the edit reflected
- Network-failure: simulate by revoking token mid-session → Save fails → error banner inside modal, modal stays open

### 8.4 Out of V2 e2e (deferred)

End-to-end Playwright for compose/edit needs a writable test repo; deferred to Phase 3 final pass with both compose and promote covered together.

## 9. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Stale `expected_head` causes commit conflict (someone else pushed concurrently) | Surface error banner; user retries; expected race for low-traffic personal repos |
| Token expiration mid-session | Catch 401, show banner "token expired", suggest re-auth |
| Slug collision (existing file with same slug) | New mode: error if `<category>/<slug>.md` already exists in mempool tree; suggest appending `-2` |
| Large body on save (e.g., paste of long article) | Acceptable — GitHub raw can serve any reasonable size; commit infra is byte-streaming |
| YAML edge cases in frontmatter values (colons, quotes) | Use simple quote-wrap strategy on serialize; document as a known limitation |

## 10. Acceptance Criteria

Phase 2 is complete when:

1. With a GitHub token in session storage (`set_github_token` called), the compose button appears in the filter bar.
2. Composing a new entry, saving, and reloading shows the entry in the mempool with all frontmatter fields rendered.
3. Clicking an existing entry in author mode opens it in the editor pre-filled; editing and saving updates the GitHub repo.
4. Without a token, the compose button is hidden and clicks open the read-only preview (Phase 1 unchanged).
5. Validation errors prevent saving and display next to the offending field.
6. All unit + integration tests pass.
7. `superpowers:code-reviewer` agent has cleared the change with no outstanding CRITICAL or HIGH findings.

## 11. Open Questions

Resolved by anchor decisions above. Items resolved by Phase 2 plan:
- Slug regex: `^[a-z0-9][a-z0-9-]*$` (kebab-case, ASCII only)
- Default `modified`: `today` in user's local timezone, formatted `YYYY-MM-DD`
- Conflict on slug collision: blocked save with inline message + suggested alternative

Items deferred to Phase 3:
- How to handle a draft that was promoted between fetch and save: Phase 3 will add the same `expected_head` check on the promote path.
