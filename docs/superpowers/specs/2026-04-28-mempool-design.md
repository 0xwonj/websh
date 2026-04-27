# Mempool — Design Specification

**Date:** 2026-04-28
**Status:** Draft (awaiting review)
**Scope:** `/ledger` page mempool feature, end-to-end (storage, mount, render, authoring, promotion)

## 1. Overview

Add a *mempool* to the `/ledger` page: a visible pool of pending entries (drafts, in-review work) that sits above the chain head, mirroring the blockchain mempool concept. Entries graduate from the mempool to the chain via a *promote* (mining) action.

This document is the single source of truth for the feature. It captures both the design and the rationale behind each decision so future readers can judge edge cases without rereading conversation history.

## 2. Constraints

The site is a static IPFS deployment. Three constraints shape every decision:

1. **No server.** Build, publish, and pin all happen on the author's machine at deploy time.
2. **Live updates are required.** The mempool must reflect new drafts without a fresh IPFS deploy.
3. **Bundle integrity must stay clean.** The bundled `ledger.json` is signed and pinned; the mempool is, by definition, *outside* that integrity story.

## 3. Anchor Decisions

| # | Decision | Rationale |
|---|---|---|
| 1 | Mempool is a separate GitHub repo (e.g. `0xwonj/websh-mempool`), public | Mutable across deploys; reuses existing GitHub mount infrastructure; "pending tx" is naturally public-facing |
| 2 | Mounted at `/mempool` (no `/mnt` prefix) | First-class architectural concept, single instance, clean URL/path semantics; the `/mnt` namespace remains free for future generic mounts |
| 3 | Bundle (`/site` via `BOOTSTRAP_SITE`) and mempool are *separate trees* | Preserves bundle integrity; live updates do not require rebuild |
| 4 | Frontmatter `status: draft|review` distinguishes states *within* the mempool | Visual differentiation only; mere presence in `/mempool` already implies "pending" |
| 5 | Promotion is a two-commit GitHub transaction (delete from mempool, add to bundle source) | Reuses existing Phase 3a write infrastructure; explicit deploy step preserved |
| 6 | Mempool item click → modal preview, **not** URL navigation | Avoids exposing `/mempool/...` paths in the URL bar; matches `ledger.html` interaction model where rows are action triggers |
| 7 | Local daemon deferred to V2 | Phase 3a's existing GitHub commit flow is sufficient for V1 authoring |

### 3.1 Decisions explicitly *not* taken (and why)

- **Merging GitHub mount into bundle namespace** (e.g., serving `/writing/foo.md` from either bundle or GitHub depending on availability). Rejected: would muddy `ledger.json` integrity; requires conflict-resolution rules; out of scope for blog UX.
- **Single repo with frontmatter `status` filtering** (drafts and published in same repo, distinguished by frontmatter). Rejected: bundle build still has to *exclude* drafts, and mutable drafts in the bundle source repo pollute commit history.
- **Path-based draft directory** (e.g. `content/drafts/...`). Rejected: drafts then ride the bundle's deploy cadence and lose live-update capability.

## 4. Storage

### 4.1 Mempool repo layout

Layout mirrors `content/`:

```
websh-mempool/
├── writing/
│   ├── on-writing-slow.md
│   └── async-tasks-in-pictures.md
├── projects/
├── papers/
└── talks/
```

A repo with no entries is acceptable — mounting succeeds, the UI hides the section.

### 4.2 Mount declaration

A new file in the bundle source: `content/.websh/mounts/mempool.mount.json`

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

The runtime loader (`src/core/runtime/loader.rs::load_mount_declarations`) discovers this file on boot, parses it as `MountDeclaration`, and mounts the repo via the existing GitHub backend. **No code change is required to support `/mempool` as a mount path** — `mount_at` is already an arbitrary `VirtualPath` (verified against `is_canonical_mount_root` only).

### 4.3 Read path

For `/mempool/writing/foo.md`:

```
GET https://raw.githubusercontent.com/0xwonj/websh-mempool/main/writing/foo.md
```

Public repo, no auth required for reads. Rate limit governs cold loads.

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
| `gas` | Markdown: word count rendered as `~N,NNN words`. Binary: file size via `format_size`. |
| `desc` | First non-heading paragraph, truncated to ~140 chars |

### 5.4 Validation

- Unknown frontmatter keys are ignored, not errors (forward compatibility).
- Missing `status` → entry treated as `draft` with a warning log.
- Missing `modified` → entry treated as undated; sorts to the bottom.
- Malformed frontmatter → entry skipped, error logged.

## 6. Frontend Component Design

### 6.1 Component tree

```
LedgerPage
├── LedgerIdentifier
├── LedgerHeader
├── LedgerFilterBar
├── Mempool                ← new
│   ├── MempoolHeader      (label + count)
│   └── MempoolList
│       └── MempoolItem    (× N)
└── LedgerChain
```

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
    href: String,            // for modal-preview routing
    title: String,
    desc: String,
    status: MempoolStatus,
    priority: Option<Priority>,
    kind: String,
    category: String,
    modified: String,
    sort_key: Option<String>,
    gas: String,
    tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MempoolStatus { Draft, Review }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Priority { Low, Med, High }
