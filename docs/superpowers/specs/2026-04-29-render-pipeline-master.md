# Render Pipeline Refactor — Master Plan

**Date:** 2026-04-29
**Status:** Active
**Reviewers:** _self_

This is the **single entry point** for the render pipeline refactor. Every phase begins by re-reading this document.

---

## 1. Goal

Clean up the residual classification duplication, dead arms, synthetic frames, and unused fields at the `engine` ↔ `components` boundary. When the refactor is done:

- Adding a new file extension requires touching only one place in the engine.
- Intents that `Reader` cannot handle are blocked **at the type level**.
- Synthetic `RouteFrame` patterns that bypass the engine are either absorbed into routing or partitioned into an explicit type.
- Every field of `RenderIntent` is actually consumed.
- The router focuses on dispatch and nothing else.

## 2. Phase Plan

Sequential. Each phase is its own self-contained set of changes; the next phase begins only after the previous phase's review clears. **All phases accumulate in the working tree — no per-phase commits — and a single commit lands at the end.**

| # | Title | Outcome | Status |
|---|---|---|---|
| 1 | Engine classification unification | Decompose `RenderIntent::DocumentReader` so the engine produces the final content variant | **Complete** |
| 2 | Narrow `ReaderIntent` type | A narrower enum containing only the variants `Reader` accepts; remove the `Unsupported` arm | **Complete** |
| 3 | `layout` field audit | If unused, remove (YAGNI); if used, split layout-bearing intents into a sub-type | **Complete** |
| 4 | Synthetic `RouteFrame` policy | Decide engine-absorption vs. explicit `BuiltinRoute` partition, then apply | **Complete** |
| 5 | Router cleanup | Move the focus side-effect out of the router; remove dispatch-arm unwrap noise | **Complete** |

## 3. Per-Phase Workflow

Every phase follows this exact sequence. **Skipping or reordering steps undermines the safety the structure provides.**

```
┌─ Phase Start ──────────────────────────────────────────────────────┐
│  1. Re-read this master plan                                       │
│  2. Write design doc:                                              │
│       docs/superpowers/specs/2026-04-29-render-pipeline-phase<N>-  │
│         design.md                                                  │
│       — scope · types · components · tests · acceptance            │
│       — self-review (placeholders · contradictions · scope)        │
│       — get user approval                                          │
│  3. Write implementation plan:                                     │
│       docs/superpowers/plans/2026-04-29-render-pipeline-phase<N>-  │
│         plan.md                                                    │
│       — concrete steps · file changes · tests · risks              │
│       — get user approval                                          │
│  4. Implement per plan, mark each step complete as it ships        │
│  5. Local verification:                                            │
│       cargo test --lib                                             │
│       cargo check --target wasm32-unknown-unknown --lib            │
│       trunk build (visual QA via trunk serve when applicable)      │
│  6. Invoke superpowers:code-reviewer on the change                 │
│       — pass: design doc + plan + diff                             │
│       — address all CRITICAL / HIGH findings                       │
│  7. Update §2 status → Complete, append to §5 Decision Log         │
│  8. **Do not commit.** Per-phase changes accumulate in the working │
│     tree; a single commit covers the entire refactor at the end.   │
│  9. Begin next phase from step 1                                   │
│                                                                    │
│  After Phase 5 passes review: stage all changes (code + spec docs  │
│  + plan docs + master updates) into one commit.                    │
└────────────────────────────────────────────────────────────────────┘
```

### 3.1 Workflow rules

- Do not start a new phase before the previous phase's review has cleared (no outstanding CRITICAL/HIGH).
- Do not write code before the phase's design and plan have user approval. The design + plan are the contract.
- Reviews run at the end of each phase, not mid-implementation. Mid-flight reviews waste cycles on incomplete code.
- **No commits during the refactor.** All five phases accumulate in the working tree. The final commit lands after Phase 5 review passes.
- If §1 goal or §2 phase plan shifts mid-flight, update this master *first*, then update the affected phase's design doc.

## 4. Document Index

Accumulates as phases progress. Append a row when a new artifact lands.

