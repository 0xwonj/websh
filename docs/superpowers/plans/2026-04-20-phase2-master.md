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

---

## Decision Log (autonomous execution)

Recording decisions made during Phase 2 execution, in the order they happen. Each entry: decision + brief rationale.

### Track D — Route/FS resolve (merged `ca379ce`)
- **Test context for `test_cd_empty_string_exit_1` switched from `Root` to `Browse`**: the Root-context path already errored on `""` through a different branch, so the test wouldn't have proven the new fix. Browse context exercises the actual silent-stay bug.
- **Added `#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]` on `AppRoute::resolve`**: the only wasm-gated caller in `AppRouter` means native builds see it as dead code even though tests use it. Not a design smell — tests run under `cfg(test)` which doesn't trigger dead-code check for the attribute's target.
- **FS resolution runs in `AppRouter`, not in `AppRoute::from_path`**: kept `from_path` pure (parse only); resolution is a separate explicit step that can depend on the reactive `fs` signal via Memo. Decouples hash parsing (synchronous, early) from fs knowledge (loads async after boot).
- **Heuristic fallback in `resolve`**: when fs has no entry for the path, keep the extension-based decision instead of defaulting to one variant. Covers the "loading window" between boot and manifest fetch.
- **Minor suggestions deferred**: M1 (extract heuristic helper) and M2 (stronger test of fallback) from the code review are noted; small QoL, not blocking. Revisit if Track P or another track touches route.rs.

### Track B — Command filters (merged)
- **D-B-1: H4 (iterator streaming) deferred to Phase 3+.** WebSH's typical pipeline processes dozens-to-low-hundreds of lines (`ls`, `help`, `id`, etc.). Iterator streaming requires stateful exit-code accumulation (grep's exit code depends on match count, not known until iteration ends) — forces `Rc<Cell<i32>>` plumbing or `CommandResult` redesign. Cost/benefit unfavorable at current scale. Revisit if a command starts producing thousands of lines.
- **D-B-2: `grep` default is now case-sensitive.** Reverses prior case-insensitive default. POSIX-correct; `-i` opt-in.
- **D-B-3: `head 5` / `tail 2` (bare positional, no dash) now errors with exit 2.** POSIX requires `-N` or `-n N`. No prior user code can have relied on the loose form alone — those inputs were "working by accident" via `trim_start_matches('-')`. Two internal tests updated to use `-3`/`-2` dash form.
- **D-B-4: `regex = "1"` with `default-features = false, features = ["std", "perf"]`.** Strips `unicode-*` sub-features (~200-300 KB of Unicode tables) to keep the wasm bundle small. ASCII regex is sufficient for shell pattern matching; if ever needed, user can still write `[a-zA-Z]` explicitly.
- **Minor suggestions deferred**: flag-after-pattern test, mixed short/long form test, error-message polish for `-abc`. All QoL, not blocking. Accept current error wording.

### Track A — Parser (merged)
- **D-A-1: Lexer rewrites word building inline (`parse_word_segment`) and removes `Token::Variable`.** Rationale: coalescing literal+variable+quoted into one Word is the mental model POSIX shells use. Having a separate `Token::Variable` that gets expanded later forces an artificial adjacency-tracking pass. Inline expansion is simpler and already matched the pre-existing double-quoted body.
- **D-A-2: Unquoted `$UNDEF` drops the word.** POSIX-correct. Test: `echo $UNDEF hi` → argv `["echo", "hi"]`. Implemented via the lexer's tri-state accumulator + iterator retry on None.
- **D-A-3: Quoted `"$UNDEF"` yields empty string (was literal `$UNDEF` before).** Behavior change, POSIX-aligned. Pre-existing callers weren't depending on the old non-POSIX string.
- **D-A-4: `dom::window()` returns `None` on non-wasm targets.** Cross-cutting change to enable native unit tests that touch env/wallet paths. Only caller (`wallet.rs`) already handled `Option`. Pragmatic; a proper `EnvProvider` trait is a later refactor.
- **Known gaps documented as follow-ups**: `echo foo!bar` / `echo foo!!` do NOT coalesce (`!` breaks word). POSIX bash would merge. Out of scope for Track A. Track for future.
- **Pre-existing bug flagged**: `export FOO='"quoted"'` — `execute_export` trims surrounding quotes twice. Not introduced here; Track I candidate.

### Track C — Autocomplete (merged)
- **D-C-1: `less`, `more` removed from `FILE_COMMANDS`.** They were never implemented commands — autocomplete advertised them as file-path accepting, but `Command::parse` mapped them to `Unknown(127)`. Inconsistency removed.
- **D-C-2: `find_common_prefix` switched from byte-slice to char-iterator.** `first[..prefix_chars].to_string()` was a UTF-8 panic waiting to happen (Korean, emoji, etc.). Now `first.chars().take(prefix_chars).collect()`.

