# Mempool — Phase 1 Design (Read-only)

**Date:** 2026-04-28
**Phase:** 1 of 3
**Master:** [`2026-04-28-mempool-master.md`](./2026-04-28-mempool-master.md)
**Depends on:** master plan §3 (architecture anchors) and §5 (workflow)

This document refines the master plan into Phase 1 specifics. Phase 1 ships a **read-only mempool**: the section renders pending entries fetched from a mounted GitHub repo, with category filter integration and modal preview on click. No authoring, no promotion — those are Phases 2 and 3.

## 1. Scope

In:

- Mount declaration for `0xwonj/websh-mempool` at `/mempool`
- `Mempool` Leptos component rendering pending entries above the chain
- Category filter integration (reuses `LedgerFilter`)
- Modal preview on click (reuses existing `Reader` component, default per master §9)
- Auto-derived `category`, `kind`, `gas`, `desc` from path + frontmatter + content
- Empty / missing-mount / GitHub-fetch-failure handling

Out (covered by later phases or master §7):

- Author mode signals or compose UI (Phase 2)
- Edit / save flows (Phase 2)
- Promote action and two-commit transaction (Phase 3)
- Daemon, three-tier model, image upload (V2 — master §7)

## 2. Storage Layout

The mempool repo `0xwonj/websh-mempool` mirrors `content/`:

```
websh-mempool/
├── writing/
│   ├── on-writing-slow.md
│   └── async-tasks-in-pictures.md
├── projects/
├── papers/
└── talks/
```

A repo with no entries is acceptable — mounting succeeds, the UI hides the section per §6.6.

## 3. Mount Declaration

Add a new file in the bundle source: `content/.websh/mounts/mempool.mount.json`

```json
{
  "backend": "github",
  "mount_at": "/mempool",
  "repo": "0xwonj/websh-mempool",
  "branch": "main",
  "root": "",
  "name": "mempool",
  "writable": true
}
```

The runtime loader (`src/core/runtime/loader.rs::load_mount_declarations`) discovers this file on boot, parses it as `MountDeclaration`, and mounts the repo via the existing GitHub backend. **No code change is required to support `/mempool`** — `mount_at` is already a free-form `VirtualPath` validated against `is_canonical_mount_root` only.

## 4. Read Path

For `/mempool/writing/foo.md`:

```
GET https://raw.githubusercontent.com/0xwonj/websh-mempool/main/writing/foo.md
```

Public repo, no auth required for reads. Rate limit governs cold loads. Phase 1 reads files individually via `read_text`; an aggregated manifest is V2 (master §7) if needed.

## 5. Mempool Entry Schema

### 5.1 File format

Each mempool file is markdown with YAML frontmatter:

```markdown
---
title: "On writing slow"
tags: [essay, writing-process]
status: draft
priority: med
modified: "2026-04-28"
---

# On writing slow

Body text...
```

### 5.2 Frontmatter fields

| Field | Required | Type | Notes |
|---|---|---|---|
| `title` | yes | string | Display title |
| `status` | yes | `draft \| review` | Display state in mempool |
| `modified` | yes | ISO date `YYYY-MM-DD` | Used for sort order |
| `tags` | no | string array | Optional |
| `priority` | no | `low \| med \| high` | Optional; affects display only |

### 5.3 Auto-derived fields

| Field | Derivation |
|---|---|
| `category` | First path segment under `/mempool/` (e.g., `/mempool/writing/foo.md` → `writing`) |
| `kind` | Mapped from category: `writing→writing`, `projects→project`, `papers→paper`, `talks→talk`, fallback `note` |
| `gas` | Markdown: word count rendered as `~N,NNN words`. Binary: file size via `format_size` |
| `desc` | First non-heading paragraph, truncated to ~140 chars |

### 5.4 Validation rules

- Unknown frontmatter keys are ignored (forward compatibility).
- Missing `status` → entry treated as `draft` with a console warning.
- Missing `modified` → entry treated as undated; sorts to the bottom.
- Malformed YAML frontmatter → entry skipped, error logged.
- Files outside `/mempool/<category>/...` (e.g., `/mempool/foo.md` directly) → category falls back to `misc`.

## 6. Frontend Component Design

### 6.1 Component tree

```
LedgerPage
├── LedgerIdentifier
├── LedgerHeader
├── LedgerFilterBar         ← reserves a right-aligned compose slot (no-op in Phase 1)
├── Mempool                 ← new
│   ├── MempoolHeader       (label + count)
│   └── MempoolList
│       └── MempoolItem     (× N)
└── LedgerChain
```

The compose-slot reservation in `LedgerFilterBar` is a Phase 1 cosmetic change only — the slot exists in the layout but renders nothing until Phase 2 wires the author signal.