| Phase | Artifact | Path | Status |
|---|---|---|---|
| Master | This file | `docs/superpowers/specs/2026-04-29-render-pipeline-master.md` | Active |
| 1 | Design | `docs/superpowers/specs/2026-04-29-render-pipeline-phase1-design.md` | Approved |
| 1 | Plan | `docs/superpowers/plans/2026-04-29-render-pipeline-phase1-plan.md` | Complete |
| 2 | Design | `docs/superpowers/specs/2026-04-29-render-pipeline-phase2-design.md` | Approved |
| 2 | Plan | `docs/superpowers/plans/2026-04-29-render-pipeline-phase2-plan.md` | Complete |
| 3 | Design | `docs/superpowers/specs/2026-04-29-render-pipeline-phase3-design.md` | Approved |
| 3 | Plan | `docs/superpowers/plans/2026-04-29-render-pipeline-phase3-plan.md` | Complete |
| 4 | Design | `docs/superpowers/specs/2026-04-29-render-pipeline-phase4-design.md` | Approved |
| 4 | Plan | `docs/superpowers/plans/2026-04-29-render-pipeline-phase4-plan.md` | Complete |
| 5 | Design | `docs/superpowers/specs/2026-04-29-render-pipeline-phase5-design.md` | Approved |
| 5 | Plan | `docs/superpowers/plans/2026-04-29-render-pipeline-phase5-plan.md` | Complete |

## 5. Decision Log

Chronological, append-only.

