# Reader Redesign — Master Plan

**Date:** 2026-04-30
**Status:** Active
**Reviewers:** _self_

This is the **single entry point** for the Reader visual redesign. Every phase begins by re-reading this document. Goals, constraints, architecture, and per-phase scope all live here; phase-specific designs and plans extend, never contradict, this file.

---

## 1. Goal

Apply the visual language captured in the root-level prototype (`render.html`, `render-app.jsx`, `render.css`) to the existing Leptos `Reader` component, **without** introducing the prototype's hard-coded palette, mock metadata, or speculative features.

When the redesign is done:

- Reader pages share the archive look already shipped on the homepage.
- Theme switching keeps working — the redesign references the existing semantic / theme tokens (`--bg-primary`, `--text-*`, `--border-*`, `--surface-tint`, `--accent`, `--terminal-green`, `--terminal-yellow`, `--archive-bar-bg`) directly. No new palette is added; no per-page `--reader-*` alias layer is introduced.
- Each `ReaderIntent` variant has a clear, layered chrome (identifier · title · meta table · body · attestation footer).
- `Reader` is split into focused per-intent files (target ≤ 200 lines each).
- Every primitive that already exists (`SiteChrome`, `MetaTable`/`MetaRow`, `AttestationSigFooter`, `MarkdownView`) is reused in place — no parallel re-implementations.

## 2. Non-goals (explicit)

The prototype contains features we are **deliberately not porting** at this time. Each is recorded so future contributors don't re-introduce them by accident:

| Prototype feature                       | Decision                                                                 |
|-----------------------------------------|--------------------------------------------------------------------------|
| Hard-coded `--ink` / `--accent` / `--hex` / `--amber` palette | **Reject.** Reference the existing semantic / theme tokens directly so theming continues to work. |
| Mock metadata (sha · words · signed-by inside meta table) | **Reject.** Use only data the runtime already has (`FileMeta`, `ReaderIntent`). Empty rows are omitted. |
| Split mode (viewer + editor side-by-side) | **Defer.** No `ReaderMode::Split`. View ↔ Edit only.                    |
| Tweaks panel (runtime layout selector)  | **Reject permanently.** Prototype-only debug surface.                    |
| PDF artifact view (thumbnail grid + ASCII TOC) | **Reject.** Iframe stays. The prototype's `Page layout` heading is replaced with a generic `Document` heading. |
| Custom sig-chip + popover variant       | **Reject.** Reuse `AttestationSigFooter` as-is.                          |
| Code renderer (syntax highlighting)     | **Defer.** `.rs`/`.py`/etc. stay on `ReaderIntent::Plain`.               |
| Hex/binary fallback renderer            | **Defer.** No new intent variant for binaries.                           |
| Append banner (post-save toast)         | **Defer.** Out of scope for this redesign.                               |

These are not "do later" backlog items unless explicitly re-opened in §6 Decision Log.

## 3. Design Constraints

These were established with the user before this master was written. They override any contrary signal from the prototype.

1. **Tokens** — reference the existing project tokens **directly**, at the semantic / theme layer (`--bg-primary`, `--bg-secondary`, `--bg-elevated`, `--bg-inset`, `--text-primary`, `--text-dim`, `--text-muted`, `--border-subtle`, `--border-muted`, `--surface-tint`, `--archive-bar-bg`, `--accent`, `--accent-muted`, `--terminal-green`, `--terminal-yellow`). These are the actual cross-page shared tokens — defined in `assets/themes/<theme>.css`, overridden per theme. **Do not introduce a `--reader-*` alias layer.** The `--home-*` and `--ledger-*` alias bundles in those modules are local conventions, not the shared system; mirroring them would just create a third identical alias bundle. Theming must continue to apply automatically.
2. **Metadata** — only what the runtime already exposes. Concretely: `FileMeta` (manifest-driven: `description`, `size`, `modified`, `date`, `tags`) plus what the `ReaderIntent` itself carries (kind, media type). When a value is missing, **omit the row** rather than show a placeholder.
3. **Footer** — `AttestationSigFooter` stays exactly as it is. Sha-256 / signed-by appear **only** in the footer chip, never duplicated in the meta table.
4. **PDF** — iframe rendering is unchanged. The prototype's `Abstract` block reads from `FileMeta.description` (rendered as a section, not a meta row). The prototype's `Page layout` heading becomes `Document`.
5. **Toolbar** — footnote-mark style only (`* mode  rendered · edit ⌘S    ● synced`). No tabs / pill / kbd-only variants.
6. **Reuse over re-create** — if a component with equivalent semantics exists, reuse it. New components only when no existing one fits.
7. **No split view, no tweaks panel, no code/hex renderer.** (Re-statement of §2.)

