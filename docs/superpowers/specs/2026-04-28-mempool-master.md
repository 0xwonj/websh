# Mempool V1 — Master Plan

**Date:** 2026-04-28
**Status:** Active
**Reviewers:** _self_

This is the **single entry point** for the Mempool V1 effort. Every phase begins by re-reading this document. Per-phase design and plan documents live alongside this file and are referenced from §6.

---

## 1. V1 Final Goal

Add a mempool — a pool of pending content entries — to the `/ledger` page, plus the authoring and promotion flows that let me write, edit, and publish posts directly from the deployed site.

When V1 is complete, I can:

1. **See** pending drafts and reviews above the chain head, filtered by category alongside published blocks.
2. **Compose** a new draft from a button on the website, which is committed to the mempool repo and appears immediately for me (and any viewer) on next page load.
3. **Edit** an existing draft from the same site, with changes committed back to the mempool repo.
4. **Promote** a draft from the mempool to the canonical chain via the local CLI (`websh-cli mempool promote`), which atomically writes to `content/`, regenerates ledger + manifest, and produces a single bundle-source commit. Mempool repo cleanup is a follow-up best-effort call.
5. **Deploy** is still a manual step (existing `just pin` / trunk build flow); after deploy, the promoted entry becomes a confirmed block on the IPFS-anchored ledger.

V1 closes the loop "draft → mempool → promote → deploy → confirmed block". Compose / edit happen entirely from the static-site UX (browser-only, no terminal). Promote and deploy happen at the local terminal as part of the same publish ritual.

## 2. Constraints

The site is a static IPFS deployment. Three constraints shape every decision:

1. **No server.** All build / publish / pin operations happen on my machine at deploy time.
2. **Mempool must update without redeploy.** A new draft becomes visible immediately after a GitHub commit, no IPFS rebuild required.
3. **Bundle integrity stays clean.** The bundled `ledger.json` is signed and pinned. The mempool sits *outside* that integrity story by design — pending entries are explicitly "not in the chain yet".

## 3. Architecture Anchors

These are the load-bearing decisions for V1 as a whole. Per-phase design docs MUST honor them; if a phase needs to deviate, update this section first and link the rationale.

| # | Anchor | Rationale |
|---|---|---|
| A1 | Mempool storage = a separate public GitHub repo, `0xwonj/websh-mempool` | Mutable across deploys; reuses existing GitHub mount; pending content is naturally public ("pending tx" in a public mempool) |
| A2 | Mounted at `/mempool` (no `/mnt` prefix) | First-class architectural concept, single instance, clean paths; `/mnt` namespace stays free for future generic mounts |
| A3 | Bundle (`/site` via `BOOTSTRAP_SITE`) and mempool are separate trees, not merged | Preserves bundle integrity; live updates without rebuild; conflict-resolution logic avoided |
| A4 | Authoring uses the existing Phase 3a GitHub commit infrastructure | No daemon required; works from any browser with a write token in session |
| A5 | ~~Mempool item click → modal preview, not URL navigation~~ | **Dropped 2026-04-29 (Phase 6).** Original rationale was URL-bar privacy, but A1 already declares the mempool repo public; URL exposure has no confidentiality cost. Modals removed in favor of URL-driven flows: `/<path>` view, `/edit/<path>` edit, `/new` compose. See Phase 6 design §2. |
| A6 | Promotion is a CLI-only operation, not a browser flow | **Revised after Phase 4 live-QA** (was: two-commit browser transaction). Bundle source is the local-deploy source-of-truth; promote is a local atomic commit there. Browser keeps compose/edit; promote moves to CLI. Removes cross-repo race, drops bundle-write PAT from browser, lets ledger/manifest/attestation regenerate before deploy. |
| A7 | Local daemon is V2, not V1 | GitHub commit covers V1 needs; daemon adds value only for offline / automated-deploy use cases |
| A8 | CLI in V1: mount-init (Phase 4) + promote/drop (Phase 5) | **Revised**. Original anchor said "no CLI work in V1"; live-QA forced two CLI surfaces. (1) `mount init` for repo bootstrap (Phase 4). (2) `mempool promote` / `mempool drop` for atomic publishing (Phase 5). Both are deploy-time host operations — they do not run in wasm. |
| A9 | **Reserved URL prefixes** for the hash-router: `/`, `/ledger`, `/websh`, `/explorer`, `/new`, `/edit/`. Content repo (`/site` via `BOOTSTRAP_SITE`) and mempool repo MUST NOT introduce files or directories whose top-level URL segment collides with this list. | Added 2026-04-29 (Phase 6). `/new` and `/edit/` are claimed by the URL-driven mempool authoring flow; `/ledger`/`/websh`/`/explorer` are claimed by the existing router; `/` is the home route. A future content file at `content/new.md` or `content/edit/foo.md` would be silently shadowed and unreachable. |