```

`LedgerFilter` is reused as-is (already a category-or-all enum on the ledger page).

### 6.3 Build path

```rust
async fn load_mempool(ctx: AppContext) -> Result<MempoolArtifact, String>;
fn build_mempool_model(fs: &GlobalFs, mempool_root: &VirtualPath, filter: &LedgerFilter)
    -> MempoolModel;
```

Walks the `/mempool` subtree from `GlobalFs`, parses each file's frontmatter (via a shared helper, see §6.4), constructs `MempoolEntry` instances, sorts by `(sort_key desc, path asc)`, applies filter.

If `/mempool` mount does not exist (declaration missing or scan failed), `entries` is empty.

### 6.4 Frontmatter parsing helper

Lives co-located in `src/components/mempool/parse.rs`. Reuses ISO-date validation already implemented in `src/cli/ledger.rs::iso_date_prefix` — that helper is moved to `src/utils/format.rs` so both the (host-only) CLI and the (wasm-targeted) component can share it.

```rust
fn parse_mempool_frontmatter(body: &str) -> Option<RawMempoolMeta>;
// in utils::format (lifted from cli):
fn iso_date_prefix(value: &str) -> Option<&str>;
```

### 6.5 Item rendering

Three-column grid mirroring `ledger.html` mempool style:

```
| 92px (status) | 1fr (body) | auto (modified) |
```

**Status cell** (`MempoolStatus`):
- `Draft` → `⏳ DRAFT` (dim gray)
- `Review` → `⌛ REVIEW` (amber, blinking)

**Body cell:**
- Title with `kind-tag` prefix (reuses ledger `.kind` color palette)
- Description (first paragraph, ~140 chars max)
- Meta line: `priority: med · gas: ~3,400 words` (omit fields not present)

**Modified cell:** ISO date, right-aligned, monospace, faint color.

### 6.6 Filter integration

Filter is the same `LedgerFilter` used by the chain. Resolution:
- `LedgerFilter::All` → all mempool entries
- `LedgerFilter::Category(c)` → entries with `category == c`

Header count format:
- All filter: `MEMPOOL · 7 pending`
- Category filter (with subset): `MEMPOOL · 3 / 7 pending`
- Category filter (zero match, but mempool non-empty): `MEMPOOL · 0 / 7 pending` + empty placeholder
- Mempool entirely empty: section hidden

### 6.7 Click interaction (V1)

Click on a row → opens a modal preview using the existing `Reader` component, sourced from the mempool path. Modal close returns to the ledger page. URL bar does not change.

In Phase 2/3, the same modal grows Edit and Promote actions when author mode is active.

### 6.8 Visual placement

Between `LedgerFilterBar` and `LedgerChain`, with a small dotted connector to the chain head (mirroring the existing block-to-block connector style).

## 7. Authoring UX (Phases 2–3)

V1 ships read-only. Phases 2 and 3 add authoring on top, using the existing GitHub commit infrastructure (Phase 3a) — no daemon required.

### 7.1 Phase 2: Compose & edit

**Author-mode detection:** A session has author privileges if a write token is present in storage (already implemented via `utils::session_token_storage`).

**Compose button:** Appears in `LedgerFilterBar` (right-aligned) when author mode is active. Click → opens an editor modal preconfigured with frontmatter scaffolding:

```yaml
---
title: ""
tags: []
status: draft
modified: "<today>"
---
```

**Edit existing draft:** Click an existing mempool item in author mode → modal opens with current content; Save → GitHub commit to mempool repo (uses Phase 3a `runtime/commit.rs`).

### 7.2 Phase 3: Promote (mining)

**Promote button** appears on each mempool item in author mode. Click → confirmation modal → executes a two-commit transaction:

1. **Add commit** to bundle source repo (`0xwonj/websh`): write file to `content/<category>/<file>.md`. Frontmatter is rewritten to remove `status` and add `date: <today>`.
2. **Delete commit** to mempool repo (`0xwonj/websh-mempool`): remove the file.

If step 1 succeeds and step 2 fails: surface a clear error noting the file is duplicated; provide a manual cleanup hint. (Reverse order — delete first — is rejected because a partial state where the file exists nowhere is worse than a duplicated state.)

After promotion, the new entry is in the bundle source but **not yet deployed**. The author triggers deploy separately (`just pin` or equivalent). Until then, the mempool item is gone but the chain has not gained a block.

### 7.3 Phase 3: Deploy hint

After a successful promotion, the UI shows a small banner: "1 entry promoted, awaiting deploy. Run `just pin` to publish." No automated deploy in V1–V3 (daemon territory, V2-of-the-larger-roadmap).

## 8. Out of Scope

The following are explicitly deferred:

- Local daemon (browser ↔ localhost service)
- Three-tier display (confirmed / pending / mempool — would require live diff between bundle ledger and bundle source repo's live state)
- Automated deploy after promote
- Pre-aggregated mempool manifest (`.websh/mempool.json` generated by mempool-repo CI)
- Image/binary upload via UI
- ENS DNSLink automation (CID still requires a manual on-chain or DNS update)
- Private mempool (would require either auth-gated GitHub repo + browser token handling, or daemon)
- Mobile editing UX optimizations (works through existing GitHub flow; no special handling)

## 9. Test Strategy

### 9.1 Unit tests

| Function | Cases |
|---|---|
| `parse_mempool_frontmatter` | valid full, valid minimal, missing required, malformed YAML, unknown fields ignored |
| `iso_date_prefix` (lifted from CLI) | valid ISO, ISO with time suffix, non-ISO, empty |
| `derive_gas` | markdown word count, binary file size, empty file |
| `extract_mempool_category` | typical paths, nested paths, root-level (invalid → fallback) |
| `MempoolModel::sort` | modified desc with stable path tiebreak, undated to bottom |
| `LedgerFilter` integration | All, Category match, Category miss |

### 9.2 Integration tests

- Build `MempoolModel` from a fixture `GlobalFs` populated with a synthetic `/mempool` subtree (4–6 entries across categories with mixed dates and priorities)
- Empty mempool root: model has zero entries, no panics
- Missing mempool root entirely: graceful fallback to empty model
- Filter integration: build with `LedgerFilter::Category("writing")`, assert only writing entries returned

### 9.3 Visual QA (Playwright)

- `/ledger` route: mempool section visible above chain
- `/writing` filter: mempool filtered to writing items only, count `N / total` shown
- `/projects` filter: similar
- Empty filter result: header shows `0 / N`, empty placeholder visible
- Click row: modal preview opens with content rendered
- Modal close: returns to ledger page, URL unchanged

### 9.4 Out-of-scope tests for V1

- GitHub commit flow (Phase 2+)
- Promotion two-commit (Phase 3)

## 10. Files Touched / Added

### 10.1 New files

| Path | Purpose |
|---|---|
| `content/.websh/mounts/mempool.mount.json` | Mount declaration — picked up at runtime |
| `src/components/mempool/mod.rs` | `Mempool` Leptos component |
| `src/components/mempool/model.rs` | `MempoolModel`, `MempoolEntry`, builders |
| `src/components/mempool/parse.rs` | Frontmatter parsing, gas derivation, category extraction |
| `src/components/mempool/mempool.module.css` | Styles (mirrors `ledger.html` mempool aesthetic) |
| `tests/mempool_model.rs` | Integration tests against fixture `GlobalFs` |

### 10.2 Modified files

| Path | Change |
|---|---|
| `src/components/mod.rs` | Re-export `Mempool` |
| `src/components/ledger_page.rs` | Render `<Mempool ... />` between filter bar and chain |
| `src/utils/format.rs` | Lift `iso_date_prefix` from `src/cli/ledger.rs`; optionally add a word-count helper for gas |
| `src/cli/ledger.rs` | Replace local `iso_date_prefix` with re-export from `utils::format` |

### 10.3 Out of scope for this PR

- CLI mempool processing (`src/cli/`) — mempool is purely runtime-fetched. The only CLI touch is the `iso_date_prefix` lift mentioned above; no new behavior added.
- Anything in `src/crypto/` — mempool is outside ledger integrity.

## 11. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Mempool repo not yet created on first deploy | Medium | Mount declaration fails gracefully (scan returns empty); UI hides section |
| GitHub fetch failure at runtime | Low | Empty model fallback + console-level log; user sees just no mempool |
| Many mempool items → slow page load | Low (small N expected) | V1 scans tree once; if N > 50 becomes annoying, add aggregated manifest in V2 |
| Frontmatter schema drift | Low | Tests cover validation; unknown fields ignored |
| Public mempool exposes drafts | By design | Documented as "pending tx" semantics; private drafts are V2 daemon territory |
| Two-commit promotion partial failure (Phase 3) | Medium | Add commit first, delete second; clear error UX on partial; manual cleanup documented |
| Author misuses ENS DNSLink (CID stale) | Low | Deploy banner reminds; out of scope for automation |

## 12. Acceptance Criteria

V1 (Phase 1) is complete when:

1. Pushing a new file to `0xwonj/websh-mempool` and reloading the deployed site shows it in the mempool section *without* a redeploy.
2. Total mempool is empty → mempool section is hidden entirely. Total non-empty but current filter yields zero matches → header still renders with `0 / N` count and an empty-state placeholder is shown.
3. Status, priority, and gas render per §6.5.
4. Click on a mempool row opens the modal preview without changing the URL.
5. All unit and integration tests pass.
6. Visual QA scenarios in §9.3 pass.

## 13. Open Questions

The following are not blocking but worth confirming before plan execution:

1. **Mempool repo name**: `0xwonj/websh-mempool` proposed — confirm.
2. **Modal preview implementation**: Reuse the existing `Reader` component as-is, or a stripped variant without breadcrumbs/header? Default for V1 proposal: reuse as-is and revisit if it feels heavy.
3. **Author-mode trigger**: V1 ships read-only, but should the compose button be wired conditionally so it only appears when author mode is active in V2? Default: yes — reserve the slot but render no-op until Phase 2.

