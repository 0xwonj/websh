# WebSH Development Roadmap

Last updated: 2026-04-21

This document is the single entry point for someone picking up WebSH development on a new machine (server, new clone). It summarizes what has shipped, what's queued, and where to look for detail.

---

## Snapshot

**Current main**: Phase 1 + Phase 2 + follow-ups + Phase 3a merged.
**Build**: `cargo build --release --target wasm32-unknown-unknown` — clean.
**Tests**: `cargo test` — **340 pass / 0 fail** (lib). `cargo test --features mock` additionally runs the commit-path integration test.

### Branches preserved in origin

| Branch | Purpose |
|---|---|
| `main` | Active development. Phase 1 + Phase 2 merged. |
| `wip/january-2026-restructure` | **Preserved reference** — the abandoned Jan 2026 write-mode refactor (editor, storage backend, FsState overlay, sync UI). Contains ~5000 lines of candidate code for Phase 3. Not merged; cherry-pick or rewrite as needed. |

Phase boundaries are identifiable from commit messages (`Merge Phase 1: ...`, `Merge Phase 2 Track ...: ...`) — use `git log --oneline --graph --first-parent main` to navigate.

---

## Completed Phases

### ✅ Phase 1 — Core Contracts

Establishes two foundational abstractions the rest of the app relies on.

**Scope:**
- `config::mounts() -> &'static MountRegistry` singleton (`OnceLock`, non-empty invariant).
- `MountRegistry::home()` / `resolve(alias)` / `all()` accessors.
- `CommandResult { output, exit_code: i32, side_effect: Option<SideEffect> }`.
- `SideEffect` enum — 5 variants covering Navigate, Login, Logout, SwitchView, SwitchViewAndNavigate.
- `terminal.rs::dispatch_side_effect` — single funnel for all UI side effects.
- POSIX exit codes: 0 / 1 / 2 / 127.

**Detailed plan:** [`docs/superpowers/plans/2026-04-20-phase1-mount-and-command-contracts.md`](plans/2026-04-20-phase1-mount-and-command-contracts.md)

### ✅ Phase 2 — POSIX Polish, a11y, Perf

Ten tracks addressing 26 issues from the initial review.

**Scope (one-line per track):**
- **D** — `AppRoute::resolve(fs)` corrects file/dir classification; `cd ""` errors (POSIX).
- **B** — `grep` regex + `-i`/`-v`/`-E`/`-F` flags; `head`/`tail` strict POSIX parsing.
- **A** — Lexer word coalescing, unclosed-quote errors, POSIX `$UNDEF` drop, multi-var export.
- **C** — Autocomplete UTF-8 safety; `less`/`more` removed from FILE_COMMANDS.
- **P** — Cherry-pick from WIP: CSP `api.ensideas.com`, breadcrumb absolute-path fix.
- **F** — Terminal render perf: `ctx.fs.with()`, `OutputLineId` newtype, no RingBuffer double-clone.
- **E** — Reader race fix: `Effect + spawn_local` → `LocalResource`.
- **G** — Explorer UI: keyboard nav on FileListItem, dropdown helper, debug-log cleanup.
- **H** — Navigation: delete in-app `forward_stack`, delegate to browser history.
- **I** — Cleanup: ErrorBoundary CSS extraction, `wallet::disconnect(ctx)` helper, lock icon SVG, BottomSheet a11y.

**Follow-ups** (after multi-perspective review):
- `DisplayPermissions` format fix (cleared 4 pre-existing test failures).
- `help.txt` + `CLAUDE.md` documentation refresh.
- Breadcrumb path unit tests (extracted `build_segment_path`).
- `AppError` unified enum + `From` impls for `WalletError` / `FetchError` / `EnvironmentError`.
- `grep -F` POSIX fixed-strings flag + clearer extra-positional error.
- Router `Memo` Root fast-path; `navigate_history` uses `.with()`.

**Detailed plans:**
- Master: [`docs/superpowers/plans/2026-04-20-phase2-master.md`](plans/2026-04-20-phase2-master.md) — contains the full **Decision Log** for every non-obvious choice in Phase 2.
- Per-track plans at `docs/superpowers/plans/2026-04-20-phase2-*.md`.
- Follow-ups: [`docs/superpowers/plans/2026-04-20-phase2-followups.md`](plans/2026-04-20-phase2-followups.md).

---

### ✅ Phase 3a — Write Capability (Direct Commit)

