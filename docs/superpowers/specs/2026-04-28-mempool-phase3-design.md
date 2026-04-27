# Mempool — Phase 3 Design (Promotion: Mempool → Canonical Chain)

**Date:** 2026-04-28
**Phase:** 3 of 3
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Phase 1:** [`2026-04-28-mempool-phase1-design.md`](./2026-04-28-mempool-phase1-design.md) — landed
**Phase 2:** [`2026-04-28-mempool-phase2-design.md`](./2026-04-28-mempool-phase2-design.md) — landed
**Depends on:** master plan §3 (architecture anchors), Phase 1 + Phase 2 surface (Mempool, MempoolEntry, ComposeMode, ComposeForm, ComposeModal, save_compose, mempool_root)

This document refines the master plan into Phase 3 specifics. Phase 3 ships **promotion**: a mempool entry can be moved from the mempool repo (`0xwonj/websh-mempool`) onto the canonical chain (the bundle source repo, `0xwonj/websh`) via a two-commit transaction. After a successful promotion, the user must still run `just pin` / `trunk build --release` locally to publish the new bundle to IPFS — Phase 3 surfaces that requirement as a deploy-hint banner.

## 1. Scope

In:

- A "Promote" action on each mempool entry (gated on author mode, same gate as Phase 2 compose/edit).
- Confirmation prompt: clicking Promote opens a small confirmation surface that summarizes destination path + commit messages and asks for explicit confirm.
- Two-commit transaction: (1) **add** the file to `/<category>/<slug>.md` in the bundle source mount, then (2) **delete** the same file from `/mempool/<category>/<slug>.md` in the mempool mount.
- Partial-failure handling: if step 1 fails, no mempool change is made. If step 2 fails after step 1 succeeded, surface a recovery banner that explains exactly what state the user is now in (entry exists in both repos) and gives a one-click "retry mempool delete" option.
- Deploy-hint banner that appears after the full transaction succeeds, instructing the user to run `just pin`. Persists across mempool list refresh until dismissed (or until the user navigates away from `/ledger`).
- Mempool list refresh on success (uses the same `mempool_refresh` signal Phase 2 already wired).

Out (covered by master §7):

- Daemon, three-tier display, automated `just pin`, IPFS pin invocation from browser (V2 — master §7).
- Standalone delete button (no UI affordance for deleting a mempool entry without promoting). Promotion implicitly handles the delete arm. Master §3 explicitly defers a separate Delete affordance.
- Re-rendering the bundle source's `ledger.json` from the browser. The CLI builds that artifact at deploy time; the UI does not need to touch it.

## 2. Anchor Decisions (Phase 3 specifics)

| # | Decision | Rationale |
|---|---|---|
| P3-1 | Promote target = `/<category>/<slug>.md` (bundle source mount, via the BOOTSTRAP_SITE backend at `/`) | Aligns with how `/ledger` already routes promoted blocks (`/writing/foo.md` virtual → `content/writing/foo.md` repo). No new mount, no new backend. |
| P3-2 | Two sequential `commit_backend` calls (no shared atomic transaction primitive) | The two repos are independent GitHub repos. Atomicity across distinct GitHub repos is not achievable from the browser. Sequential with explicit recovery is the honest design. |
| P3-3 | Bundle source commit happens **first**, mempool delete **second** | Adopt-then-remove keeps the entry visible somewhere if step 2 fails. The reverse (remove-then-add) would orphan the draft on partial failure. |
| P3-4 | Confirmation prompt = small inline confirm panel inside `MempoolPreviewModal`-like dialog (or a tiny dedicated `PromoteConfirmModal`) | Promotion is a high-impact action — it permanently changes the canonical chain after deploy. Explicit confirm is non-negotiable. |
| P3-5 | Deploy hint = banner rendered in `LedgerPage` between filter bar and mempool section, dismissable | Avoids hijacking the terminal. The banner is informational, not blocking. |
| P3-6 | Promote button on each `MempoolItem` (not just inside the editor modal) | Discoverable: a draft can be promoted without first opening the editor. The button is gated on author mode, just like compose. |
| P3-7 | Frontmatter-published cleanup (e.g., bumping `status: review` → omitted, or stripping mempool-only fields) is **out of scope** for V1 | The author can edit the file in the bundle source repo later if needed. V1 keeps promotion content-preserving. |
| P3-8 | Promotion does not require `iso_date_prefix` rewrite — the existing `modified` field is preserved | Simpler. The CLI's date-sort logic already accommodates frontmatter `modified` / `date` fields. |
| P3-9 | Bundle source filename collision = block + surface error before any commit | Exposing an existing canonical file at the same path would silently overwrite an already-published block. Pre-flight check via `view_global_fs` mirrors the Phase 2 `New`-mode collision check. |

