# Migration Workflow

Defines the execution loop. Every change during the migration goes through this loop.

## Per-phase artifacts

Each phase produces three documents in addition to its commits and ADR:

```
docs/refactor/3-crate-workspace/
├── phases/
│   ├── B-websh-core.md      design doc — the "what" for this phase, post-recon
│   ├── B-review.md          multi-perspective review consolidation
│   └── B-tasks.md           agreed task breakdown driving the per-task loop
```

Phase letters: `A`, `B`, `C`, `D`, `E`, `F`. The design doc is written **after recon and Plan-agent findings**, **before** any code moves. The review doc captures the multi-perspective review that validates the design. The tasks doc decomposes the agreed design into the concrete units the execution loop walks.

## Phase structure

Six phases (G is held):

```
A — Workspace skeleton
B — websh-core populated
C — websh-cli populated + engine extraction
D — websh-web populated + UI consolidation
E — Trunk + asset paths working
F — Docs + repository hygiene
```

Each phase ends with at least one commit on `refactor/3-crate-workspace`. Granular commits within a phase are preferred over phase-level squashes.

## Per-phase loop

```
1. RECON
   • Read the relevant section of architecture.md.
   • Re-read conventions.md and principles.md (they apply to every change).
   • Dispatch an Explore or Plan agent to map files and surface gotchas
     against the architecture's plan for this phase.
   • Read the agent's report. Note hard blockers, plan corrections, and
     things that compile cleanly as written.

2. DESIGN
   • Write phases/<letter>-<name>.md — the phase design doc.
     Captures: target layout for this phase, file moves with sources,
     pre-move refactoring needed, deps that must be wired, deviations from
     architecture.md, decisions left for implementation.
   • Cross-reference architecture.md sections; note where this design
     refines or corrects the architecture.

3. CONSENSUS REVIEW (multi-perspective)
   • Dispatch ≥3 review agents in parallel on the phase design doc, each
     with a distinct lens (e.g., architecture/layering, Rust idioms,
     Leptos best practice, code quality, conventions adherence).
   • Read every report.
   • Reconcile conflicts: if reviewers disagree, pick the option that best
     respects the principles in principles.md and document the choice.
   • Update the phase design doc with the agreed adjustments.
   • Write phases/<letter>-review.md summarising the review and
     consolidation outcome.

4. TASK PLAN
   • Decompose the agreed design into concrete sub-tasks.
   • Write phases/<letter>-tasks.md — ordered list, one task per intended
     commit.
   • Mirror to TaskCreate so progress is observable via TaskList.

5. PER-TASK LOOP — repeat until phase tasks done
   a. RECON       Read the touched files. Confirm assumptions still hold.
   b. DECISION    Does the phase design still look optimal?
                  → YES               proceed.
                  → BETTER ALTERNATIVE  weigh, choose, record (see § Deviation rules).
                  → BLOCKED            see § Stop / escalate.
   c. IMPLEMENT   Make the change. Apply conventions.md and principles.md.
   d. VERIFY      cargo fmt + cargo check (relevant targets) + cargo test
                  (touched packages) + cargo clippy (touched targets).
   e. SELF-REVIEW Read the diff. Look for over-extraction, missed pieces,
                  stale comments, dead exports.
   f. COMMIT      Granular, focused commit. Conventional Commits format
                  (see conventions.md). No internal jargon in the message.
   g. MARK DONE   TaskUpdate completed.

6. WRAP-UP REVIEW (multi-perspective; re-run consensus shape)
   • Full verification (see § Wrap-up checklist).
   • Dispatch ≥3 review agents in parallel on the phase's cumulative diff,
     each with a distinct lens. Required perspectives:
       (a) Goal achievement — does this phase deliver what architecture.md
           and the phase design doc promised?
       (b) Principles adherence — every pattern in principles.md applied
           where applicable; every anti-pattern absent.
       (c) Conventions adherence — commit messages, comments, naming,
           file size, error handling per conventions.md.
       (d) Correctness — tests, types, no UB regressions.
   • Reconcile findings; fix anything HIGH or CRITICAL before proceeding.
   • Append to phases/<letter>-review.md (post-implementation section).
   • Update Status table in README.md.
   • Append to deviation-log.md if anything diverged.
   • Write the phase ADR (see § ADRs).
   • Single wrap-up commit if reviewer fixes need consolidation; otherwise
     commits stay granular.
   • Brief status note ("<phase> done. N commits. All checks pass.
     M deviations. K reviewer findings addressed.").
```

