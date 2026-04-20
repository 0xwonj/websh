# WebSH Development Roadmap

Last updated: 2026-04-20

This document is the single entry point for someone picking up WebSH development on a new machine (server, new clone). It summarizes what has shipped, what's queued, and where to look for detail.

---

## Snapshot

**Current main**: Phase 1 + Phase 2 + follow-ups merged.
**Build**: `cargo build --release --target wasm32-unknown-unknown` — clean.
**Tests**: `cargo test --bin websh` — **205 pass / 0 fail**.

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

## Upcoming Phases

### 📋 Phase 3 — Write Capability

**Goal:** Users can edit files from the browser and commit changes back to GitHub.

**Source material:** `wip/january-2026-restructure` branch (abandoned Jan 2026 attempt).

**Scope:**
- **Filesystem layering** — `FsState` combining `VirtualFs` (base) + `PendingChanges` overlay + `StagedChanges`; `MergedFs` read-only view that feeds the current `&VirtualFs` call sites.
- **Storage backend** — `StorageBackend` trait; `GitHubBackend` via Contents API (base64 put, manifest re-serialize, commit).
- **Admin auth** — `ADMIN_ADDRESSES` constant, `is_admin(wallet)`.
- **Async side effects** — extend `SideEffect` with `CommitAsync` / `UpdatePending` / etc., OR make `dispatch_side_effect` uniformly async.
- **New commands** — `touch`, `mkdir`, `rm`, `rmdir`, `sync {status,add,reset,commit,discard,auth}`.
- **Editor UI** — `components/reader/editor.rs` + `preview.rs` (HackMD-style 3-mode reader).
- **Sync UI** — `components/status/sync_panel.rs` (staged/unstaged view, commit button).
- **Markdown image rewriting** — `markdown_to_html_with_images` for data-URL pending uploads.
- **New deps** — `base64`, `urlencoding`, additional `web-sys` features (`File`, `FileReader`).

**Already prepared in Phase 2 (do NOT redo):**
- `AppError` enum for cross-domain `?` propagation.
- `MountRegistry` singleton (ready for `is_writable()` extension).
- `CommandResult` foundation (ready for async variant).

**Key design decisions needed before coding:**
1. Async `SideEffect` shape — concrete write command spec drives this.
2. `ctx.fs` signal semantics when FsState is mutable — batching write operations to avoid Memo storms.
3. localStorage quota strategy (pending binary uploads base64-encoded hit ~5-10MB limit fast).
4. `ADMIN_ADDRESSES` in source vs. dynamic auth list.
5. Debug `console.log`s to be removed before merge (WIP branch has 8+ in save_content).

**Estimated size:** largest phase by far — multi-week.

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