End-to-end "edit markdown in browser → atomic commit to GitHub" for an authenticated admin, with IndexedDB-persisted drafts and compare-and-swap conflict detection. Phase 3 was split: 3a ships the direct-commit path now; 3b (staging UI, binary uploads, sync panel, 3-mode HackMD editor) is deferred.

**Scope that shipped:**
- `core::changes::ChangeSet` — unified `BTreeMap<VirtualPath, Entry>` draft tracker (every 3a entry staged-by-default; stage/unstage wired but UI lands in 3b).
- `core::merge::merge_view(base, changes)` — pure overlay feeding `ctx.view_fs` (a `Signal<Rc<VirtualFs>>` derived from base + changes).
- `core::admin::{admin_status, can_write_to}` — single-admin allowlist (`ADMIN_ADDRESSES`).
- `core::storage::{StorageBackend, StorageError, CommitOutcome, GitHubBackend}` — narrow trait; GitHub impl via GraphQL `createCommitOnBranch` with `expectedHeadOid` CAS. `MockBackend` behind a `mock` cargo feature for integration tests.
- `VirtualFs::serialize_manifest()` — byte-stable re-emission; commits always include a regenerated `manifest.json`.
- IndexedDB (`idb` 0.6) `drafts` + `metadata` stores; 300 ms debounced persist effect; boot-time hydration.
- New `Command` variants: `Touch`, `Mkdir`, `Rm [-r]`, `Rmdir`, `Edit`, `EchoRedirect` (`echo "body" > path`), `Sync(SyncSubcommand)` with subcommands `status` / `commit -m <msg>` / `refresh` / `auth <pat>` / `auth clear`.
- New `SideEffect` variants (Phase 1 contract preserved): `ApplyChange` / `Stage|Unstage|Discard Change` / `StageAll` / `UnstageAll` / `Commit { message, expected_head }` / `RefreshManifest` / `SetAuthToken` / `ClearAuthToken` / `OpenEditor`. Async handlers inside `dispatch_side_effect` via `spawn_local`.
- `AppContext` gains `changes`, `view_fs`, `backend`, `remote_head`, `editor_open`.
- `AppError::Storage(StorageError)` on the existing error funnel.
- Autocomplete covers write + sync commands (two-level `sync` completion).
- Minimal `EditModal` (textarea + Save/Cancel), triggered by `SideEffect::OpenEditor` and mounted inside the root `ErrorBoundary`. Save routes through `dispatch_side_effect(ApplyChange)` — never mutates `ctx.changes` directly.
- Session-scoped GitHub PAT in sessionStorage (documented security caveat in README).

**Detailed plan:** [`docs/superpowers/plans/2026-04-20-phase3a-write-direct-commit.md`](plans/2026-04-20-phase3a-write-direct-commit.md).
**Design doc:** [`docs/superpowers/specs/2026-04-20-phase3-write-design.md`](specs/2026-04-20-phase3-write-design.md).
**Manual QA checklist:** [`docs/superpowers/checklists/2026-04-20-phase3a-manual-qa.md`](checklists/2026-04-20-phase3a-manual-qa.md).

---

## Upcoming Phases

### 📋 Phase 3b — Write Capability (Staging + Binary + Editor UI)

Built on top of 3a's `ChangeSet` / `StorageBackend` / `AppContext` foundation.

**Scope:**
- **Staging UI** — `components/status/sync_panel.rs` surfacing `iter_staged` / `iter_unstaged` with per-entry stage/unstage/discard. 3a already emits the side effects; 3b wires the UI.
- **Default-unstaged** — flip `ChangeSet::upsert` to `staged = false` and require explicit `sync add` (spec §12.2/§12.3).
- **Binary uploads** — `CreateBinary { blob_id, mime, meta }` path end-to-end: data-URL intake, IDB blob store, GraphQL `additions` encoded as base64.
- **3-mode editor** — `components/reader/editor.rs` + `preview.rs` (HackMD-style edit / split / preview); replaces 3a's minimal textarea modal for markdown files.
- **Markdown image rewriting** — `markdown_to_html_with_images` substituting data-URL references for pending uploads before the render pass.
- **New commands** — `sync add <path>`, `sync reset <path>`, `sync discard <path>`.
- **Dynamic admin list** (optional) — move `ADMIN_ADDRESSES` off the hard-coded constant if a concrete multi-admin need appears.

**Estimated size:** comparable to 3a.

### 📋 Phase 4 — Cryptography Decision

**Goal:** Close the gap between what the code claims (`EncryptionInfo.algorithm: "AES-256-GCM"`, `WrappedKey[]`) and what's actually implemented (UI-only, no crypto).

