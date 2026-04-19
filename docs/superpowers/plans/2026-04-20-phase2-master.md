# Phase 2 Master Plan (Thin)

**Status:** Active. Baseline: `phase1-complete` (merged into main as `375bf88`).

**Purpose of this document:** Track-level overview. Per-track detailed plans are written individually at the start of each track (via `superpowers:writing-plans`), not here.

---

## Goal

Address remaining HIGH / MEDIUM / LOW issues from the original code review, plus a small curated cherry-pick from the abandoned `wip/january-2026-restructure` branch. Phase 1 established two core contracts (Mount singleton, CommandResult/SideEffect); Phase 2 is polish + edge-case hardening on top of that baseline.

## Out of Scope (Deferred to Phase 3+)

- **All crypto / encryption work** — implementing ECIES, `EncryptionInfo` → real wrapped keys, etc.
- **All deployment / CSP hardening** except the one-line ENS entry (see Cherry-pick).
- **Write capability** (editor, storage backend, pending/staged overlay). The WIP Jan 2026 work stays on `wip/january-2026-restructure` for future reference.
- **Architectural rewrites** not covered by an existing HIGH/MEDIUM issue.

---

## Tracks

Dependency order matters for merging back into main. Tracks are grouped into **waves**; within a wave, tracks are independent.

### Wave 1 — Foundation adjustments (start here)

| # | Track | Issues | Files (primary) | Est. size |
|---|---|---|---|---|
| **D** | Route / FS resolve | H2 (file/dir heuristic), M8 (empty-path cd) | `models/route.rs`, `core/filesystem.rs` | Medium |
| **B** | Command filters | H4 (iterator streaming), H5 (grep regex + flags), M6 (head/tail strict parsing) | `core/commands/filters.rs`, `core/commands/mod.rs` | Medium |
| **A** | Parser | C2 (token concat + unclosed quotes), M5 (`$UNDEF` empty arg), M7 (multi-var export parsing) | `core/parser/{lexer,expand,mod}.rs` | Medium |
| **C** | Autocomplete | H8 (UTF-8 boundary panic), M12 (less/more vs `Command::names` inconsistency) | `core/autocomplete.rs` | Small |
| **P** | Cherry-pick from WIP | See below | `index.html`, `src/components/breadcrumb.rs` | Small |

### Wave 2 — UI and state (after Wave 1 merges)

| # | Track | Issues | Files (primary) | Est. size |
|---|---|---|---|---|
| **F** | Terminal render perf | H1 (VirtualFs clone in execute path), M2 (OutputLineId newtype), M3 (RingBuffer to_vec per render) | `components/terminal/terminal.rs`, `models/terminal.rs` | Medium |
| **E** | Reader race condition | H9 (Effect + spawn_local → LocalResource migration) | `components/reader/mod.rs` | Small |
| **G** | Explorer UI / a11y | H10 (debug logs cleanup), H11 (keyboard nav in FileListItem), M10 (dropdown hook dedup) | `components/explorer/{header,file_list}.rs`, new hook module | Medium |
| **H** | Navigation semantics | M1 (forward_stack vs browser history), M11 (`pop_forward` idiom) | `app.rs`, `components/explorer/header.rs`, new `router.rs` glue | Small |

### Wave 3 — Cleanup (last)

| # | Track | Issues | Files | Est. size |
|---|---|---|---|---|
| **I** | Error types & misc | M4 (AppError + `From` chain), M9 (ErrorBoundary CSS extraction), plus curated LOWs | `core/error.rs`, `app.rs`, new CSS module | Medium |

LOW items to address in Track I: L1 (Clone+Copy convention doc), L2 (`wallet::clear_session` dedup), L5 (`PathArg` newtype depth), L10 (`🔒` emoji → icon), L12 (BottomSheet drag handle a11y). Other LOWs are deferred.

---

## Cherry-pick (Track P)

Small, high-confidence items from `wip/january-2026-restructure` (commit `4fec9f4`). Only items with clear value AND no dependency on WIP's larger write-capability work.

| Item | File | Rationale |
|---|---|---|
| CSP: add `https://api.ensideas.com` to `connect-src` | `index.html` | Phase 1 final review flagged that ENS resolution is broken in production because the API host isn't whitelisted. One-line fix. |
| Breadcrumb absolute-path navigation | `src/components/breadcrumb.rs` | Appears to fix a bug where nested breadcrumb clicks built paths via `route.join(segment)` (relative, fragile) instead of absolute segments. **Must be verified** when starting the track — if stylistic not bugfix, drop it. |

Not cherry-picking now (each has a reason):

- **`Storage` enum split from `Mount`** — Architecturally nicer but overlaps with how Phase 1 reshaped `MountRegistry`. Worth doing as its own track when write capability is revisited; not Phase 2.
- **Serde on `FileMetadata`/`DirectoryMetadata`** — Would be useful but has no current consumer. YAGNI.
- **`markdown_to_html_with_images`** — Only useful with editor + image upload. Defer until write capability is planned.
- **Icon additions, `ReaderViewMode`, `current_timestamp`, Cargo deps (`base64`, `urlencoding`)** — All only useful with the write/editor features. Defer.

---

## Execution Model

Per the strategy set in Phase 1:

1. **Per track: one fresh worktree branched off `main` (or the most recently merged track if dependencies require).**
2. **Per track: a detailed plan document written at start** (via `superpowers:writing-plans`). Saved to `docs/superpowers/plans/YYYY-MM-DD-phase2-<track-id>.md`.
3. **Per track: implementation via `superpowers:subagent-driven-development`** — implementer (Opus) + spec reviewer (Opus) + code quality reviewer (Opus).
4. **Per track: merged back to `main` as a merge commit (`--no-ff`)** for clean history.
5. **Within a wave, tracks may run in parallel** on separate worktrees. Merges are sequential.

## Merge Order Within Waves

Wave 1 merge order (minimizes future conflicts):
- **D → B → A → C → P** (route/fs foundation first, then filter/parser on top, then UI-agnostic items).

Wave 2 merge order:
- **F → E → G → H** (terminal render changes touch files that Explorer UI also touches; do F first to reduce conflicts).

Wave 3:
- **I** alone.

## Done Criteria (Phase 2 overall)

- All HIGH issues from the review resolved except deferred crypto/deployment items.
- All MEDIUM issues in the track scope above resolved.
- Curated LOW items addressed in Track I.
- Tag `phase2-complete` on the merge commit of Track I.
- `cargo test` still passing (may add tests; may remove only the 4 pre-existing `test_permissions_*` failures as a separate bug).
- `trunk serve` smoke-tested after Waves 2 and 3.

---

## Track Indexing

Per-track detailed plans will be created at `docs/superpowers/plans/2026-MM-DD-phase2-<id>-<short-name>.md`. Suggested naming:

- Track D → `phase2-d-route-resolve.md`
- Track B → `phase2-b-filters.md`
- Track A → `phase2-a-parser.md`
- Track C → `phase2-c-autocomplete.md`
- Track P → `phase2-p-cherry-pick.md`
- Track F → `phase2-f-terminal-render.md`
- Track E → `phase2-e-reader-race.md`
- Track G → `phase2-g-explorer-ui.md`
- Track H → `phase2-h-navigation.md`
- Track I → `phase2-i-cleanup.md`

Each per-track plan is self-contained; readers start there, not here.