### 3.1 Explicitly rejected alternatives

- **Merging GitHub mount into the bundle namespace** (serving `/writing/foo.md` from either source): muddies `ledger.json` integrity, demands conflict rules, scope creep beyond blog UX.
- **Single repo with `status`-based filtering** (drafts and published in the same repo): mutable drafts pollute the bundle source repo's history; bundle build still must exclude drafts.
- **Path-based draft directory** (e.g. `content/drafts/...`): drafts then ride bundle deploy cadence, losing live-update capability.

## 4. Phase Plan

Three phases, executed sequentially. Each is its own PR.

| Phase | Title | Outcome | Status |
|---|---|---|---|
| 1 | Read-only Mempool | Mempool section renders pending entries from `/mempool` mount; click opens modal preview; filter integration | **Complete** |
| 2 | Authoring (Compose & Edit) | Author-mode toggle, compose modal, edit existing draft, GitHub commit to mempool repo | **Complete** |
| 3 | Promotion (browser) | Promote button on mempool item, two-commit transaction, deploy hint banner | **Complete (superseded by Phase 5)** |
| 4 | Hardening | Strict mount-root match for writes, 404-tolerant scan, compose runtime reload, manifest pre_build hook, CLI `mount init` | **Complete** (pending live QA) |
| 5 | CLI Promote (browser → host) | Replace browser promote modal with `websh-cli mempool promote/drop`. Atomic single-commit on bundle source; ledger/manifest/attestations regenerated locally; mempool drop as follow-up | **Complete** |
| 6 | Reader-Unified Mempool UI | Drop modals; mempool participates in URL navigation. `/<path>` view, `/edit/<path>` edit, `/new` compose. `Reader` and both modal components deleted; `MempoolEditor` + `MempoolEditorPage` host the un-modal'd compose form | **Complete** |

After Phase 6, V1 is complete (Phases 4–6 are post-Phase-3 hardening + architectural pivot + UX consolidation — see Decision Log). V2 items (§7) are queued separately.

## 5. Per-Phase Workflow

Every phase follows this exact sequence. The workflow is non-negotiable: skipping or reordering steps undermines the safety the structure provides.

```
┌─ Phase Start ─────────────────────────────────────────────────┐
│  1. Re-read this master plan (§3 anchors must hold)            │
│  2. Write design doc:                                          │
│       docs/superpowers/specs/<date>-mempool-phase<N>-design.md │
│       — scope, schema, components, tests, acceptance           │
│       — review for placeholders, contradictions, ambiguity     │
│       — get user approval                                      │
│  3. Write implementation plan:                                 │
│       docs/superpowers/plans/<date>-mempool-phase<N>-plan.md   │
│       — concrete steps, file changes, test cases, risks        │
│       — get user approval                                      │
│  4. Implement per plan, mark each step complete as it ships    │
│  5. Run full local verification:                               │
│       cargo test --lib                                          │
│       cargo check --target wasm32-unknown-unknown --lib         │
│       (visual QA via trunk serve when applicable)              │
│  6. Invoke superpowers:code-reviewer agent on the change       │
│       — pass: design doc + plan + diff                         │
│       — address all CRITICAL / HIGH findings                   │
│  7. Update plan with final state, mark phase complete in §4    │
│  8. Commit + push + PR (one PR per phase)                      │
│  9. Begin next phase from step 1                               │
└────────────────────────────────────────────────────────────────┘
```