## Deviation rules

Default: follow `architecture.md`.

**Deviate when**:
- Empirical evidence contradicts the doc (compile error, dep behavior differs from research, type signature won't permit the planned shape).
- A clearly-better alternative emerges from reading actual code (a simpler structure, a missed Rust idiom, a Leptos pattern the architecture-time research didn't surface).
- An explicit user instruction in chat overrides the doc.

**Don't deviate for**:
- Aesthetic preferences without measurable benefit.
- "Could be cleaner if…" without a concrete improvement.
- Speculative future flexibility (YAGNI).

**When deviating**:
1. Append an entry to `deviation-log.md`. One paragraph, with date, phase, what changed, why.
2. If the deviation is non-trivial (changes a phase's shape, removes a planned feature, swaps a dep, materially affects another phase), open an ADR in `adrs/` describing the choice.
3. Continue execution. Do not block on user confirmation unless the deviation involves user-visible behavior change.
4. At wrap-up, surface deviations briefly in the status note.

## Stop / escalate conditions

Pause and surface to the user when:

- Verification fails and 3 fix attempts haven't resolved it.
- A design assumption proves empirically wrong in a way that changes more than one phase.
- A subsystem turns out substantially larger than anticipated and would balloon the migration scope.
- A user-visible behavior change becomes necessary (a feature must be cut, semantic of an existing feature changes, etc.).

Don't pause for:

- Routine compile errors (fix them).
- Cargo dependency feature mismatches (research and fix).
- Clippy warnings (fix or `#[allow]` with a one-line justification).
- Test failures with an obvious cause (fix them).

## Wrap-up checklist (every phase)

Run, in order:

```
cargo fmt --check                                                 # no formatting drift
cargo check -p websh-core                                         # native target
cargo check -p websh-core --target wasm32-unknown-unknown         # wasm target
cargo check -p websh-cli                                          # CLI compiles
cargo check -p websh-web --target wasm32-unknown-unknown          # web compiles
cargo clippy --workspace --all-targets -- -D warnings             # zero warnings
cargo test --workspace                                            # everything passes
cargo test --features mock --test commit_integration              # mock-feature test
trunk build                                                       # only required at end of E onward
```

(For phases that don't yet involve all crates — A and B — irrelevant rows are skipped, but the rest must pass.)

Beyond the mechanical checks, every wrap-up runs a **three-axis review**:

1. **Correctness** — tests pass, types are sound, no UB introduced (esp. around `unsafe`).
2. **Architecture** — the change respects the layering in architecture.md. No upward dep flow (UI → CLI, CLI → UI, etc.). No engines hiding behind clap shims after Phase C. No ad-hoc duplications of helpers we just consolidated.
3. **Code quality** — readability, naming, file size discipline, comment hygiene per conventions.md, no commented-out code, no dead imports, idiomatic Rust per principles.md.

If any axis fails, fix before committing the phase wrap-up. Don't ship "passing tests + bad layering" — that's exactly what the migration is supposed to prevent.

## Agent usage

Specialized agents are explicitly part of the workflow.

| Agent | When |
|---|---|
| **Explore** (read-only search) | Phase recon. Map files and trace cross-module dependencies. |
| **Plan** (architect) | Phase recon companion. Validate the architecture's plan for this phase against the actual codebase; surface hard blockers and plan corrections. |
| **general-purpose** (research) | When a non-trivial unknown surfaces mid-phase (a crate's wasm compatibility changed, a Leptos API behaves differently than docs suggest). Dispatch with a focused prompt; do not over-delegate. |
| **code-reviewer** | At consensus review (step 3) and wrap-up review (step 6). At least 3 instances per review, each with a distinct lens (architecture / Rust idioms / Leptos / code quality / conventions). |
| **codex:codex-rescue** | Only when stuck (3+ failed attempts) and an outside perspective might unblock. Sparingly. |
| **advisor()** | At Phase B and Phase E wrap-ups, after multi-perspective review consolidation. Once when stuck. Once before declaring the migration done. |

When dispatching, give the agent the architecture/conventions/principles paths and the specific concern. Don't ask agents to "review the code" — give them a structured question and the lens they should apply.

### Multi-perspective review template

Every consensus review (step 3) and wrap-up review (step 6) dispatches at least 3 review agents in parallel. Required lenses:

1. **Goal-achievement / architecture lens** — does this phase deliver against `architecture.md` and the phase design doc? Layering invariants intact? Cross-crate boundaries respected?
2. **Principles + idioms lens** — every pattern from `principles.md` applied where applicable; every anti-pattern absent. Rust + Leptos idioms correct.
3. **Conventions + code-quality lens** — commit messages, comments, naming, file size, error handling per `conventions.md`. Readability, dead code, over-extraction.

Optional fourth lens for high-risk phases:

4. **Correctness lens** — tests, types, no UB regressions, error-path coverage.

Each agent's prompt names the lens explicitly. Reviewers report findings as `CRITICAL` / `HIGH` / `MEDIUM` / `LOW` with file:line references. The orchestrator (you) reconciles conflicts. A `CRITICAL` or `HIGH` finding blocks the phase wrap-up until addressed; `MEDIUM` is fixed when cheap; `LOW` is logged and deferred.

## Pause points (natural handoff opportunities)

These are advisory, not blocking. The loop continues unless the agent explicitly surfaces.

- **End of Phase A** — workspace skeleton compiles. Quick "looks right?" before populating.
- **End of Phase B** — `websh-core` standalone compiles for both targets. Largest risk.
- **End of Phase E** — `trunk serve` works. Migration is functionally complete.
- **Before Phase G** — held by default; revisit only when the user opens it.

## Communication protocol

- **Phase start**: one sentence. "Starting Phase B: populating websh-core."
- **Mid-phase silence is fine**. TaskList is the progress view; commits are the durable record.
- **Surfacing a deviation**: brief paragraph naming the choice and the rationale. Then continue.
- **Phase end**: one sentence. "Phase B done. N commits. All checks pass." Plus a one-line summary of any deviations.
- **Verbose only when surfacing an issue or asking for direction**.

## Tracking

- **TaskCreate** one parent Task per phase; sub-Tasks for major sub-steps.
- **TaskList** is the at-a-glance progress view at any moment.
- **Commits** are the durable record. Subject lines reference what changed (file/feature), not "Phase X."
- **README.md Status table** is updated at every phase boundary commit.
- **deviation-log.md** is append-only and timestamped.
- **adrs/NNNN-…md** is one ADR per phase + one per material deviation.

## ADRs

Every phase produces an ADR. ADRs are short — half a page each. They record: what was decided, what alternatives were considered, what trade-offs were accepted. They do not restate `architecture.md`; they record decisions made *during* execution that the architecture didn't lock down.

Numbering: `NNNN-kebab-case-title.md`, four-digit zero-padded, monotonically increasing.

Phase ADRs are numbered on completion: `0001-workspace-skeleton.md` for Phase A's wrap-up ADR, etc. Mid-phase deviations get their own ADRs interleaved (`0002-…`, `0003-…`).

Template: [adrs/0000-template.md](./adrs/0000-template.md).

## Versioning

The migration is a breaking change. We don't version-bump the project to "v2" or rename anything to `*_v1` / `*_v2`. Old code is replaced, not co-existed. The crate names are `websh-core`, `websh-cli`, `websh-web` — no version suffix.

`Cargo.toml`'s `version` stays at `0.1.0` unless we have a real reason to bump (none for this migration).