## 4. Architecture

### 4.1 Component tree (final shape)

```
Reader (mod.rs — routing, View/Edit toggle, draft state, save dispatch)
├─ SiteChrome                      (reused — site-scoped chrome)
└─ <main class="page">
   ├─ Ident                        (paper id + revision strip — small new component)
   ├─ TitleBlock                   (h1 + MetaTable; per-intent row policy in §4.4)
   ├─ ReaderToolbar                (footnote-mark style; mempool author-mode only)
   ├─ ErrorBanner / Loading        (existing primitives)
   ├─ {one of}
   │   ├─ MarkdownReaderView       (paper layout wrapper around MarkdownView)
   │   ├─ MarkdownEditorView       (vim-style textarea — line gutter + dirty/clean badge)
   │   ├─ HtmlReaderView           (sanitized HTML insertion via MarkdownView)
   │   ├─ PlainReaderView          (<pre class="rawText">)
   │   ├─ PdfReaderView            (TitleBlock + optional Abstract section + iframe)
   │   ├─ AssetReaderView          (image <figure>)
   │   └─ Redirecting              (placeholder text — same as today)
   └─ AttestationSigFooter         (reused — unchanged)
```

### 4.2 File layout (final shape — star marks new files)

```
src/components/
  reader/
    mod.rs              ← state, dispatch, save (currently ~466 lines → target ~200)
    intent.rs           (unchanged)
    meta.rs       ★    (file_meta_for_path helper — Reader's view into FileMeta)
    title_block.rs ★    (Ident + h1 + MetaTable; per-intent row policy)
    toolbar.rs    ★    (footnote-mark ReaderToolbar + ⌘S/r/e keybindings)
    views/
      markdown.rs ★    (MarkdownReaderView + MarkdownEditorView)
      html.rs     ★
      plain.rs    ★
      pdf.rs      ★    (TitleBlock + Abstract + iframe wrapper)
      asset.rs    ★    (image)
      redirect.rs ★
    reader.module.css   (rewritten to archive look using --home-* tokens)
  shared/
    file_meta.rs  ★    (FileMeta moved out of explorer/preview/hook.rs)
    …
```

### 4.3 Data flow

- **Path → FileMeta**: `shared::file_meta_for_path(ctx, &path)` reads `ctx.view_global_fs` and projects the `FsEntry::File` row into `FileMeta`. Implementation moves verbatim from `explorer/preview/hook.rs`. Both the explorer preview and the reader call into this single helper.
- **ReaderIntent → row policy**: each intent variant has a deterministic list of rows (§4.4). `TitleBlock` consumes `(intent, FileMeta)` and renders only the rows whose values are non-empty.
- **Attestation**: `AttestationSigFooter` continues to read from `AttestationArtifact::from_homepage_asset()` keyed on `attestation_route_for_node_path(&canonical_path)`. **Reader does not duplicate this read** — sha and signer live in the footer only.
- **Draft / save**: existing `RwSignal<draft_body>` / `draft_dirty` / `saving` / `save_error` / `refetch_epoch` pattern is preserved verbatim. Toolbar wiring changes; semantics don't.

### 4.4 Per-intent meta table policy

Rows are listed in display order. **A row whose value is empty / `None` / zero is not rendered.** No placeholders, no `—`.

| Intent     | Rows                                                        | Section below table |
|------------|-------------------------------------------------------------|---------------------|
| Markdown   | type · modified · date · tags                               | (rendered body)     |
| Html       | type · modified                                             | (rendered body)     |
| Plain      | type · size · modified                                      | (rendered body)     |
| Pdf        | type (`application/pdf`) · size · modified · tags           | `<h2>Abstract</h2>` from `FileMeta.description` (omitted if empty) → `<h2>Document</h2>` + iframe |
| Asset(img) | type (`image/*`) · size · modified · description (caption)  | (figure + img)      |
| Redirect   | (no meta table — redirect happens immediately)              | redirecting message |

Notes:
- `type` value renders as a `.tag` chip plus optional dim suffix (e.g. `markdown   UTF-8 · CommonMark`).
- `tags` renders as multiple `.tag` chips.
- `modified` is `format_date_iso(meta.modified / 1000)` when present.
- `description` for assets renders as a single dim caption span next to the value, not as a separate section.

## 5. CSS strategy