### 2.1 Explicit non-decisions

- We do **not** stage the promotion through `ChangeSet` UI staging (the same way local `touch` / `edit` flow). Promotion bypasses the staging surface — the two commits go directly through `commit_backend`. Rationale: staging is a draft surface for the local user; promotion is an authored, intent-explicit action with its own confirmation gate.
- We do **not** rewrite frontmatter on promote (no `status: published` insertion). The bundle source already treats `/<category>/<slug>.md` as canonical regardless of in-file status.
- We do **not** introduce a multi-backend transaction primitive. Two `commit_backend` calls, in order, with explicit error mapping.

## 3. Architecture: Two-Commit Transaction

### 3.1 Sequence diagram

```
User clicks Promote on /mempool/writing/foo.md
        │
        ▼
PromoteConfirm modal shows: "Promote to /writing/foo.md? (irreversible without backout)"
        │
        ▼ user confirms
preflight checks ────────────────────────────────────────────────────────────────────┐
  - validate the source file exists in /mempool                                      │
  - validate /<category>/<slug>.md does NOT already exist in bundle source           │
  - validate token + both backends are configured                                    │
        │                                                                            │
        ▼ on any preflight error: abort, show error in modal                         │
                                                                                     │
commit #1 — bundle source                                                            │
   commit_backend(                                                                   │
       backend = bundle_source_backend (mount_root /),                               │
       changes = ChangeSet { CreateFile at /<category>/<slug>.md, content = body },  │
       message = "promote: add <category>/<slug>",                                   │
       expected_head = remote_head_for_path(/),                                      │
       token = github_token_for_commit(),                                            │
   )                                                                                 │
        │                                                                            │
        ├─ Err(_) ─► abort, modal shows error, no recovery banner needed             │
        │                                                                            │
        ▼ Ok(_)                                                                      │
commit #2 — mempool                                                                  │
   commit_backend(                                                                   │
       backend = mempool_backend (mount_root /mempool),                              │
       changes = ChangeSet { DeleteFile at /mempool/<category>/<slug>.md },          │
       message = "mempool: drop <category>/<slug> (promoted)",                       │
       expected_head = remote_head_for_path(/mempool),                               │
       token = github_token_for_commit(),                                            │
   )                                                                                 │
        │                                                                            │
        ├─ Err(_) ─► partial failure: bundle source has new file,                    │
        │           mempool still has the old entry. Show recovery banner with       │
        │           an explicit "Retry mempool delete" button. Terminal also logs    │
        │           the partial state.                                               │
        │                                                                            │
        ▼ Ok(_)                                                                      │
deploy hint banner appears: "Promoted. Run `just pin` to publish to IPFS."           │
mempool list refreshes (entry disappears).                                           │
canonical ledger refreshes if needed (entry will appear after `just pin` + reload).  │
        ────────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Why "add first, delete second"

If step 2 (mempool delete) fails after step 1 (bundle add) succeeded, the user is left with the file in **both** repos. That's a recoverable state:

- Reload: the mempool list still shows the entry, the deploy banner shows a partial-state warning, and the user can retry just the mempool delete.
- The bundle source repo has the file regardless — `just pin` will treat it as the new canonical entry.

Inverse order ("delete first, add second") would orphan the draft on partial failure (mempool already gone, bundle source has nothing). That's strictly worse: lost work.

### 3.3 Post-commit bookkeeping

Each successful `commit_backend` call returns a `CommitOutcome { new_head, committed_paths }`. Phase 2's `save_compose` discards this — a latent bug that only single-commit-per-session usage hides. Phase 3 fixes the pattern by introducing a small helper `apply_commit_outcome(ctx, mount_root, outcome)` that:

1. Updates `ctx.remote_heads` so subsequent `expected_head` lookups for the same mount return the just-committed OID.
2. Persists the new head to IDB (`remote_head.<storage_id>`) so the next session is also fresh.

After both promotion commits succeed, the promote flow additionally calls `runtime::reload_runtime()` and `apply_runtime_load`, mirroring the `terminal::sync` path. This refreshes `view_global_fs` so:

- The promoted entry shows up in the bundle source scan (visible to UI code that walks `/writing/...`).
- The mempool entry is gone from the next mempool scan (the `mempool_refresh` signal would re-load anyway, but reload_runtime is the source of truth).

We also fix `save_compose` to call `apply_commit_outcome` after its single commit. Scope creep is small and the latent bug is uncovered by the same investigation, so fixing in-place is cleaner than filing a separate ticket.

### 3.4 Recovery surface

When step 2 fails after step 1 succeeded:

1. The promote modal shows a partial-failure banner: *"Bundle commit succeeded but mempool delete failed: \<error\>. The entry exists in both repos."*
2. The modal stays open with a single button: **Retry mempool delete**.
3. Clicking retries only the second commit (no re-execution of step 1).
4. On retry success, the modal closes and the deploy hint banner appears as normal.
5. If retry continues to fail, the user can dismiss the modal; the mempool list will still show the entry, and a sticky warning banner ("entry promoted to bundle but still in mempool — manual cleanup needed") persists at the top of the mempool section until the next successful promote-or-delete.

The recovery surface uses the same `commit_backend` call. No special path — only the second arm is replayable.

## 4. UX Surface

### 4.1 Promote button

Location: each `MempoolItem` gains a small action area. The button is rendered conditionally on `author_mode.get()` (same memo Phase 2 introduced).

Visible label: `promote ↗`. Tooltip / aria-label: `"Promote to canonical chain"`. Style: subtle border + accent color when hovered. Sits next to the `modified` date in the item footer.

Click behavior: opens `PromoteConfirmModal` for that entry (does not navigate, does not open the read-only preview, does not open the editor).

Click on the surrounding `MempoolItem` continues to behave as Phase 2 specified (open editor in author mode, preview otherwise). The Promote button uses `event.stop_propagation()` so clicking it does not also fire the row click.

### 4.2 PromoteConfirmModal

A small dialog with three sections:

1. **Header:** `"Promote to canonical chain"`
2. **Body:**
   - `from: /mempool/<category>/<slug>.md`
   - `to:   /<category>/<slug>.md`
   - `commits: 2 (add + delete)`
   - status banner (idle / running / partial-failure / error)
3. **Footer:** `Cancel`, `Confirm promote` (or `Retry mempool delete` in partial-failure state).

Keyboard:
- `Esc` → cancel (only when not running).
- `Enter` (with focus on confirm) → trigger promote.

Disabled `Confirm` button while a commit is in flight.

### 4.3 Deploy-hint banner

Renders inline, between `LedgerFilterBar` and the mempool section, after a successful promote. Content:

```
✓ Promoted <category>/<slug> to canonical chain.
  Run `just pin` (then reload) to publish the new bundle to IPFS.
  [ Dismiss ]