### Track P — Cherry-pick from WIP (merged)
- **D-P-1: CSP `api.ensideas.com` added to `connect-src`.** Phase 1 final review flagged that ENS resolution silently fails because the API host wasn't whitelisted. One-line fix from WIP.
- **D-P-2: Breadcrumb absolute-path construction.** Verified as a genuine bugfix — the old `route.join(segment)` path broke for nested navigation from a `Read` route (which joins relative to parent). New code builds `Browse { mount, path: abs_path }` directly.

### Track F — Terminal render perf (merged)
- **D-F-1: `ctx.fs.get()` → `ctx.fs.with()`** in `create_submit_callback`. Old code cloned the entire `VirtualFs` (recursive `HashMap<String, FsEntry>`) per command dispatch.
- **D-F-2: `OutputLineId(u64)` newtype.** Atomic counter `AtomicU64`; `PartialEq` on `OutputLine` now derived structurally (includes id). The hand-written id-ignoring `PartialEq` was a footgun — pattern matching on `.data` is the right comparison point anyway.
- **D-F-3: `history_signal.get().to_vec()` → `history_signal.with(|buf| buf.iter().cloned().collect())`.** Dropped one full RingBuffer clone per render.

### Track E — Reader race (merged)
- **D-E-1: `Effect + spawn_local` → `LocalResource`.** Closed a real race where stale fetches could overwrite current content. `LocalResource` cancels previous futures on input change. Mirrors the existing `preview/hook.rs::use_preview` pattern.

### Track G — Explorer UI a11y (merged)
- **D-G-1: 11 `console::log_1` debug calls removed** from stub UI handlers (`explorer/header.rs`, `reader/mod.rs`). No TODO comments left — stubs are empty-bodied.
- **D-G-2: `FileListItem` keyboard nav.** Enter = open, Space = select. Implemented by extracting `do_select` / `do_open` as shared closures, called from both mouse and keyboard handlers.
- **D-G-3: `close_on_focus_out` helper extracted.** `NewMenu`/`MoreMenu` in `header.rs` now share the focus-out close logic (-15 net lines).