### 5.1 Token layering — what's actually shared

```
assets/tokens/primitive.css        Tier 1 — raw values (space, font-size, z-index, …)
assets/tokens/semantic.css         Tier 2 — role-based, theme-agnostic (pad-*, gap-*, content-width-*)
assets/themes/<theme>.css          Tier 3 — colors / surfaces — the real shared theming surface:
                                            --bg-primary  --bg-secondary  --bg-elevated  --bg-inset
                                            --text-primary  --text-dim  --text-muted
                                            --border-subtle  --border-muted
                                            --surface-tint  --archive-bar-bg
                                            --accent  --accent-muted
                                            --terminal-green  --terminal-yellow  (plus cyan/red/purple/orange)
home.module.css / ledger_page.module.css   Local alias bundles (`--home-*`, `--ledger-*`) over
                                            Tier 3, used only inside that module. These are
                                            convenience, not infrastructure.
```

The `--home-*` and `--ledger-*` bundles map 1:1 onto the same Tier-3 tokens (e.g. `--home-bg = --ledger-bg = var(--bg-primary)`). They are page-local readability sugar, not the shared design system.

### 5.2 Reader's choice

- All new reader styles live in `src/components/reader/reader.module.css` (rewritten in Phase 2).
- Reader CSS references **Tier-3 tokens directly** — `var(--bg-primary)`, `var(--text-dim)`, `var(--border-subtle)`, `var(--surface-tint)`, `var(--terminal-green)` (the prototype's `--hex`), `var(--terminal-yellow)` (the prototype's `--amber`), `var(--archive-bar-bg)`, etc.
- **No `--reader-*` alias bundle** is introduced. Reader does not borrow `--home-*` either — that bundle is home-local.
- Body container: `max-width: var(--content-width-default, 50rem)` or a `92ch` literal, mirroring Home. Font family: `var(--font-mono)`.
- Class names use stylance auto-scoping (e.g. `css::metaTable`, `css::titleBlock`); no `:global()` except where wrapping `MarkdownView`-injected HTML.
- Typography for the markdown body parallels Home's `.home p / ul / ol / h1 / h2` rules. Selectors are local; values come from Tier-3 tokens directly so the look stays consistent across home / reader / ledger by virtue of the shared theme, not by sharing rules.

### 5.3 Future consolidation (Phase 4 candidate)