### 6.2 Data model

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
struct MempoolModel {
    filter: LedgerFilter,
    entries: Vec<MempoolEntry>,
    total_count: usize,
    counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MempoolEntry {
    path: VirtualPath,
    title: String,
    desc: String,
    status: MempoolStatus,
    priority: Option<Priority>,
    kind: String,
    category: String,
    modified: String,
    sort_key: Option<String>,    // ISO-validated form of `modified`; None if invalid
    gas: String,                  // pre-formatted, e.g. "~3,400 words" or "12.4 KB"
    tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MempoolStatus { Draft, Review }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Priority { Low, Med, High }
```

`LedgerFilter` is reused as-is.

### 6.3 Build path

```rust
async fn load_mempool(ctx: AppContext) -> Result<Vec<MempoolEntry>, String>;

fn build_mempool_model(
    fs: &GlobalFs,
    mempool_root: &VirtualPath,
    filter: &LedgerFilter,
) -> MempoolModel;
```

Walks the `/mempool` subtree from `GlobalFs`, reads each file's body via the GitHub backend, parses frontmatter, derives auto fields, constructs `MempoolEntry`. Sorts by `(sort_key desc, path asc)`. Applies filter for the visible subset; preserves `total_count` and per-category `counts` from the unfiltered set.

If `/mempool` mount does not exist (declaration missing or scan failed), `entries` is empty and the section is hidden.

### 6.4 Frontmatter parsing

Lives in `src/components/mempool/parse.rs`. Reuses ISO-date validation already implemented in `src/cli/ledger.rs::iso_date_prefix` — that helper is moved to `src/utils/format.rs` so wasm and host targets share it.

```rust
// in src/components/mempool/parse.rs
fn parse_mempool_frontmatter(body: &str) -> Option<RawMempoolMeta>;
fn derive_gas(body_after_frontmatter: &str, byte_len: usize, is_markdown: bool) -> String;
fn extract_first_paragraph(body_after_frontmatter: &str) -> String;
fn parse_mempool_status(value: &str) -> Option<MempoolStatus>;
fn parse_priority(value: &str) -> Option<Priority>;

// in src/utils/format.rs (moved from cli/ledger.rs)
pub fn iso_date_prefix(value: &str) -> Option<&str>;
```

### 6.5 Item rendering

Three-column grid mirroring the `ledger.html` mempool aesthetic:

```
| 92px (status) | 1fr (body) | auto (modified) |
```

**Status cell** (`MempoolStatus`):

- `Draft` → `⏳ DRAFT` (dim gray)
- `Review` → `⌛ REVIEW` (amber, blinking via existing `@keyframes blink`)

**Body cell:**

- Title prefixed with a `kind-tag` (reuses ledger `.kind` color palette per category)
- Description (first paragraph, ~140 chars max)
- Meta line: `priority: med · gas: ~3,400 words` (omit fields not present)

**Modified cell:** ISO date, right-aligned, monospace, faint color.

### 6.6 Filter integration

Filter is the same `LedgerFilter` used by the chain. Resolution:

- `LedgerFilter::All` → all mempool entries
- `LedgerFilter::Category(c)` → entries with `category == c`

Header count format:

- All filter, mempool non-empty: `MEMPOOL · 7 pending`
- Category filter, subset of total: `MEMPOOL · 3 / 7 pending`
- Category filter, zero match (but mempool non-empty): `MEMPOOL · 0 / 7 pending` + empty placeholder
- Mempool entirely empty: section hidden

### 6.7 Click interaction

Click on a row → opens a modal containing the existing `Reader` component, sourced from the mempool path. Modal close returns to the ledger page. URL bar does not change.

If the breadcrumb-path leakage from `Reader` (showing `~ / mempool / writing / foo`) feels jarring during visual QA, file a follow-up to gate Reader's breadcrumb on a `mode: full | preview` prop. This is a known cosmetic risk acknowledged in master §9.

### 6.8 Visual placement

Between `LedgerFilterBar` and `LedgerChain`, with a small dotted vertical connector below the mempool box mirroring the chain's block-to-block connector style.

## 7. Test Strategy

### 7.1 Unit tests

| Function | Cases |
|---|---|
| `parse_mempool_frontmatter` | valid full, valid minimal, missing `title`, missing `status`, malformed YAML, unknown fields ignored |
| `iso_date_prefix` (lifted from CLI) | valid ISO, ISO with time suffix, non-ISO, empty, padding edge cases |
| `derive_gas` | markdown word count, binary file size, empty file, near-thousand boundary |
| `extract_first_paragraph` | leading heading, no heading, multiple paragraphs, edge truncation |
| `category_for_mempool_path` | `/mempool/writing/foo.md`, nested `/mempool/papers/series/foo.md`, root-level `/mempool/foo.md` (→ `misc`) |
| `parse_mempool_status` | `"draft"`, `"review"`, unknown → `None`, case sensitivity |
| `parse_priority` | `"low"`, `"med"`, `"high"`, unknown → `None` |

### 7.2 Integration tests

Located at `tests/mempool_model.rs`:

- Build `MempoolModel` from a fixture `GlobalFs` populated with a synthetic `/mempool` subtree (4–6 entries across categories with mixed dates, statuses, priorities)
- Empty mempool root: model has zero entries, no panics
- Missing `/mempool` mount entirely: graceful fallback to empty model
- Filter integration: build with `LedgerFilter::Category("writing")` and assert only writing entries in `entries`, but `total_count` and `counts` reflect the unfiltered set
- Sorting: assert `(modified desc, path asc)` order

### 7.3 Visual QA (Playwright)

Adds to existing `tests/e2e/`:

- `/ledger` route: mempool section visible above chain when `0xwonj/websh-mempool` has entries
- `/writing` filter: mempool filtered to writing items only, count `N / total` shown
- `/projects` filter: similar
- Empty filter result: header shows `0 / N`, empty placeholder visible
- Click row: modal preview opens with content rendered, close returns to page, URL unchanged

## 8. Files Touched

### 8.1 New files

| Path | Purpose |
|---|---|
| `content/.websh/mounts/mempool.mount.json` | Mount declaration — picked up at runtime |
| `src/components/mempool/mod.rs` | `Mempool` Leptos component + sub-components |
| `src/components/mempool/model.rs` | `MempoolModel`, `MempoolEntry`, builders |
| `src/components/mempool/parse.rs` | Frontmatter parsing, gas derivation, category extraction |
| `src/components/mempool/mempool.module.css` | Styles (mirrors `ledger.html` mempool aesthetic) |
| `tests/mempool_model.rs` | Integration tests against fixture `GlobalFs` |
| `tests/e2e/mempool.spec.js` | Playwright visual QA |

### 8.2 Modified files

| Path | Change |
|---|---|
| `src/components/mod.rs` | Re-export `Mempool` |
| `src/components/ledger_page.rs` | Render `<Mempool ... />` between filter bar and chain; reserve compose slot in `LedgerFilterBar` |
| `src/components/ledger_page.module.css` | Compose-slot CSS (no-op rendering in Phase 1, just space) |
| `src/utils/format.rs` | Lift `iso_date_prefix` from `src/cli/ledger.rs`; optionally add a word-count helper |
| `src/cli/ledger.rs` | Replace local `iso_date_prefix` with re-export from `utils::format` |

### 8.3 Out of scope for this phase

- New CLI behavior (`src/cli/`) — mempool is purely runtime-fetched.
- Anything in `src/crypto/` — mempool is outside ledger integrity.
- Author-mode signal wiring (Phase 2).
- Editor / commit code paths (Phase 2 / Phase 3).

## 9. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Mempool repo not yet created on first deploy | High (the repo doesn't exist yet) | Mount declaration fails gracefully (scan returns empty); UI hides section. Document the repo-creation step in the Phase 1 plan as a prerequisite |
| GitHub fetch failure at runtime | Low | Empty model fallback + console-level log; user sees no mempool but page still renders |
| Many mempool items → slow page load | Low (small N expected) | V1 reads files individually; if N > 50 becomes annoying, add aggregated manifest in V2 (master §7) |
| Frontmatter schema drift | Low | Tests cover validation; unknown fields ignored |
| Breadcrumb leaks `/mempool/...` path in Reader modal | Medium | Acknowledged cosmetic risk (master §9); follow-up if jarring during QA |
| Public mempool repo exposes drafts | By design | Documented in master §3.1; private drafts are V2 daemon territory |

## 10. Acceptance Criteria

Phase 1 is complete when:

1. Pushing a new file to `0xwonj/websh-mempool` and reloading the deployed site shows it in the mempool section *without* a redeploy.
2. Total mempool empty → mempool section hidden entirely. Total non-empty but current filter yields zero matches → header still renders with `0 / N` count and an empty-state placeholder.
3. Status, priority, and gas render per §6.5.
4. Click on a mempool row opens the modal preview without changing the URL.
5. All unit and integration tests pass.
6. All visual-QA scenarios in §7.3 pass.
7. `superpowers:code-reviewer` agent has cleared the change with no outstanding CRITICAL or HIGH findings (per master §5).

## 11. Pre-Phase Checklist

Before writing the plan or coding:

- [ ] Master plan §3 anchors confirmed unchanged
- [ ] `0xwonj/websh-mempool` repo exists (create it before testing the live mount)
- [ ] At least 4 seed entries pushed to the mempool repo (mixed categories, statuses, priorities) for visual QA
- [ ] User has approved this design doc