### Track H — Navigation (merged)
- **D-H-1: Deleted `ExplorerState.forward_stack` entirely; delegated to browser history.** The in-app stack couldn't see navigations initiated by the browser's own back/forward buttons (the router listens on hashchange but the stack isn't wired into that path). Delegating `window.history().back()` / `.forward()` makes the browser the authoritative source. Forward button is now always-active — a no-op click is the browser's own behavior.
- **D-H-2: Back button stays `is_root`-disabled.** At Root, `history.back()` would take the user out of the app. Keep the guard.

### Track I — Cleanup (merged)
- **D-I-1: ErrorBoundary styles moved from inline to `src/components/error_boundary.module.css`.** Kept literal color values (the site's design tokens in `assets/base.css` use a different palette — Tokyo-Night — and the ErrorBoundary intentionally uses the legacy dark-blue scheme).
- **D-I-2: `wallet::disconnect(ctx)` helper** dedupes the `clear_session + set Disconnected` pair in `terminal.rs:78` and `shell.rs:66`.
- **D-I-3: Lock emoji `🔒` replaced with `<Icon icon=ic::LOCK />` SVG** in `output.rs` list-entry rendering. Added `.lockIcon` class in `output.module.css`. `aria-label="encrypted"` for screen-reader context.
- **D-I-4: BottomSheet drag handle a11y** — `role="button"`, `aria-label`, `tabindex=0`, Enter/Space binding that calls `close()` (best-effort; no prior click handler to reuse, and the handle has no persistent snap-point state for ArrowUp/Down).
- **D-I-5: Convention note added** in `src/app.rs` documenting why signal-container structs derive `Clone, Copy` (all fields are Leptos signals which are `Copy`-cheap arena pointers).

## Phase 2 Deferred Items (Phase 3+ candidates)

These were considered in Phase 2 scope but deferred with explicit reasoning:

- **H4 (iterator streaming for pipe filters)**: buffered `Vec<OutputLine>` is adequate for current workloads. See D-B-1.
- **M4 (`AppError` unification + `From` chain)**: three domain errors are cleanly used locally. Unifying is refactor-for-refactor's-sake until a concrete cross-domain path appears.
- **L5 (`PathArg` newtype thinness)**: wraps `String` with no unique methods. Deletion or thickening both churn command match arms without user payoff.
- **`!`-coalescing in lexer**: `echo foo!bar` splits into two tokens; POSIX bash would concat. Out of Track A scope; documented follow-up.
- **`export FOO='"quoted"'` quote double-trimming**: pre-existing bug in `execute_export`; not introduced by Phase 2 changes.
- **All Track D minor suggestions** (M1/M2 from Track D code review): heuristic helper extraction, stronger fallback test. QoL, not blocking.
- **All Track B minor suggestions**: flag-after-pattern test, mixed short/long form test, error-message polish for `-abc`.

## Phase 2 Retrospective

**Merged to main** (17 tracks including Phase 1, 9 Phase 2 tracks):
- `df5a53d` → `phase2-complete` tag
- Tests: 148 → 189 passing (+41); 4 pre-existing permission-string failures remain, flagged for a separate bug fix.
- `cargo build --release --target wasm32-unknown-unknown`: clean throughout.

**Issues addressed** (26 items):
- **CRITICAL**: C1, C2
- **HIGH**: H1, H2, H5, H6, H8, H9, H10, H11 + H3, H7 from Phase 1
- **MEDIUM**: M1, M2, M3, M5, M6, M7, M8, M9, M10, M11, M12
- **LOW**: L1, L2, L10, L12
- **Cherry-pick from WIP Jan 2026**: CSP ENS, breadcrumb fix

**Key architectural wins**:
1. Mount registry is a single `&'static` singleton with compile-time non-empty invariant.
2. `CommandResult { exit_code, side_effect }` gives POSIX-correct error propagation and a unified UI dispatcher.
3. Reader fetches are race-safe via `LocalResource`.
4. Forward navigation now uses the browser's own history, eliminating a whole class of desync bugs.
5. Parser respects POSIX word-coalescing and unclosed-quote errors.

**Cycle notes**:
- Phase 2 ran in 3 waves: foundation (D, B, A, C, P), UI/state (F, E, G, H), cleanup (I).
- Each track: write plan → dispatch Opus implementer → dispatch Opus reviewer(s) → fix review issues → merge to main with `--no-ff`.
- 2 Opus agent timeouts during the run (Track E — partial work rescued and committed manually); otherwise clean.
- Wave 1 through I took one autonomous session end-to-end.

## Phase 2 Follow-ups (merged, `phase2-complete` tag moved)

After the first `phase2-complete` tag (`9037f4c`), a 6-agent multi-perspective review (Architecture, POSIX/Shell, Leptos idioms, WASM perf, Test coverage, Backward-compat) surfaced additional actionable items. All were addressed in a single follow-up track merged as `d7d0e88`; the tag now points there.

### Fixes applied
- **D-FU-1**: `DisplayPermissions` format corrected from `"d r - x"` (spaced) to `"dr-x"` (Unix `ls -l` canonical). One-line fix cleared all 4 pre-existing `test_permissions_*` failures.
- **D-FU-2**: `assets/text/help.txt` and `CLAUDE.md` updated to reflect Phase 2 semantics (grep case-sensitivity + `-i`, head/tail dash form, multi-assign export, new public APIs section).
- **D-FU-3**: `src/components/breadcrumb.rs` extracted `build_segment_path(segments, idx)` helper + 4 native unit tests (closes the Track P test gap). `test_resolve_unknown_path_keeps_heuristic` strengthened to exercise both heuristic-promote and heuristic-demote branches. Duplicate `test_head_default_no_args` removed.
- **D-FU-4**: `AppError` enum introduced in `src/core/error.rs` with `From` impls for `WalletError` / `FetchError` / `EnvironmentError` + `std::error::Error::source` chain. `#[allow(dead_code)]` for now — no current callers; preempts the cross-domain `?` friction flagged in architecture review. 5 tests.
- **D-FU-5**: `grep -F` / `--fixed-strings` added (uses `regex::escape`). Extra-positional error message improved to explicitly mention pipes-only model. 4 tests.
- **D-FU-6**: `AppRouter` `Memo` fast-paths `AppRoute::Root` (skips `ctx.fs` tracking when route doesn't depend on fs). `TerminalState::navigate_history` uses `.with()` instead of `.get()` — avoids cloning the entire command history on every arrow-key press.

### Test delta
- **189 pass / 4 fail → 205 pass / 0 fail** (+16 new tests, 4 pre-existing failures fixed).

### Explicitly deferred to Phase 3 (write-mode prep)
- Async `SideEffect` variant / `dispatch_side_effect` async path — requires a concrete write-command spec to design correctly.
- `FsState` / `MergedFs` overlay (pending/staged layer) — same reason.
- These two will be designed alongside the first write command; doing them speculatively now would be YAGNI.

### Also deferred
- `grep pat1 pat2` as bash-compat filename handling (we have no file I/O in the browser shell — the clearer error message is sufficient).
- BRE (POSIX basic regex) mode for grep — the `regex` crate is always ERE; users needing BRE can escape explicitly. Not worth a second regex engine.
- Help-text in-terminal `man` / `--help` per-command — each command showing its own help is a UX project, not a Phase 2 follow-up.