If after Phase 3 the home / reader / ledger modules each maintain identical alias bundles, the right move is **not** to add a fourth (reader's) — it's to extract a single archive-scope bundle (`--archive-bg`, `--archive-ink`, …) into `src/components/shared/archive.module.css` and migrate the three call sites. This is explicitly Phase 4's optional scope (§6).

## 6. Phase Plan

Sequential. Each phase is its own self-contained PR; the next phase begins only after the previous phase's review clears.

| # | Title | Outcome | Status |
|---|---|---|---|
| 1 | Bedrock — `FileMeta` shared | Move `FileMeta` from `explorer/preview/hook.rs` to `shared/`; add `file_meta_for_path` helper. No behavior change. | **Complete** |
| 2 | Reader look (View only) | Rewrite `reader.module.css` to archive look on Tier-3 tokens; split `reader/mod.rs` into `views/*` + `title_block.rs` + `meta.rs`; per-intent meta tables; PDF abstract section. | **Complete** |
| 3 | Footnote toolbar + keybindings | Replace `ReaderToolbar` with footnote-mark variant; add ⌘S / r / e shortcuts; integrate dirty/saving status. | Pending |
| 4 | Archive alias consolidation (optional) | If `--home-*` / `--ledger-*` / reader-side alias usage clearly converges, extract a single `--archive-*` bundle into `shared/archive.module.css` and migrate home / reader / ledger. Skip if Reader's direct Tier-3 usage proved sufficient. | Pending |

### Per-phase scope

#### Phase 1 — Bedrock

- **In:** Move `FileMeta` (and its small impl block) from `src/components/explorer/preview/hook.rs` to `src/components/shared/file_meta.rs`. Re-export from `shared::mod`. Update `explorer/preview/hook.rs` to import from the new location. Add `pub fn file_meta_for_path(ctx: AppContext, path: &VirtualPath) -> Option<FileMeta>` extracted from the `Signal::derive` body in `use_preview()`.
- **Out:** No reader changes. No CSS. No behavior change anywhere.
- **Acceptance:** `cargo test --lib`, `cargo check --target wasm32-unknown-unknown --lib`, `trunk build` all green. Explorer preview behavior unchanged (smoke test in trunk serve).
- **Files touched:** `src/components/explorer/preview/hook.rs`, `src/components/shared/file_meta.rs` (new), `src/components/shared/mod.rs`.

#### Phase 2 — Reader look (View only)

- **In:** Rewrite `src/components/reader/reader.module.css` against the Tier-3 theme tokens directly (`--bg-primary`, `--text-*`, `--border-*`, `--surface-tint`, `--archive-bar-bg`, `--accent`, `--terminal-green`, `--terminal-yellow`). Split `src/components/reader/mod.rs` into `mod.rs` (state + dispatch only), `meta.rs`, `title_block.rs`, and `views/{markdown,html,plain,pdf,asset,redirect}.rs`. Implement per-intent row policy from §4.4. Add `Ident` strip (paper id + rev — values come from `RouteFrame` / `attestation_route` if available; otherwise omit). Add Pdf `Abstract` section reading `FileMeta.description`; replace `Page layout` heading with `Document`.
- **Out:** Toolbar still uses the old layout (Phase 3). No keybindings. No split. No new metadata sources beyond `FileMeta` + intent.
- **Acceptance:** `cargo test --lib`, `cargo check --target wasm32-unknown-unknown --lib`, `trunk build` all green. Manual QA via `trunk serve` across at least: `/index.md` (markdown), `/x.html` (html), `/x.txt` (plain), `/papers/x.pdf` (pdf with description), `/cover.png` (image). Theme switching via the palette picker visibly cycles reader colors.
- **Files touched:** `src/components/reader/{mod.rs, meta.rs, title_block.rs, views/*.rs, reader.module.css}`.

#### Phase 3 — Footnote toolbar + keybindings

- **In:** Replace inline `ReaderToolbar` in `mod.rs` with `src/components/reader/toolbar.rs`. Footnote-mark style only. Wire keybindings: `⌘S` (save when in Edit), `r` (View), `e` (Edit), with text-area-aware suppression. Surface `saving` / dirty state in the same row.
- **Out:** No new toolbar variants. No split mode. No append banner.
- **Acceptance:** `cargo test --lib`, `cargo check --target wasm32-unknown-unknown --lib`, `trunk build` all green. Manual QA: `/new` route enters Edit; `r`/`e` toggle; `⌘S` saves and clears dirty; toolbar is hidden for non-author or non-mempool paths.
- **Files touched:** `src/components/reader/{mod.rs, toolbar.rs, reader.module.css}`.

#### Phase 4 — Archive primitive extraction (optional)

- **In:** If the same `.metaRow` / `.tag` / `.sectionTitle` patterns plus the duplicated `--home-*` / `--ledger-*` alias bundles converge across home + reader + ledger, extract a single `--archive-*` token bundle and the shared rules into `src/components/shared/archive.module.css`, migrating the three modules to consume it. If duplication is shallow or Reader's direct Tier-3 usage didn't push the other modules toward an alias, **skip this phase** and record the skip in the Decision Log.
- **Acceptance:** Same as above; no visual regression; Home / Reader / Ledger pixel-identical to before.
- **Files touched:** `src/components/shared/archive.module.css` (new), `src/components/{home,reader,ledger_page}.module.css`, callers.

## 7. Per-Phase Workflow

Every phase follows this exact sequence. **Skipping or reordering steps undermines the safety the structure provides.**

```
┌─ Phase Start ────────────────────────────────────────────────────────┐
│  1. Re-read this master plan                                         │
│  2. Write design doc:                                                │
│       docs/superpowers/specs/2026-04-30-reader-redesign-phase<N>-    │
│         design.md                                                    │
│       — scope · types · components · CSS · tests · acceptance        │
│       — self-review (placeholders · contradictions · scope creep)    │
│       — get user approval                                            │
│  3. Write implementation plan:                                       │
│       docs/superpowers/plans/2026-04-30-reader-redesign-phase<N>-    │
│         plan.md                                                      │
│       — concrete steps · file changes · tests · risks                │
│       — get user approval                                            │
│  4. Implement per plan, mark each step complete as it ships          │
│  5. Local verification:                                              │
│       cargo test --lib                                               │
│       cargo check --target wasm32-unknown-unknown --lib              │
│       trunk build (visual QA via trunk serve when applicable)        │
│  6. Invoke superpowers:code-reviewer on the change                   │
│       — pass: master + phase design + phase plan + diff              │
│       — address all CRITICAL / HIGH findings                         │
│  7. Update §6 status → Complete; append entry to §10 Decision Log    │
│  8. Commit the phase (code + design + plan + master update) as one   │
│     conventional-commit-style commit                                 │
│  9. Begin next phase from step 1                                     │
└──────────────────────────────────────────────────────────────────────┘
```

### 7.1 Workflow rules

- **Read this master first, every phase.** It is the only authoritative source for goals, constraints, and architecture.
- Do not start a new phase before the previous phase's review has cleared (no outstanding CRITICAL/HIGH findings).
- Do not write code before the phase's design and plan have user approval. The design + plan are the contract.
- Reviews run at the end of each phase, not mid-implementation. Mid-flight reviews waste cycles on incomplete code.
- **One commit per phase.** Each phase delivers an independently-revertable, green-build slice. The commit message follows the repo convention (`feat:` / `refactor:` / etc.) and lists the phase number.
- If §1 goal, §3 constraints, or §6 phase plan shifts mid-flight, update this master **first**, then update the affected phase's design doc.
- If the prototype tempts a feature beyond §2 Non-goals, the answer is no unless the user re-opens the decision in §10.

## 8. Document Index

Accumulates as phases progress. Append a row when a new artifact lands.

| Phase | Artifact | Path | Status |
|---|---|---|---|
| Master | This file | `docs/superpowers/specs/2026-04-30-reader-redesign-master.md` | Active |
| 1 | Design | `docs/superpowers/specs/2026-04-30-reader-redesign-phase1-design.md` | Approved |
| 1 | Plan | `docs/superpowers/plans/2026-04-30-reader-redesign-phase1-plan.md` | Complete |
| 2 | Design | `docs/superpowers/specs/2026-04-30-reader-redesign-phase2-design.md` | Approved |
| 2 | Plan | `docs/superpowers/plans/2026-04-30-reader-redesign-phase2-plan.md` | Complete |

## 9. Reusable primitives — do not re-implement

When a phase's design names one of these, point to it; do not create a parallel.

| Primitive                       | Location                                              | Use for                                  |
|---------------------------------|-------------------------------------------------------|------------------------------------------|
| `SiteChrome`                    | `src/components/chrome/mod.rs`                        | Top archive bar (identity · breadcrumb · nav · palette) |
| `MetaTable` / `MetaRow`         | `src/components/shared/meta_table.rs`                 | The `meta-tbl` row block in `TitleBlock` |
| `AttestationSigFooter`          | `src/components/shared/signature_footer.rs`           | The page-foot sig chip + popover         |
| `MarkdownView` / `InlineMarkdownView` | `src/components/markdown.rs`                    | Sanitized HTML insertion + math hydration |
| `FileMeta` (after Phase 1)      | `src/components/shared/file_meta.rs`                  | Manifest-driven file metadata projection |
| Tier-3 theme tokens             | `assets/themes/<theme>.css` (per-theme overrides)     | All colors / surfaces / rules / tints — `--bg-*`, `--text-*`, `--border-*`, `--surface-tint`, `--archive-bar-bg`, `--accent`, `--terminal-*`. Referenced **directly** from reader CSS. |
| `format_date_iso`               | `src/utils/format`                                    | `modified` row formatting                |

## 10. Decision Log

Chronological, append-only.

| Date | Decision | Reference |
|---|---|---|
| 2026-04-30 | Four-phase plan adopted: bedrock (`FileMeta` move) → Reader look (View) → footnote toolbar + keybindings → optional primitive extraction. | §6 |
| 2026-04-30 | Design tokens: reference Tier-3 theme tokens (`--bg-primary`, `--text-*`, `--border-*`, `--surface-tint`, `--archive-bar-bg`, `--accent`, `--terminal-green`, `--terminal-yellow`) directly from `reader.module.css`. Reject the prototype's `--ink/--accent/--hex/--amber` palette and reject introducing a `--reader-*` alias bundle. The `--home-*` / `--ledger-*` bundles are local readability sugar over the same Tier-3 tokens; reader does not borrow them. Theming continues to work without per-route overrides. | §3, §5 |
| 2026-04-30 | Initial draft of this master used the phrasing "reuse `--home-*` tokens", which conflated home's local alias bundle with the actual shared system. Corrected throughout: §1 / §2 / §3 / §5 / §6 Phase 2 / §6 Phase 4 / §9 / §10. The shared system is the Tier-3 theme tokens; the `--home-*` bundle is home-local. | §3, §5 |
| 2026-04-30 | Phase 1 complete. `FileMeta` (struct + 3-method impl) moved verbatim from `explorer/preview/hook.rs` to `shared/file_meta.rs`; new `file_meta_for_path(ctx, &path) -> Option<FileMeta>` helper extracts the projection body so `use_preview` becomes a one-liner. **Deviation from design §3**: `FileMeta` re-export in `preview/mod.rs` was dropped entirely rather than kept — grep confirmed zero external consumers, so the retain-for-compat clause was cargo-cult. The one internal consumer (`preview/content.rs`) was updated to import from `shared` directly. 525 tests passing; cargo + wasm32 + trunk all green. Reviewer cleared with no CRITICAL/HIGH/MEDIUM; one LOW addressed (`use leptos::prelude::*` aligned with sibling shared modules); one LOW deferred (file-level doc comment mentions Reader as a consumer — becomes accurate at Phase 2). | §6, §8 |
| 2026-04-30 | Phase 2 complete. Archive look applied to Reader's view paths. New modules: `reader/{meta.rs, title_block.rs, views/*}`. `reader.module.css` rewritten against Tier-3 tokens (no `--ink/-hex/-amber/-paper/-chrome` anywhere); CSS file moved from sibling `src/components/reader.module.css` to colocated `src/components/reader/reader.module.css`. `RendererContent` enum split into `Markdown / Html / Text / Pdf / Image / Redirecting` so the dispatcher can pick the right view component without re-parsing media types. **Deviation from master §4.4**: `Modified` and `Date` collapsed into a single `Date` row (prefers `meta.date`, falls back to `modified_iso`) — two-row arrangement was awkward when they conflict and redundant when they don't. **Deviation from design**: `mod.rs` is 470 lines (target ≤ 250); the bulk is event-handler / save-flow logic that Phase 3's toolbar redesign will partly absorb (toolbar markup will move to its own file). 540 tests passing (525 baseline + 15 new in `meta.rs` and `title_block.rs`); cargo + wasm32 + trunk all green. Reviewer cleared with no CRITICAL/HIGH; three MEDIUMs addressed inline (image-asset Tags row narrowed to PDF-only; dead `media_type` field on `RendererContent::Image` removed; stylance constant warning storm eliminated by consolidating the macro to a single `pub(crate) css` module in `mod.rs` consumed via `crate::components::reader::css` — the `import_crate_style!` macro accepts `#[allow(dead_code)] pub(crate) ident, path` directly, no wrapper module needed); two LOWs addressed (unreachable `path.is_empty()` guard in `Ident` removed; `<Show>` skips `Ident`+`TitleBlock` for `Redirect` intent); one LOW left as-is (`data-n=""` cosmetic forward hook for numbered sections). Plan-vs-implementation note: plan step 1 said `format_date_iso(ts as i64 / 1000)` but implementation correctly calls `format_date_iso(ts)` directly — `meta.modified` is already in seconds (consistent with `ledger_page.rs` and `explorer/file_list.rs`). | §6, §4.4, §8 |
| 2026-04-30 | Metadata source: `FileMeta` (manifest) + `ReaderIntent` (kind / media type) only. Empty rows are omitted. Sha and signer live exclusively in the footer chip. | §3, §4.4 |
| 2026-04-30 | Split mode, tweaks panel, code renderer, hex fallback, custom sig chip, append banner — all out of scope. PDF stays as iframe; abstract = `FileMeta.description`. | §2 |
| 2026-04-30 | Toolbar style: footnote-mark only. Other prototype variants (tabs / pill / kbd-only / bracket / colon / prose / minimal) are not ported. | §3, §6 Phase 3 |
| 2026-04-30 | Per-phase commits (one per phase) at user-style request, in contrast to the prior render-pipeline refactor's accumulate-then-commit policy — each phase delivers a user-visible slice that benefits from being individually bisectable / revertable. | §7 |

## 11. Acceptance — redesign as a whole

- Every phase's acceptance criteria are met.
- `cargo test --lib`, `cargo check --target wasm32-unknown-unknown --lib`, and `trunk build` all green.
- Switching the palette picker visibly retones the reader, with no per-theme overrides in `reader.module.css`.
- Reader content for every `ReaderIntent` variant renders without blank rows or placeholder dashes.
- `code-reviewer` has cleared every phase with no outstanding CRITICAL or HIGH findings.
- `git log --oneline` shows one commit per phase with a clear conventional-commit message.

## 12. State

- **Active phase:** Phase 3 — pending design doc.
- **Last updated:** 2026-04-30 (after Phase 2 commit)