```

Stays visible until the user dismisses it or navigates away. Multiple promotes within a single session collapse to one banner that lists the most recent slug (no queue UI).

### 4.4 Partial-failure banner

When a partial failure occurs and the user dismisses the modal without retrying, a sticky red banner replaces the deploy-hint banner:

```
⚠ <category>/<slug> was added to the canonical chain but the mempool delete
  failed: <error>. The entry still appears below until cleanup. Either retry
  via the entry's Promote button or remove it from the mempool repo manually.
  [ Dismiss ]
```

This banner persists across the same session for the affected slug. Dismissing it just hides the banner — the underlying state is unchanged.

## 5. Component Tree (Phase 3 additions)

```
LedgerPage
├── LedgerIdentifier
├── LedgerHeader
├── LedgerFilterBar (Phase 2 unchanged)
├── PromoteStatusBanner          (Phase 3 adds; renders deploy-hint or partial-failure)
├── Mempool
│   └── MempoolItem (×N)
│       └── PromoteAction        (Phase 3 adds; gated on author_mode)
├── LedgerChain
├── MempoolPreviewModal          (Phase 1, unchanged)
├── ComposeModal                 (Phase 2, unchanged)
└── PromoteConfirmModal          (Phase 3 adds)
```

## 6. Files

### 6.1 New files

| Path | Purpose |
|---|---|
| `src/components/mempool/promote.rs` | `promote_target_path`, `commit_message_*`, async `promote_entry` (preflight + commit #1 + commit #2 + recovery), `retry_mempool_delete`, `PromoteConfirmModal` component, `PromoteOutcome` enum |
| `src/components/mempool/promote.module.css` | Modal + banner styles (reuses compose.module.css idiom) |
| `tests/mempool_promote.rs` | Pure-helper integration tests: target path mapping, commit message construction, ChangeSet shapes, preflight error matrix |

### 6.2 Modified files

| Path | Change |
|---|---|
| `src/components/mempool/mod.rs` | Re-export new `promote` symbols (`PromoteConfirmModal`, `promote_target_path`, `promote_commit_messages`, `build_promote_change_sets`, `PromoteState`, etc.) |
| `src/components/mempool/component.rs` | Add `PromoteAction` button to `MempoolItem` (conditional on a new `author_mode: bool` prop on `Mempool`); `event.stop_propagation` on the button |
| `src/components/mempool/mempool.module.css` | Add `.mpActions`, `.mpPromote` styles |
| `src/components/ledger_page.rs` | Plumb `author_mode` into `Mempool`, manage `promote_target` signal, mount `PromoteConfirmModal`, render `PromoteStatusBanner`, reuse `mempool_refresh` for list invalidation |
| `src/components/ledger_page.module.css` | `.deployHint`, `.partialBanner` styles |

### 6.3 Out of scope

- `src/cli/`, `src/crypto/`, any non-runtime layer.
- `ComposeModal` keeps its current behavior; promotion is not bolted onto the editor.

## 7. Test Strategy

### 7.1 Unit tests (`compose.rs` style — colocated with `promote.rs`)

| Function | Cases |
|---|---|
| `promote_target_path` | `/mempool/writing/foo.md` → `/writing/foo.md`; rejects paths outside `/mempool`; preserves nested category (e.g., `/mempool/papers/series/foo.md` → `/papers/series/foo.md`); errors on `.md`-less inputs (currently unreachable but keeps the helper honest) |
| `promote_commit_messages` | New shape: `(add: "promote: add writing/foo", drop: "mempool: drop writing/foo (promoted)")`; survives nested categories and trims leading/trailing slashes |
| `build_bundle_add_change_set` | Produces one `CreateFile` at the bundle target path with the source body bytes verbatim; staged=true |
| `build_mempool_drop_change_set` | Produces one `DeleteFile` at the source mempool path; staged=true |
| `preflight_promote` | Returns specific errors: `MempoolEntryMissing`, `BundleTargetCollision`, `BackendMissingFor(/)`, `BackendMissingFor(/mempool)`, `TokenMissing`, `BodyReadFailed(_)`. Returns `Ok(body)` for the happy path. |

### 7.2 Integration tests (`tests/mempool_promote.rs`)

- Round-trip: build a `MempoolEntry` body, simulate `promote` payload construction, assert ChangeSet shapes and commit messages.
- Preflight matrix: each preflight error variant is reachable via a constructed `AppContext`-shaped fixture (or by passing dependencies as plain values — the helpers should take primitives where possible to keep tests free of `AppContext`).
- Path mapping: `/mempool/writing/foo.md` → `/writing/foo.md`, `/mempool/papers/q/foo.md` → `/papers/q/foo.md`.

### 7.3 Visual QA (skipped per user direction)

Visual QA is **not** performed in Phase 3 per the user's instruction. The Phase 2 visual QA was also deferred (master §10 entry, `0xwonj/websh-mempool` not yet provisioned). When the repo lands, both Phase 2 and Phase 3 will need to be exercised together: compose → promote → confirm canonical visibility post-deploy.

### 7.4 Acceptance commands

```bash
cargo test --lib
cargo test --test mempool_compose
cargo test --test mempool_promote
cargo check --target wasm32-unknown-unknown --lib
```

All four green = Phase 3 implementation acceptance bar. Reviewer agent then runs over the diff.

## 8. Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Bundle commit succeeds but mempool delete fails (partial state) | Recovery banner + retry button; never undo the bundle commit (would itself need network) |
| User's token lacks `contents:write` on the bundle source repo | First commit fails with 403; surfaced in modal; user re-auths with proper scope |
| Stale `expected_head` for either repo | Same as Phase 2 — surface 409 error; user retries |
| Slug collision in the bundle source (canonical file already exists) | Pre-flight check via `view_global_fs` rejects before any commit |
| Mempool entry was deleted (somehow) between fetch and promote | Pre-flight check via `view_global_fs` rejects before any commit |
| Two simultaneous promotes from different tabs | Acceptable race for V1 (single-user, low traffic). Worst case: second promote sees `BundleTargetCollision` or commit-side 409. |
| File body mid-promote drifts (e.g., user edits in another tab between fetch and promote) | Phase 3 promote always re-reads via `ctx.read_text(&entry.path)` immediately before committing — never trusts an in-memory MempoolEntry body |
| User runs `just pin` before retry of mempool delete | Bundle has the file → it gets published; mempool also has it → it appears in the next session's mempool too. The partial-failure banner explicitly calls this out. |
| User dismisses partial-failure banner and forgets | Banner is sticky for the session; `mempool_refresh` does not auto-clear it. Next session starts clean (banner is in-memory only). Acceptable: the mempool list itself is the persistent indicator. |

## 9. Acceptance Criteria

Phase 3 is complete when:

1. With a GitHub token in session storage, every `MempoolItem` shows a Promote button.
2. Clicking Promote opens a confirmation modal showing the canonical destination path.
3. Confirming triggers two sequential commits: bundle add, then mempool drop.
4. On full success, the deploy-hint banner appears above the mempool section, the mempool list refreshes (entry disappears), and the entry is now in the bundle source repo at `content/<category>/<slug>.md`.
5. On step 1 failure, modal shows error, both repos unchanged.
6. On step 2 failure (after step 1 succeeded), modal stays open with a Retry-mempool-delete button; if dismissed, a partial-failure banner replaces the deploy-hint surface in `LedgerPage`.
7. Pre-flight rejection surfaces clearly when (a) the mempool entry is missing, (b) the bundle source path collides, (c) either backend / the token is missing.
8. Without a token, the Promote button is hidden (Phase 2 author-mode gate continues to apply).
9. All unit + integration tests pass: `cargo test --lib`, `cargo test --test mempool_compose`, `cargo test --test mempool_promote`. Wasm typecheck passes: `cargo check --target wasm32-unknown-unknown --lib`.
10. `superpowers:code-reviewer` agent has cleared the change with no outstanding CRITICAL or HIGH findings.

## 10. Open Questions

Resolved here in this design (all derived from master §9):

- **Error UX for partial-failure**: a modal-internal recovery banner with a Retry button, plus a sticky page-level partial-failure banner if the user dismisses without retrying. (Resolves master §9 open question.)

Deferred to V2:

- Frontmatter rewrite on promote (`status: published`, etc.) — not needed for V1; bundle source treats anything in `content/<category>/` as canonical.
- Automated `just pin` invocation — explicitly out of V1 (master §7).
- Three-tier display showing "promoted but not yet deployed" between mempool and chain — out of scope per master §1 / §7.
