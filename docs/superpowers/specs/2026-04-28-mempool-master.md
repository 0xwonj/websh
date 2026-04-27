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
4. **Promote** a draft from the mempool to the canonical chain, which translates to a two-commit transaction (delete from mempool repo, add to bundle source repo).
5. **Deploy** is still a manual step (existing `just pin` / trunk build flow); after deploy, the promoted entry becomes a confirmed block on the IPFS-anchored ledger.

V1 closes the loop "draft → mempool → promote → deploy → confirmed block" entirely from the static-site UX, with no local daemon and no server.

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
| A5 | Mempool item click → modal preview, not URL navigation | Avoids exposing `/mempool/...` paths in the URL bar; matches the `ledger.html` interaction model |
| A6 | Promotion is a two-commit transaction (delete from mempool repo, add to bundle source repo) | Existing infra; explicit deploy step preserved; failure modes are bounded |
| A7 | Local daemon is V2, not V1 | GitHub commit covers V1 needs; daemon adds value only for offline / automated-deploy use cases |
| A8 | No CLI work in V1 | Mempool is purely runtime-fetched. Single small touch is lifting `iso_date_prefix` from `cli/ledger.rs` to `utils::format` so wasm and host can share it. |

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
| 3 | Promotion | Promote button on mempool item, two-commit transaction, deploy hint banner | **In Design** |

After Phase 3, V1 is complete. V2 items (§7) are queued separately.

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
| 3 | Design | _(to be added)_ | — |
| 3 | Plan | _(to be added)_ | — |

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

V1 is complete when *all three* phase acceptance criteria are met *and*:

1. I can compose a draft entirely in the deployed site, see it in the mempool, edit it, and promote it — without any terminal interaction beyond `just pin` for the final IPFS deploy.
2. The deployed `/ledger` page renders mempool above chain, both filtering correctly by category.
3. The two-commit promotion is documented with a partial-failure recovery path.
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