**Scope — pick ONE direction:**

**Option A — Real implementation**
- Add `ecies` / `aes-gcm` / `k256` crates.
- Use `eth_getEncryptionPublicKey` to wrap keys per recipient.
- Content is fetched as ciphertext, decrypted client-side.
- Validate EIP-55 checksum on recipient addresses.
- Nonce per file (12-byte random), authenticated AEAD.
- ENS-style key rotation path.

**Option B — Honest rebrand**
- Rename `EncryptionInfo` → `AccessFilter` (or similar).
- Remove `"AES-256-GCM"` string fields.
- Update README/metadata — no longer advertise "cryptographic private archive".
- Keep UI lock icon as "listed-recipients-only" hint, not cryptographic boundary.
- Decision: 1–2 days of doc + rename work.

**Recommendation:** **Option B first** (fast, truthful), **Option A later** if there's a concrete user need. Today's code advertises crypto guarantees it doesn't keep — that's the most critical integrity issue in the project.

**Related:** if Phase 3 ships write capability, encrypted-write complexity balloons (need wrap keys at commit time). So **Phase 4 decision should precede Phase 3 completion**, even if only in the "rebrand" form.

### 📋 Phase 5 — Production Hardening

**Goal:** Make the build genuinely deployable without security/reliability caveats.

**Scope:**
- **CSP tightening** — strip `unsafe-inline` / `unsafe-eval` / `ws://localhost:*` from production build; use hash-based CSP for required inline SVGs.
- **PDF.js pinning** — bundle Mozilla's viewer with SRI, or replace with a Rust PDF renderer.
- **Manifest integrity** — sign `manifest.json` (secp256k1 or Ed25519) and verify in WASM; defends against GitHub account takeover.
- **ENS resolution on-chain** — remove `api.ensideas.com` dependency; use `eth_call` through the connected wallet.
- **Wallet session hygiene** — on `accountsChanged`, purge sessionStorage cache and force-navigate away from routes tied to the old account.
- **Toolchain pinning** — `rust-toolchain.toml` → specific version (`1.87.0` etc.), not `stable`.
- **PWA manifest** — `start_url: "/"` → `"./"` (IPFS-gateway-safe).
- **CI** — GitHub Actions for `cargo check`, `cargo test --bin websh`, `cargo build --release --target wasm32-unknown-unknown`, Cargo.lock up-to-date check.

### 📋 Phase 6+ — Automation & Polish

- **Release pipeline** — `just pin` → CID → automated ENS contenthash update via `cast`.
- **Per-command `--help`** — builtin help text per command, not just a flat `help`.
- **Remaining Phase 2 gaps**:
  - `!`-coalescing in lexer (`echo foo!bar` — currently splits).
  - `export FOO='"quoted"'` double-trim (`execute_export` strips quotes after lexer already did).
- **Docs site** (optional) — public-facing project site separate from in-repo `docs/`.

---

## How to Resume Work

On a fresh clone (e.g., on the server):

```bash
git clone <repo-url> websh
cd websh

# See what's planned
cat docs/superpowers/ROADMAP.md
cat docs/superpowers/plans/2026-04-20-phase2-master.md   # Phase 2 decision log

# Navigate phase history
git log --oneline --graph --first-parent main | head -30
```

To explore the WIP Jan 2026 write-mode prototype before Phase 3:

```bash
git checkout wip/january-2026-restructure
# ... inspect
git checkout main
```

Local dev loop (unchanged):

```bash
# First time only:
rustup target add wasm32-unknown-unknown
cargo install trunk stylance-cli

# Build + serve with hot reload:
trunk serve
# → http://127.0.0.1:8080
```

---

## Philosophy Notes

- **Decision logs are canonical.** Every non-obvious choice in Phase 1/2 is recorded in the master plan docs with rationale. When Phase 3 starts, read those first.
- **`wip/january-2026-restructure` is reference, not a merge target.** Its `CommandResult` shape is incompatible with Phase 1+2's — integration requires rewrite, not cherry-pick (except for a few small items already taken in Track P).
- **Speculative infrastructure is avoided.** `AppError` landed in Phase 2 follow-ups because it was small and preempted known churn. Async `SideEffect` / `FsState` overlay were NOT added speculatively — they need a concrete write-command spec to shape correctly.
- **YAGNI for ergonomic sugar, POSIX-strict for user-facing.** The parser rejects `head 5` (bare positional) because POSIX bash does; UX-only helpers aren't added without a concrete user need.