| Date | Decision | Reference |
|---|---|---|
| 2026-04-29 | Five-phase plan adopted: engine classification → narrow `ReaderIntent` → `layout` audit → synthetic frame policy → router cleanup. | §2 |
| 2026-04-29 | Single final commit (no per-phase commits) at user request. | §3 |
| 2026-04-29 | Phase 1 complete. `RenderIntent::DocumentReader` decomposed into `HtmlContent` / `MarkdownContent` / `PlainContent` (Page+Document axis collapsed at intent layer; `ResolvedKind` keeps the routing-internals split). `Reader::load_renderer_content` is now a flat dispatcher. Engine reuses `FileType::from_path` and `utils::media_type_for_path` for classification. 11 intent tests cover every (kind, ext) combo including the design-§5.4 set plus a follow-on Image-asset test from reviewer feedback (M1). Reviewer cleared with no CRITICAL/HIGH; one MEDIUM accepted (Image test added), one MEDIUM accepted-deferred (Page→Plain regression test, optional). Two LOW deferred to Phase 2/3 (Reader trusting engine `media_type`, manifest `RendererKind` alignment). | §2, §4 |
| 2026-04-29 | Phase 2 complete. `ReaderIntent` enum + `ReaderFrame` struct introduced in `reader.rs`; `Reader` now takes `Memo<ReaderFrame>` so `DirectoryListing` / `TerminalApp` cannot reach it. `RendererContent::Unsupported` removed. Router dispatch is exhaustive (no `_ =>`); `TryFrom<RouteFrame> for ReaderFrame` rejects the two surface-bound variants. `SiteChrome` keeps its `RouteFrame` interface via a derived `chrome_route` Memo using `From<ReaderFrame> for RouteFrame`. 519 tests passing (508 baseline → 514 after Phase 2 initial → 519 after reviewer-driven test additions for round-trip across all five Reader variants and `From<ReaderIntent>` field preservation). Reviewer cleared with no CRITICAL/HIGH; both MEDIUMs (round-trip variant coverage, `From` impl test) accepted and addressed; two LOWs deferred (extracting `intent.rs` submodule, `mount_reader` helper extraction). | §2, §4 |
| 2026-04-29 | Phase 3 complete. `layout` field — produced by `intent.rs:40-42` from sidecar metadata, propagated through every `RenderIntent` and `ReaderIntent` variant — confirmed dead via grep (zero consumers). Removed from all runtime intent types and conversions; tests updated; reviewer's MEDIUM-1 (drop now-unused `_fs` parameter from `build_render_intent`) addressed inline by removing the parameter and `super::global_fs::GlobalFs` import. Persistence types (`FileSidecarMetadata.layout`, `DirectorySidecarMetadata.layout`, `LoadedNodeMetadata.layout`) intentionally kept as forward-compat for on-disk JSON. 519 tests still passing; reviewer cleared with no CRITICAL/HIGH. | §2, §4 |
| 2026-04-29 | Phase 4 complete. `BuiltinRoute` enum + `detect` method introduced in `router.rs`; the three independent `is_*_route` predicates collapse into a single `match BuiltinRoute::detect(&request)` two-stage dispatch (builtin first, engine second). Synthetic frame helpers (`home_frame`, `ledger_filter_frame`, `new_compose_frame`) stay private to `router.rs`. `builtin_home_frame` renamed `home_frame` for symmetry. **Decision: BuiltinRoute partition over engine absorption** — engine stays UI-agnostic per CLAUDE.md ("UI renders engine output, engine does not assemble UI"). 524 tests passing (+5 builtin detection tests including `/ledger/foo` rejection per reviewer LOW). Reviewer cleared with no CRITICAL/HIGH/MEDIUM. | §2, §4 |
| 2026-04-29 | Phase 5 complete. Two helpers extracted in `router.rs`: `static_route_memo(frame)` removes the `Memo::new(move \|_\| route.get().expect(\"frame available\"))` boilerplate from three engine dispatch arms; `install_terminal_focus_effect(raw_request, route)` lifts the inline focus side-effect into a named function. `RouterView` body shrank by ~20 lines; the `Effect::new` for focus moves out of the router proper without changing behaviour. 524 tests still passing. Reviewer cleared with no CRITICAL/HIGH for the refactor itself; flagged commit-hygiene HIGHs (untracked scratch files at repo root + unrelated `content/*` drift + `mempool/component.rs` wording tweak) which are explicitly excluded from the final refactor commit. | §2, §4 |
| 2026-04-29 | Refactor complete (Phases 1-5). All five reviewer passes cleared; 524 tests passing; cargo + trunk builds green; engine-only spike (`.txt` plain intent) traceable to a single `intent.rs::content_intent_for_node` arm via the `FileType::Unknown` branch. Final commit will bundle: Phase 1-5 code (intent.rs, reader.rs, router.rs, global_fs.rs), the Phase-0 ledger connector refactor (ledger_page.rs, ledger_page.module.css), and the full doc set under `docs/superpowers/` (master + 5 designs + 5 plans). Excluded from the commit (per reviewer H1/H2): `content/manifest.json`, `content/now.toml`, `src/components/mempool/component.rs`, `ledger.html`, `render-app.jsx`, `render.css`, `render.html`. | §6 |
| 2026-04-30 | Final-review polish. Phase 1 LOW (`load_asset` re-derives `media_type` from path despite the engine carrying it on `RenderIntent::Asset`) closed: `load_asset` now takes `media_type: String` from the caller; the dispatch arm destructures it from `ReaderIntent::Asset { media_type, .. }`; the `media_type_for_path` import in `reader.rs` is dropped (only the engine still calls it). Phase 1's promise — engine is the single owner of classification — now holds without exception. Manual UI QA confirmed by user (no regressions across content / ledger / compose / shell-focus). 525 tests passing; cargo + trunk green. | §6 |
| 2026-04-30 | Deferred-item sweep. Three remaining LOWs / MEDIUMs from the phase reviews resolved: (1) `raw_source`'s `FileType::Markdown` gating (`reader.rs:218`) replaced with `matches!(frame.get().intent, ReaderIntent::Markdown { .. })` — `FileType` import dropped from the reader module; Phase 1 invariant ("no extension dispatch in components/") now holds without exception. (2) `reader.rs` (727 lines, 91% of style cap) split into `src/components/reader/{mod.rs (471), intent.rs (264)}`; types and conversions live alongside their tests in the sibling module, the component file holds component logic only. (3) `home_frame` / `ledger_filter_frame` no longer rebuild `RouteRequest::new(request.url_path)` — they pass the incoming request through, removing a dead re-normalization and protecting future `RouteRequest` field additions from silent drop. Two reviewer items kept deferred-as-accepted: manifest `RendererKind` rename (different layer / different concept; rename would break on-disk schema), and `Page → PlainContent` regression test (duplicate of existing `Document → PlainContent` test exercising the same `content_intent_for_node` arm). 525 tests passing; cargo + trunk green. | §6 |

## 6. Acceptance — refactor as a whole

- Every phase's acceptance criteria are met.
- `cargo test --lib`, `cargo check --target wasm32-unknown-unknown --lib`, and `trunk build` all green.
- Adding a new file type is a **single-source-of-truth change** (verification spike: introducing a `.txt` plain-text intent should require touching only one engine file).
- Zero synthetic `RouteFrame` constructions outside an explicit, named partition type.
- `code-reviewer` has cleared every phase with no outstanding CRITICAL or HIGH findings.

## 7. State

- **Active phase:** Complete — ready for final commit
- **Last updated:** 2026-04-29