### 5.1 Workflow rules

- **Do not start a phase before the previous phase's PR is merged** (or explicitly merged-in-spirit if PRs are batched). Each phase builds on the previous.
- **Do not begin coding before the plan is approved.** The design doc + plan are the contract; coding without one means the contract is implicit, which is where bugs hide.
- **Reviewer agent runs at end of phase, not mid-implementation.** Mid-implementation reviews waste cycles on incomplete code.
- **If anchors change** (§3), update this master file *first*, then revisit the affected per-phase design.

## 6. Document Index

Generated as phases progress. Update the table when a new artifact lands.

| Phase | Artifact | Path | Status |
|---|---|---|---|
| Master | This file | `docs/superpowers/specs/2026-04-28-mempool-master.md` | Active |
| 1 | Design | `docs/superpowers/specs/2026-04-28-mempool-phase1-design.md` | Approved |
| 1 | Plan | `docs/superpowers/plans/2026-04-28-mempool-phase1-plan.md` | Complete |
| 2 | Design | `docs/superpowers/specs/2026-04-28-mempool-phase2-design.md` | Approved |
| 2 | Plan | `docs/superpowers/plans/2026-04-28-mempool-phase2-plan.md` | Complete |
| 3 | Design | `docs/superpowers/specs/2026-04-28-mempool-phase3-design.md` | Approved |
| 3 | Plan | `docs/superpowers/plans/2026-04-28-mempool-phase3-plan.md` | Complete |
| 4 | Design | `docs/superpowers/specs/2026-04-28-mempool-phase4-design.md` | Approved |
| 5 | Design | `docs/superpowers/specs/2026-04-28-mempool-phase5-design.md` | Approved |
| 5 | Plan | `docs/superpowers/plans/2026-04-28-mempool-phase5-plan.md` | Complete |
| 6 | Design | `docs/superpowers/specs/2026-04-29-mempool-phase6-design.md` | Approved |
| 6 | Plan | `docs/superpowers/plans/2026-04-29-mempool-phase6-plan.md` | Complete |

## 7. Out of Scope (V2 and Beyond)

These items are explicitly deferred:

- **Local daemon** for offline writing or automated build/pin
- **Three-tier display** (confirmed / pending / mempool — would require live diff between bundle ledger and bundle source repo's live state)
- **Pre-aggregated mempool manifest** for read performance
- **Image / binary upload** via UI
- **ENS DNSLink automation** (CID still requires manual on-chain or DNS update)
- **Private mempool** (would need either auth-gated GitHub repo + browser token handling, or a daemon)
- **Mobile-specific UX optimizations** (works through existing GitHub flow; no special handling)

Adding any V2 item to V1 requires updating §3 anchors, §4 phase plan, and the affected phase's design doc.

## 8. Acceptance — V1 as a Whole

V1 is complete when each phase's acceptance criteria are met *and*:

1. I can compose a draft entirely in the deployed site, see it in the mempool, and edit it without leaving the browser. Promotion happens at the local terminal via `websh-cli mempool promote` followed by `git push` + `just pin` — the same publish ritual the deploy step already requires.
2. The deployed `/ledger` page renders mempool above chain, both filtering correctly by category.
3. Promotion is a single git commit on the bundle source plus an optional best-effort mempool drop. Both surfaces have explicit failure recovery documented in Phase 5 §3.2 / §3.3.
4. Reviewer agent has cleared each phase with no outstanding CRITICAL or HIGH findings.

## 9. Open Questions

Resolved before plan execution:

- ✅ Mempool repo: `0xwonj/websh-mempool`
- ✅ Modal preview: reuse existing `Reader` component as-is for V1 (revisit if breadcrumb path leakage is jarring)
- ✅ Author-mode compose-button slot: reserve in V1 layout, render conditionally from Phase 2

Open / will resolve in per-phase design docs:

- Phase 2: where exactly does the author-mode signal live? AppContext or component-local?
- Phase 3: error UX for partial-failure (add succeeded, delete failed). Banner? Toast? Modal?

## 10. Decision Log

Captured chronologically. Append-only.

| Date | Decision | Reference |
|---|---|---|
| 2026-04-28 | Mempool storage = separate GitHub repo (vs same repo with status filter, vs path-based drafts) | §3.1 |
| 2026-04-28 | Mount path `/mempool` (vs `/mnt/mempool`) | §3 A2 |
| 2026-04-28 | Phases 1+2+3 = V1 (vs V1 = Phase 1 only) | §1 |
| 2026-04-28 | Per-phase design + plan + reviewer workflow adopted | §5 |
| 2026-04-28 | Phase 1 (read-only) complete: 12 tasks shipped, reviewer findings (1 Critical, 2 Important) addressed in 2 fix commits | §4 |
| 2026-04-28 | Phase 2 (authoring) complete: ComposeModal + save flow + author-mode wiring shipped across 4 feat commits + 1 test commit; reviewer findings (1 Critical slug-collision, 2 High YAML/edit-fetch) closed in `9bd0d06` along with priority/tags validators and Esc/Cmd-S shortcuts | §4 |
| 2026-04-28 | Phase 2 visual QA (design §8.3) **skipped** — `0xwonj/websh-mempool` repo not yet provisioned (HTTP 404 from api.github.com). Mempool section renders empty state when mount scan returns zero entries, so the live UX cannot be exercised until the repo exists. Automated coverage (478 tests + wasm/host typechecks) stands; manual QA deferred to first natural opportunity (Phase 3 promote flow needs the repo too) | §4 |
| 2026-04-28 | Phase 3 (promotion) complete: pure helpers + apply_commit_outcome bookkeeping (closes Phase 2's stale `remote_heads` bug) + two-commit promote orchestration with partial-failure recovery + PromoteConfirmModal + per-item Promote button + LedgerPage banners + 12-test integration suite, shipped across 6 feat/test/fix commits + 1 docs commit. Reviewer findings (Esc handler, backdrop-while-running guard, page banner timing, dead variants) closed in `69de9c0`. Visual QA skipped per user direction; deferred to first natural opportunity together with Phase 2 once `0xwonj/websh-mempool` is provisioned. V1 `draft → mempool → promote → deploy → block` loop is now closed end-to-end pending live validation. | §4 |
| 2026-04-28 | Phase 4 (hardening) implemented after live-QA exposed five gaps: (1) `backend_for_path` silently fell back from `/mempool` to `/` when mempool wasn't registered, sending two compose drafts to the bundle source repo by accident; (2) freshly-provisioned GitHub mounts had no `manifest.json`, blocking the runtime's first scan; (3) `save_compose` skipped runtime reload, leaving the new entry invisible until manual page reload; (4) bundle source manifest could be stale in committed history (root cause of #1); (5) no CLI surface existed for setting up new mounts. Six commits land: 404-tolerant scan, strict `backend_for_mount_root` for writes, `save_compose` runtime reload, Trunk `pre_build` manifest hook, `cargo run --bin websh-cli -- mount init`, plus this design doc. Live QA pending — user will run `mount init` against the existing empty `0xwonj/websh-mempool` repo and exercise the full compose → promote loop. | §4 |
| 2026-04-28 | Phase 4 live-QA chain — 4 follow-on commits closed authentication, caching, and stale-state issues that surfaced during browser promote/edit testing: (a) `gh` config symlink repaired, mempool-repo bootstrapped via `mount init`; (b) GitHub backend's `expected_head=None` chicken-and-egg fixed by fetching the branch HEAD before commit (`e59054e`); (c) `RequestCache::NoCache` added to manifest scan (`3fc9404`); (d) scan switched from raw.githubusercontent.com to authenticated `api.github.com` Contents API to bypass CDN propagation (`fe9ad4a`, `4754ceb`). | §4 |
| 2026-04-28 | Phase 5 (CLI promote) — architectural pivot. Live-QA of Phase 3's browser promote exposed four structural issues: cross-repo non-atomicity (~150 lines of `PartialFailure` recovery existed only to paper over the lack of a GitHub cross-repo transaction); bundle-source PAT in browser session; ledger/manifest/attestations not refreshed until deploy; truth-boundary violation (browser reaching across into the bundle source on every promote). Pivot moves promote to the CLI: `websh-cli mempool {list, promote, drop}`. Browser keeps compose/edit. Anchor revisions: A6 (CLI-only promote), A8 (CLI legitimately part of V1). 9 implementation commits land: scaffolding, shared `gh` helper extraction + `read_mempool_mount_declaration`, `list`, `promote`, `drop` (including manifest-update + blob-delete to keep the mempool repo consistent), browser-side deletion (~1232 lines net deletion: `promote.rs`, `promote.module.css`, `tests/mempool_promote.rs`, modal + banners + signals from `LedgerPage`), polish, plus master plan + Phase 5 design + plan docs. Reviewer findings closed in this batch: design (2 CRITICAL + 5 HIGH), plan (1 CRITICAL + 3 HIGH), implementation (2 HIGH — rollback ordering + master-plan completion). Live QA: `mempool promote --path talks/test-3.md --no-attest` → single local commit, then reverted as smoke-test artifact; `mempool drop` cleaned up the mempool repo's manifest + blob. V1 `compose (browser) → drop/promote (CLI) → deploy` ritual closed end-to-end. | §4 |
| 2026-04-29 | A5 dropped — mempool items now use URL navigation; mempool paths exposed in URL bar. Original A5 hid mempool paths to avoid URL exposure, but A1 already declared the mempool repo public, so the privacy framing was incoherent. URL-driven flows enable bookmarking, refresh-during-edit, and browser-back from edit. | §3 A5 |
| 2026-04-29 | A9 added — reserved URL prefixes (`/`, `/ledger`, `/websh`, `/explorer`, `/new`, `/edit/`) recorded as load-bearing constraint. Phase 6 reserves `/new` and `/edit/` at the URL layer; future content/mempool entries that produce one of these top-level URL segments would be unreachable. | §3 A9 |
| 2026-04-29 | Phase 6 (Reader-Unified Mempool UI) — modal-free authoring. Three URL-distinct flows replace the two modals: `/#/<path>` (view, unchanged), `/#/edit/<path>` (edit), `/#/new` (compose). `MempoolEditorPage` (router-mounted) hosts both edit and new; `MempoolEditor` (inner) is the un-modal'd compose form. `Reader` deleted (only consumer was `MempoolPreviewModal`); `MempoolPreviewModal` deleted; `ComposeModal` component deleted (compose helpers retained). Edit affordance from a viewed mempool entry surfaces via the existing `SiteChromeActions` slot on `RendererPage`, gated on author-mode + `/mempool/` path. Net diff: ~ -1100 lines (Reader is the bulk). 6 implementation commits across phases A–F land + 1 reviewer-fix commit. Reviewer findings closed in this batch: design pass 1 (4 HIGH + 4 MEDIUM + 4 NIT), design pass 2 (2 CRITICAL + 2 HIGH + 2 MEDIUM + 2 NIT), plan pass (1 BLOCKING + 4 IMPORTANT), interim implementation review after C5 (0 CRITICAL + 2 MEDIUM + 4 LOW + 3 NIT — 2 MEDIUM + 1 NIT addressed in `e801e9d`). | §4 |
