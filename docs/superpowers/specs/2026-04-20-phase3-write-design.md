# Phase 3 — Write Capability: Design Document

Status: **DRAFT** (2026-04-20, in progress)
Type: Design document (end-state description, not execution plan)
Supersedes: `docs/superpowers/plans/2026-04-20-phase3-wip-analysis.md` (research artifact)

---

## 0. Purpose of this document

This document describes **what the websh system looks like when Phase 3 write capability is complete** — its architecture, modules, contracts, and behavior. It is intentionally NOT an execution plan; it answers "what is the ideal shape?" so that a subsequent implementation plan (via `writing-plans` skill) has a fixed target.

Scope boundaries, sequencing into 3a/3b/3c, and non-goals are in §12 and §14 but the main body describes the end state.

Design influences:
- **Sveltia CMS** architecture — IndexedDB drafts, GraphQL `createCommitOnBranch`, `expectedHeadOid` compare-and-swap. The closest fit to websh's "pure browser, no backend" constraint.
- **Git's object model** — working tree, index, HEAD; staged is a subset of working tree, not a disjoint collection.
- **Phase 1/2 contracts** — `CommandResult`, `SideEffect`, `dispatch_side_effect`, `AppError`, `MountRegistry`. Extended, not replaced.
- **`wip/january-2026-restructure`** — reference only; concrete patterns adopted noted inline, anti-patterns rejected noted in §13.

---

## 1. Goals & Non-Goals

### 1.1 Goals

An authenticated admin (wallet-gated) can, from the browser, with no server component:

1. Edit existing markdown/text files in the VFS.
2. Create new files and directories (`touch`, `mkdir`).
3. Remove files and directories (`rm`, `rmdir`).
4. Upload binary assets (images) and reference them from markdown.
5. Accumulate a batch of changes as unstaged drafts, selectively stage a subset, and commit the staged subset atomically to GitHub in a single operation.
6. Recover unsaved drafts on page reload.
7. Receive an honest error when the remote has moved since drafts began (no silent clobber).

### 1.2 Non-Goals (end state — not deferred, simply out of scope)

- **Multi-user concurrent editing.** Single-admin mental model. CAS detects conflicts; resolution is "reload and redo."
- **Rich-text WYSIWYG editor.** Plain textarea + live preview. CodeMirror/Monaco deferred indefinitely.
- **Three-way merge / CRDT.** If remote changed during edit, the admin re-edits.
- **Server-proxied OAuth.** Browser-only auth flow (session-token model, see §8).
- **Real content encryption.** Phase 4 Option B (honest rebrand) is a prerequisite; Phase 4 Option A (real ECIES) is out.
- **Real-time collaborative presence.** Not applicable to single-admin model.
- **Offline write queue with automatic sync-on-reconnect.** Drafts persist offline; commits require online explicit trigger.

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  UI Layer                                                        │
│  - Terminal (shell commands: sync / touch / rm / edit / mkdir)  │
│  - Reader (3-mode: read / preview / edit — 3c adds edit mode)   │
│  - SyncPanel (staged/unstaged view, commit button — 3b)         │
└────────────┬────────────────────────────────────────────────────┘
             │ SideEffect enum (extended with async variants)
             │ dispatch_side_effect → spawn_local for async paths
┌────────────▼────────────────────────────────────────────────────┐
│  Command Layer (Phase 1 shape preserved)                         │
│  - Command enum (+ Sync, Touch, Rm, Rmdir, Mkdir, Edit)          │
│  - execute_pipeline → CommandResult (sync return — unchanged)    │
│  - Async write ops: returned as SideEffect variants, dispatched  │
│    via spawn_local by terminal.rs dispatcher                     │
└────────────┬────────────────────────────────────────────────────┘
             │ reads ctx.view_fs (merged view)
             │ mutates via ctx.changes.update(...) in side-effect handlers
┌────────────▼────────────────────────────────────────────────────┐
│  Reactive State                                                  │
│  - ctx.fs:       RwSignal<VirtualFs>      (remote base)          │
│  - ctx.changes:  RwSignal<ChangeSet>      (edits + staged flag)  │
│  - ctx.view_fs:  Memo<Rc<VirtualFs>>      (cached merged view)   │
│  - ctx.wallet:   RwSignal<WalletState>    (existing)             │
│  - ctx.backend:  StoredValue<Arc<dyn StorageBackend>> (ref)      │
└────────────┬────────────────────────────────────────────────────┘
             │ hydrate on boot / persist on mutate
┌────────────▼────────────────────────────────────────────────────┐
│  Persistence — IndexedDB (idb crate)                             │
│  - draft_store:    mount_id → ChangeSet (serialized)             │
│  - blob_store:     content_hash → Blob   (binary assets — 3c)    │
└────────────┬────────────────────────────────────────────────────┘
             │ commit batch (atomic)
┌────────────▼────────────────────────────────────────────────────┐
│  Storage Abstraction                                             │
│  - trait StorageBackend { commit, fetch_manifest }               │
│  - GitHubBackend: GraphQL createCommitOnBranch + expectedHeadOid │
└─────────────────────────────────────────────────────────────────┘
```

**Core design principles**:

1. **Phase 1/2 contracts preserved.** `CommandResult { output, exit_code, side_effect }` shape unchanged. No `pending` or `staged` fields added to `CommandResult`. Write mutations flow through `SideEffect` dispatch, not through result fields.
2. **Atomic commits are the only commit model.** GraphQL `createCommitOnBranch` with `expectedHeadOid`. Per-file REST PUT (Contents API) is not supported. Multi-file commits are all-or-nothing.
3. **IndexedDB is the canonical draft store.** `localStorage` remains only for env vars and wallet session (pre-existing).
4. **Phase 4 Option B is a prerequisite.** `EncryptionInfo → AccessFilter` rename lands before Phase 3 ships. The codebase must not claim cryptographic guarantees it doesn't deliver while also letting admins edit "encrypted" files.
5. **Backend trait even for one implementation.** Trait abstracts the atomic-batch-commit boundary. Justified by: (a) mock backend enables pure-logic integration tests for commit flows, (b) IPFS/local/custom backends are plausible future extensions, (c) the trait documents the contract explicitly.
6. **Async stays inside side-effect handlers.** `dispatch_side_effect` remains sync at the call-site; specific variants that need async work spawn_local internally. This keeps `execute_pipeline` sync and testable without a reactive runtime.

---

## 3. State Model

Three reactive pieces, one stored ref. Detailed below.

### 3.1 `ctx.fs: RwSignal<VirtualFs>` — remote base

Existing signal. Continues to hold the `VirtualFs` constructed from the currently-loaded manifest. Mutated only by:
- Initial manifest load on boot.
- Post-commit manifest refresh (after `StorageBackend::commit` succeeds, we re-fetch and `set()`).

No per-edit mutation. The base is always "last known remote state."

### 3.2 `ctx.changes: RwSignal<ChangeSet>` — unified edit tracker

**Single source of truth** for all in-progress work. Unifies what WIP split into `PendingChanges` + `StagedChanges` (and eliminates the sync-drift bug that split created).

```rust
pub struct ChangeSet {
    entries: BTreeMap<VirtualPath, Entry>,  // path-sorted, deterministic iteration
}

pub struct Entry {
    pub change: ChangeType,
    pub staged: bool,           // included in next commit?
    pub timestamp: u64,         // created/last-modified (for display only)
}

pub enum ChangeType {
    CreateFile      { content: String, meta: FileMetadata },
    CreateBinary    { blob_id: BlobId, mime: String, meta: FileMetadata },  // 3c
    UpdateFile      { content: String, description: Option<String> },
    DeleteFile,
    CreateDirectory { meta: DirectoryMetadata },
    DeleteDirectory,
}
```

**Why `BTreeMap`, not `HashMap` + insertion-order `Vec`** (WIP did this): path-sorted iteration is deterministic across sessions and test runs, and `sync status` displays alphabetical naturally. Insertion order adds no user value.

**Why `staged: bool` inlined, not a separate set**: the split in WIP allowed staged to reference non-existent pending paths (sync bug). Inlined flag makes the invariant enforce-at-type-system.

**Operations** (method surface on `ChangeSet`):

```rust
impl ChangeSet {
    pub fn upsert(&mut self, path, change);          // stages new entries by default in 3a
    pub fn stage(&mut self, path);
    pub fn unstage(&mut self, path);
    pub fn discard(&mut self, path);                 // remove from set entirely
    pub fn stage_all(&mut self);
    pub fn unstage_all(&mut self);
    pub fn clear(&mut self);                         // post-commit clean

    pub fn get(&self, path) -> Option<&Entry>;
    pub fn is_staged(&self, path) -> bool;
    pub fn is_deleted(&self, path) -> bool;          // matches any Delete* variant

    pub fn iter_all(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)>;
    pub fn iter_staged(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)>;
    pub fn iter_unstaged(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)>;

    pub fn summary(&self) -> Summary;                // counts: creates/updates/deletes × staged/unstaged
    pub fn is_empty(&self) -> bool;
}
```

**Phase 3a behavior**: `upsert` creates entries with `staged = true`. `sync commit -m` commits everything. `sync add`/`sync reset` commands don't exist yet.

**Phase 3b behavior**: `upsert` creates entries with `staged = false`. `sync add <path>` / `sync reset <path>` toggle the flag. `sync commit -m` commits only `iter_staged()`.

### 3.3 `ctx.view_fs: Memo<Rc<VirtualFs>>` — merged read view

Derived value, not state. Consumers that read "the filesystem as currently visible" (ls, explorer, reader-in-read-mode) read `ctx.view_fs.with(|fs| ...)` or `ctx.view_fs.get()`.

```rust
// Pure function, no signals
pub fn merge_view(base: &VirtualFs, changes: &ChangeSet) -> VirtualFs {
    let mut merged = base.clone();
    for (path, entry) in changes.iter_all() {
        apply_change(&mut merged, path, &entry.change);
    }
    merged
}

// Memo constructed at AppContext::new
let view_fs = Memo::new(move |_| {
    Rc::new(ctx.fs.with(|base| {
        ctx.changes.with(|changes| merge_view(base, changes))
    }))
});
```

**Why `Memo<Rc<VirtualFs>>`, not a function at each call site**: VirtualFs is large; cloning per-read is the exact bug WIP's `FsState::get()` had. Memo caches until `fs` or `changes` actually changes. `Rc` wrap makes consumer `.get()` O(1).

**Why not mutate `ctx.fs` in place for edits (simpler)?**:
- Loses the distinction between "remote committed" and "my uncommitted edit" — needed for `sync status`, conflict detection, discard.
- Post-commit refresh would have to selectively preserve non-committed changes; complicated.
- Edit preview needs base to compare; overwriting loses that.

### 3.4 `ctx.backend: StoredValue<Arc<dyn StorageBackend>>` — backend ref

Not a signal (backend doesn't change reactively during session). `StoredValue` gives Leptos-compatible sharing without Memo overhead. Initialized at mount resolution: each writable `Mount` carries a backend reference.

### 3.5 What `AppContext` looks like end-of-Phase-3

```rust
pub struct AppContext {
    // Existing
    pub fs: RwSignal<VirtualFs>,
    pub wallet: RwSignal<WalletState>,
    pub view_mode: RwSignal<ViewMode>,
    pub terminal: TerminalState,
    pub explorer: ExplorerState,

    // New in Phase 3
    pub changes: RwSignal<ChangeSet>,
    pub view_fs: Memo<Rc<VirtualFs>>,
    pub backend: StoredValue<Option<Arc<dyn StorageBackend>>>,  // None if current mount not writable
    pub remote_head: StoredValue<Option<String>>,               // last-known HEAD SHA for CAS
    pub sync: SyncUiState,                                      // 3b: UI-only state (panel open, etc.)
}
```

`admin_status()` is a free function (§8), not a field.

`remote_head` is the SHA of the commit that `ctx.fs` was built from. Populated on boot (manifest fetch can expose the commit SHA via a sidecar request or a SHA field in the manifest itself), updated after every successful `CommitOutcome`, persisted to IDB metadata store. Read at command-dispatch time for `expected_head` (see §5.2).

---

## 4. Storage Abstraction

### 4.1 The trait

```rust
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub trait StorageBackend {
    fn backend_type(&self) -> &'static str;

    /// Commit the staged subset of a ChangeSet as an atomic batch.
    ///
    /// `expected_head` is the SHA the caller believed was current when drafting began;
    /// mismatch = Conflict error.
    fn commit(
        &self,
        changes: &ChangeSet,
        message: &str,
        expected_head: Option<&str>,
    ) -> BoxFuture<'_, StorageResult<CommitOutcome>>;

    /// Fetch the latest manifest — used post-commit, and for conflict-recovery refresh.
    fn fetch_manifest(&self) -> BoxFuture<'_, StorageResult<Manifest>>;
}

pub struct CommitOutcome {
    pub new_head: String,        // new HEAD SHA
    pub manifest: Manifest,      // updated manifest (caller sets ctx.fs)
    pub committed_paths: Vec<VirtualPath>,
}
```

**Only two methods**. No per-file CRUD. Why:
- Atomicity is a backend concern; each backend exposes it via whatever primitive it has.
- `ChangeSet` already represents the full intent; the backend translates it to its native format.
- Testing: a mock that returns fixed `CommitOutcome` is trivial.

### 4.2 GitHubBackend

Primary (and, initially, only) implementation.

**Commit path — GraphQL `createCommitOnBranch`**:

```graphql
mutation ($input: CreateCommitOnBranchInput!) {
  createCommitOnBranch(input: $input) {
    commit { oid }
  }
}
```

With input:
```json
{
  "branch": { "repositoryNameWithOwner": "0xwonj/db", "branchName": "main" },
  "message": { "headline": "..." },
  "expectedHeadOid": "<sha>",
  "fileChanges": {
    "additions": [{ "path": "...", "contents": "<base64>" }],
    "deletions": [{ "path": "..." }]
  }
}
```

**Properties**:
- One HTTP request per commit — atomic by GitHub's contract.
- `expectedHeadOid` = compare-and-swap. If HEAD moved, GitHub returns a specific error; we surface "remote changed, please reload."
- All `additions` are base64-encoded content; `deletions` are paths only.
- Updates (`UpdateFile`) become `additions` (content overwrite).
- `CreateDirectory` / `DeleteDirectory` are implicit on GitHub (directories exist iff they contain files). Our backend translates:
  - `CreateDirectory` with no files in it → error or drop (directories must contain files; if user typed `mkdir foo` with no subsequent edits, `sync commit` should reject or warn).
  - `DeleteDirectory` → emit `deletions` for every file under that path; fetch tree if needed to enumerate.

**Fallback path — REST Git Data API**:
- GraphQL `createCommitOnBranch` caps per-file size at ~10 MB (base64-in-query limit).
- If any `CreateBinary` entry exceeds the threshold, GitHubBackend transparently switches to the Git Data API path: blob POST × N → tree POST → commit POST → ref PATCH.
- Same atomic guarantee (ref is only advanced at final step); same `expectedHeadOid` semantic (check parent SHA).

**Manifest update in the same commit**:
- The commit includes `manifest.json` as an addition, re-serialized from the post-apply state.
- This means the client computes the new manifest locally, appends it to `fileChanges.additions`, and commits everything atomically. No separate "update manifest" step; no window where manifest and content disagree.
- **New infrastructure required — `VirtualFs::serialize_manifest() -> Manifest`** (or `-> String`). Today the direction is one-way: `Manifest → VirtualFs` via `VirtualFs::from_manifest`. Phase 3 adds the reverse. Two constraints:
  1. **Byte-stable output.** Same logical state must serialize to identical bytes across sessions/machines/Rust versions — otherwise every commit produces a spurious `manifest.json` diff even when no fields changed. Concretely: stable key order (either field order for structs or sorted keys for maps), fixed numeric formatting, explicit `serde` attribute choices documented at the type. Covered by a round-trip test (`manifest → VirtualFs → serialize_manifest → bytes` equals original bytes) plus a golden fixture.
  2. **Schema compatibility.** The serialized form must match whatever the read path expects — any field the reader requires, the writer must emit. This is enforceable via a single shared `Manifest` struct with `Serialize + Deserialize`; no parallel write-only types.

**Authentication**:
- PAT held in session memory (see §8). GitHubBackend reads it via a constructor arg (`GitHubBackend::new(mount, token)`).
- Token is never logged. Errors from GitHub are mapped to `StorageError::AuthFailed` for 401/403 so we don't echo tokens back in debug.

### 4.3 Error model — `StorageError` extends `AppError`

```rust
pub enum StorageError {
    AuthFailed,                               // 401/403
    Conflict { remote_head: String },         // expectedHeadOid mismatch / 409
    NotFound(String),                         // 404
    ValidationFailed(String),                 // 422 (e.g., path invalid, commit rejected)
    RateLimited { retry_after: Option<u64> }, // 429
    ServerError(u16),                         // 5xx
    NetworkError(String),                     // transport failures
    NoToken,                                  // admin not authenticated for this mount
    BadRequest(String),                       // client-side validation failed before sending
}

// AppError gains a variant + From impl
pub enum AppError {
    Wallet(WalletError),
    Fetch(FetchError),
    Environment(EnvironmentError),
    Storage(StorageError),                    // NEW
}

impl From<StorageError> for AppError { ... }
```

Pattern matches Phase 2's `AppError` extension pattern — symmetric, predictable.

### 4.4 Rate-limit handling

- On `RateLimited { retry_after }`, the commit side-effect handler surfaces an error line with "try again in N seconds." No automatic retry. Why: retry loops in the browser with user-initiated actions are bad UX; the user should know their action didn't land.
- Exception: transient `NetworkError` during a single commit may retry once after ~500ms jittered, then fail. Documented as "one auto-retry on transient network failure."

---

## 5. Reactive Flow for a Write Operation

Concrete end-state example: user types `echo 'new' > notes.md` in the terminal, then `sync commit -m "add notes"`.

(Note: `echo > file` redirection is a Phase 3 addition to the parser; see §6.)

### 5.1 The first edit

1. `execute_pipeline` parses the command, dispatches to the `echo > path` handler.
2. Handler returns `CommandResult { output: [], exit_code: 0, side_effect: Some(SideEffect::ApplyChange { path, change: ChangeType::CreateFile {...} }) }`.
3. Terminal's `dispatch_side_effect` matches `ApplyChange` → calls `ctx.changes.update(|cs| cs.upsert(path, change))`.
4. `ctx.view_fs` Memo invalidates (its dependency `changes` fired).
5. Any component reading `ctx.view_fs` (e.g., the explorer tree if open) re-renders to include `notes.md`.
6. Separately: an `Effect` watching `ctx.changes` serializes the new state to IndexedDB's `draft_store`. Debounced (~300ms) to avoid write amplification on rapid typing.

### 5.2 The commit

1. `execute_pipeline` parses `sync commit -m "add notes"`.
2. Handler checks `admin_status(...)` and current mount writability. If either fails: `CommandResult { output: [AccessDenied line], exit_code: 1, side_effect: None }`.
3. Otherwise: handler reads `ctx.remote_head` (snapshot at this moment) and builds `CommandResult { output: ["Committing..."], exit_code: 0, side_effect: Some(SideEffect::Commit { message: "add notes", expected_head: current_head }) }`. This captures the expected HEAD at command-time, not dispatch-time, so rapid double-submits don't race against each other.
4. Terminal's `dispatch_side_effect` matches `Commit` → `spawn_local(async move { ... })`.
5. Inside spawn_local:
   - Read `ctx.changes` — if empty or nothing staged, append error line via `ctx.terminal.output.update(...)` and return.
   - Read `ctx.backend` — if None, same.
   - Call `backend.commit(&changes, &msg, expected_head).await`.
   - On `Ok(CommitOutcome { new_head, manifest, committed_paths })`:
     - `ctx.fs.set(VirtualFs::from_manifest(manifest))` — replaces base.
     - `ctx.changes.update(|cs| committed_paths.iter().for_each(|p| cs.discard(p)))` — clears committed entries, preserves any unstaged drafts.
     - Append success line to `ctx.terminal.output`.
     - IndexedDB draft_store re-serialized by the existing Effect.
   - On `Err(StorageError::Conflict { remote_head })`:
     - Append "remote changed since drafting began (now: \<sha\>). Run `sync refresh` to reload base and re-stage." line.
     - **Do not** auto-refresh — admin decides.
   - On other errors: map to AppError, display.

### 5.3 Reactive contract

The key invariant enforced by this flow:
- **`ctx.fs` reflects remote reality.** Never holds uncommitted edits. Post-commit, it refreshes. This means on page reload, `ctx.fs` is a clean base — drafts are layered back via `ctx.changes` restoration from IndexedDB.
- **`ctx.changes` is the only write-mutated signal.** All command-layer mutations go through `ApplyChange` / `StageChange` / `UnstageChange` / `DiscardChange` side-effects.
- **`ctx.view_fs` is always self-consistent** because it's derived.

---

## 6. Commands & Async Dispatch

### 6.1 New `Command` variants

```rust
pub enum Command {
    // ...existing 14 variants...

    // Phase 3a
    Touch { path: PathArg },
    Mkdir { path: PathArg },
    Rm { path: PathArg, recursive: bool },
    Rmdir { path: PathArg },
    Edit { path: PathArg },             // transitions Reader to edit mode
    Sync(SyncSubcommand),
}

pub enum SyncSubcommand {
    Status,                             // print summary of ChangeSet
    Add { path: Option<PathArg> },      // stage all if path is None (3b)
    Reset { path: Option<PathArg> },    // unstage (3b)
    Commit { message: String },
    Discard { path: Option<PathArg> },  // remove entry entirely
    Refresh,                            // re-fetch base manifest, keep drafts
    Auth(AuthAction),                   // see §8
}
```

**Redirection — `echo 'x' > path`** is added to the parser (Phase 3a). This is the minimum useful write command without a real editor; becomes nice-to-have when `edit` lands.

### 6.2 Extended `SideEffect` enum

```rust
pub enum SideEffect {
    // Existing (Phase 1)
    Navigate(AppRoute),
    Login,
    Logout,
    SwitchView(ViewMode),
    SwitchViewAndNavigate(ViewMode, AppRoute),

    // Phase 3
    ApplyChange    { path: VirtualPath, change: ChangeType },
    StageChange    { path: VirtualPath },
    UnstageChange  { path: VirtualPath },
    DiscardChange  { path: VirtualPath },
    StageAll,
    UnstageAll,
    Commit         { message: String, expected_head: Option<String> },
    RefreshManifest,
    SetAuthToken   { token: String },       // session-stored, see §8
    ClearAuthToken,
    OpenEditor     { path: VirtualPath },   // triggers Reader edit mode (3c)
}
```

`dispatch_side_effect` stays sync. Handlers for sync variants mutate signals directly; handlers for `Commit` and `RefreshManifest` spawn_local internally.

### 6.3 Why this shape (and not alternatives)

**Alternative A — make `execute_pipeline` return async?** Rejected. Would force all tests to run under a reactive runtime, and the sync contract in Phase 1 is load-bearing for autocomplete and filter-pipeline composition.

**Alternative B — put `changes` updates in `CommandResult`?** (What WIP did.) Rejected. `CommandResult` is a pure value; threading signal mutations through it fights the existing architecture. Two mutation paths (signals + results) is worse than one.

**Alternative C — direct signal mutation inside `execute_command`?** Rejected. `execute_command` takes `&` references to state for testability (see §9). If it mutates signals, it can't be called in non-wasm unit tests without a reactive runtime.

The chosen shape: `execute_command` stays pure (inputs-to-`CommandResult`), the `SideEffect` enum widens, dispatch handles the reactive/async plumbing. Matches Phase 1's separation of "what the command does" vs. "how the side effect lands."

### 6.4 Autocomplete registration

`Command::names()` gains: `touch`, `mkdir`, `rm`, `rmdir`, `edit`, `sync`.

`sync` has subcommands; autocomplete for `sync <tab>` lists `status add reset commit discard refresh auth`. Added to the autocomplete layer in `core/autocomplete.rs`.

---

## 7. Persistence — IndexedDB

### 7.1 Rationale (Sveltia pattern)

localStorage is unsuitable for Phase 3c: binary assets (images) base64-encoded hit ~5–10 MB quota fast. Synchronous API stalls the main thread on large writes. IndexedDB is async, blob-native, effective quota is ~50% of free disk.

For Phase 3a/b (text only), localStorage would suffice, but migrating later is churn. Starting with IndexedDB is the right foundation.

Library: **`idb` crate** (thin wrapper on the raw API, async, WASM-compatible, small). Alternative `gloo-storage`'s IDB helpers were considered but are less flexible for schema versioning.

### 7.2 Schema

Database: `websh-state`, version 1.

Object stores:
- **`drafts`** — keyPath: `mount_id` (string). Value: serialized `ChangeSet` (JSON via serde). One entry per writable mount.
- **`blobs`** — keyPath: `blob_id` (string, content hash). Value: `Blob` (native IDB support). Binary assets referenced by `ChangeType::CreateBinary.blob_id`.
- **`metadata`** — keyPath: `key`. Misc: `remote_head.<mount_id>` (last-known HEAD SHA, for `expected_head` at commit), `schema_version`.

Schema migrations in future: version bump + `onupgradeneeded` handler. For 3a initial ship: no migrations needed; starts at v1.

### 7.3 Hydration flow

On app boot, after `ctx.fs` loads from manifest:
1. Open `websh-state` DB (createStores if first run).
2. For each writable mount, read `drafts/<mount_id>` → deserialize to `ChangeSet`. Then `ctx.changes.set(loaded_changes)`.
3. Read `metadata.remote_head.<mount_id>` → `ctx.remote_head.set_value(...)`. Used for `expected_head` at next commit.
4. Optional freshness check: if manifest-fetch response surfaces the current branch HEAD, compare to stored `remote_head`. If they differ (someone else committed since last session), surface a one-time info message: "Remote has moved since your last session. Your drafts are preserved; `sync refresh` to re-base." The admin then decides when to accept the new base.

### 7.4 Persistence trigger

A Leptos `Effect` watches `ctx.changes`, writes to IDB on change. Debounced ~300ms (via `gloo-timers::future::sleep` inside a `set_timeout`-like pattern). Avoids write amplification on rapid typing.

### 7.5 Quota handling

On `QuotaExceededError` from IDB write:
- Surface error line: "Local draft storage full. Discard old drafts or commit to free space."
- Do **not** silently drop data. The admin must take explicit action.
- Phase 3c may add a "largest blobs" command to help identify offenders.

---

## 8. Admin, Auth, Capabilities

### 8.1 Two independent concepts

1. **Admin eligibility** — is this wallet allowed to write? Derived from `WalletState` + `ADMIN_ADDRESSES` constant + `Mount::is_writable()`.
2. **Backend authentication** — does the admin have a valid GitHub token? Separate from (1).

Both must be true to commit. UI gates the editor on (1); commit call fails on (2) with `StorageError::NoToken`.

### 8.2 Admin check

```rust
// core/admin.rs (new module)

pub enum AdminStatus {
    NotConnected,       // wallet disconnected
    Connected { address: String },  // wallet connected but not in allowlist
    Admin { address: String },      // connected + allowlisted
}

pub fn admin_status(wallet: &WalletState) -> AdminStatus { ... }

pub fn can_write_to(wallet: &WalletState, mount: &Mount) -> bool {
    matches!(admin_status(wallet), AdminStatus::Admin { .. }) && mount.is_writable()
}

const ADMIN_ADDRESSES: &[&str] = &[
    // 0xwonj address here
];
```

Mount extension:

```rust
impl Mount {
    pub fn is_writable(&self) -> bool {
        match self {
            Mount::GitHub { writable, .. } => *writable,
            _ => false,
        }
    }
}
```

`writable` added to `Mount::GitHub` (and future backend variants). Configured at `config::mount_list()` — the one existing `~` mount becomes `writable: true`.

**Why constant, not dynamic**: single admin for now. Adding a governance mechanism (multisig, DAO) is out of scope. Hardcoded allowlist is honest — the admin lives in the binary.

### 8.3 Token storage

GitHub PAT needed to call the API. Options considered:

| Option | Persistence | Browser survival | Trust model |
|---|---|---|---|
| **In-memory only** | session | lost on tab close | admin pastes every session |
| **sessionStorage** | session | survives reload within tab | cleared on close; XSS risk |
| **localStorage** | permanent | survives everything | persists across browser restart; higher XSS risk |
| **IDB + ECIES wallet encrypt** | permanent, encrypted | survives everything | needs Phase 4 Option A |

**Chosen: sessionStorage** for Phase 3. Rationale:
- Matches "admin signs in for a session" mental model.
- No Phase 4 Option A dependency.
- **XSS exposure acknowledged, not mitigated.** CSP tightening (strip `unsafe-inline`/`unsafe-eval`) is Phase 5 scope; until then, any injected script can read sessionStorage. The risk is accepted for Phase 3 because: (a) single admin (the project maintainer), (b) no untrusted user content is rendered as HTML without sanitization (existing `ammonia` pass covers markdown), (c) admin usage is intentional, not automated. Moving the admin auth flow to production-grade requires Phase 5's CSP work; this should be a Phase 5-precede-Phase-3-GA gate if wider admin rollout is planned. For the initial phase with a single admin, the exposure is acceptable.

Access via `sync auth <token>` command: validates token format, calls GitHub `/user` to sanity-check, stores in `sessionStorage['websh.gh_token']`. `sync auth clear` removes it.

**Future upgrade path** (post-Phase 4 Option A): wallet-derived ECIES key encrypts the PAT; stored in IDB; decrypted on session start via `personal_sign` prompt. Out of scope for Phase 3.

### 8.4 OAuth device flow — considered, deferred

GitHub's device code flow works from a browser (CORS-enabled). Admin visits `github.com/login/device`, enters a code, gets a token back. Avoids pasting raw PATs.

Deferred because: (a) UX is clunkier than paste-once; (b) requires registering a GitHub OAuth App; (c) token has same storage challenge (same `sessionStorage` choice). The `sync auth` command surface is designed so a device-flow subcommand (`sync auth device`) is additive later.

---

## 9. UI Layer

### 9.1 Reader — 3 modes

End state: Reader component has three modes, controlled by a `mode: RwSignal<ReaderMode>` local to Reader.

```rust
pub enum ReaderMode {
    Read,       // current 3a default — rendered HTML / image / PDF viewer
    Preview,    // 3c: side-by-side source + rendered
    Edit,       // 3c: textarea only (or full-width edit)
}
```

Phase 3a state: only Read exists (unchanged). The `on_edit` placeholder at `reader/mod.rs:172` remains a no-op.

Phase 3c state: Edit transitions the current `sheet-body` to a `<textarea>` bound to a local signal holding current content (initial = last known via `ctx.view_fs`). Save button dispatches `ApplyChange` side-effect. Cancel button discards textarea state, no side-effect.

**Markdown image rewriting** (3c):
- Current `markdown_to_html(...)` receives a second arg: `changes: &ChangeSet`.
- When a `![](image.png)` reference resolves to a `ChangeType::CreateBinary` entry, rewrite the URL to `blob:` URL (created via `URL.createObjectURL` on the Blob from IDB).
- `URL.revokeObjectURL` called when preview unmounts to avoid blob leaks.

### 9.2 SyncPanel (3b)

New component: `components/status/sync_panel.rs`.

Trigger: status bar gets a "⚡ N" indicator (N = staged entries) that's clickable; also keyboard `Ctrl+Shift+S`. Opens a right-side drawer (same pattern as explorer preview sheet).

Contents:
- **Staged** section: list of `iter_staged()` entries with Stage-specific icons (`A` add, `M` modify, `D` delete). Clicking an entry unstages.
- **Unstaged** section: `iter_unstaged()`, clicking stages.
- **Commit message** input + commit button. Submit dispatches `SideEffect::Commit`.
- **Footer**: authentication status ("admin: 0x…", "token: active / missing"). Links to `sync auth` flow.

Pure presentational; all mutations via side-effects.

### 9.3 Terminal — no visual changes

The terminal gains new commands but no layout changes. Command output follows the existing `OutputLine` pattern.

`sync status` output example:
```
Staged:
  A  /home/wonjae/notes.md       (created 2m ago)
  M  /home/wonjae/blog/post.md   (modified 5m ago)

Unstaged:
  M  /home/wonjae/blog/draft.md  (modified 1m ago)

3 changes (2 staged, 1 unstaged). Run `sync commit -m <msg>` to commit staged.
```

Consistent with `git status` but POSIX-formatted (matching existing `ls -l` style).

---

## 10. Error Handling & Observability

### 10.1 Error flow

All write-path errors flow through `AppError::Storage(StorageError)`. The command handler catches the error (inside `spawn_local`), pushes a formatted `OutputLine` to `ctx.terminal.output`.

Mapping rule: users get one line that tells them what to do next:

| Error | Output |
|---|---|
| `NoToken` | `sync: no GitHub token. Run 'sync auth <token>'.` |
| `AuthFailed` | `sync: token invalid or lacks permission.` |
| `Conflict { remote_head }` | `sync: remote changed (now \<sha8\>). Run 'sync refresh'.` |
| `NotFound(path)` | `sync: path not found on remote: \<path\>.` |
| `ValidationFailed(msg)` | `sync: rejected by remote: \<msg\>.` |
| `RateLimited { retry_after: Some(n) }` | `sync: rate limited. Try again in \<n\>s.` |
| `NetworkError(msg)` | `sync: network error. Retry.` |
| `ServerError(status)` | `sync: remote server error (HTTP \<n\>).` |

No stack traces; no verbose diagnostic dumps. Debug info lives in `console.error` (not `console.log` — debug logs are a documented anti-pattern from WIP, see §13).

### 10.2 Observability

- `console.error` for unexpected errors (not `console.log`).
- `console.warn` for recoverable oddities (e.g., empty commit attempted).
- **No `console.log` in hot paths.** Specifically, the save/commit/stage paths must not emit debug logs in production builds. WIP had 14+ `console::log_1` calls in the save hot-path — this design explicitly rejects that.
- `#[cfg(debug_assertions)]`-gated `log!` calls are allowed for development but must not ship.

---

## 11. Testing Strategy

### 11.1 Pure logic tests (no WASM)

Target ≥80% coverage of the new pure-logic surface:

- `ChangeSet` operations (upsert/stage/unstage/discard/iter_*/summary).
- `merge_view(base, changes) -> VirtualFs` — unit tests for each `ChangeType` variant.
- `admin_status` / `can_write_to`.
- GraphQL query serialization (given a `ChangeSet`, assert the generated `fileChanges` JSON).
- Error mapping (HTTP status → `StorageError`).

Pattern: inline `#[cfg(test)] mod tests { ... }`, existing convention.

### 11.2 Mock-backend integration tests

A `MockBackend` struct implements `StorageBackend`:
- Records all `commit(...)` calls.
- Returns configurable `CommitOutcome` or `StorageError`.
- Testable: "calling `sync commit` with staged changes invokes `backend.commit` exactly once with expected args."

These tests cover the command-dispatch path at the level of: `CommandResult → SideEffect → handler side-effect → backend call`. They can run as normal `cargo test` (not WASM), because `spawn_local` has a native-test shim available.

### 11.3 IDB tests — wasm-only

IDB has no native-test shim. Tests that need actual IDB use `wasm-bindgen-test` with `#[wasm_bindgen_test]`. Minimum: persist/hydrate round-trip for `ChangeSet`.

### 11.4 Manual tests

Commit path end-to-end (real GitHub) is manual per-release. A throwaway test repo with a burnable PAT. Covered in the phase-end review checklist (not automated).

---

## 12. Phasing into 3a / 3b / 3c

### 12.1 Boundary rule

Each phase is independently shippable (main branch builds, tests pass, user-facing feature works) and additive (later phases don't break earlier ones). The data model and trait shape defined in this doc are stable across all three — later phases add UI and behavior, not redesign.

### 12.2 Phase 3a — direct commit

**Ships**: `ChangeSet` (all entries staged-by-default), `StorageBackend` trait, `GitHubBackend`, `AppError::Storage`, `admin.rs`, `Mount::is_writable`, commands `touch`/`mkdir`/`rm`/`rmdir`/`edit`/`sync status`/`sync commit`/`sync refresh`/`sync auth`, IndexedDB `drafts` store, **`VirtualFs::serialize_manifest` with byte-stable output + round-trip golden test**, Phase 4 Option B rebrand (prerequisite, can land in same PR).

**Skips**: staging UI, `sync add`/`reset`/`discard`, SyncPanel component, blob store, binary uploads, Reader edit mode.

**Minimum editor**: `edit <path>` command opens a modal with a `<textarea>` + Save/Cancel. No preview. On Save, dispatches `ApplyChange`. This is the placeholder-on-reader's `on_edit` wired to open the modal.

**End-of-3a experience**: admin can edit a markdown file, see the change reflected immediately in merged view, and run `sync commit -m "msg"` to push atomically to GitHub. Reload preserves drafts.

### 12.3 Phase 3b — staging + overlay UI

**Ships**: `ChangeSet.staged` flag becomes user-visible. Commands `sync add`/`reset`/`discard`. `SyncPanel` component with staged/unstaged sections. Conflict recovery UX (`sync refresh` preserves drafts, re-bases).

**Changes from 3a**: `upsert` default changes from `staged=true` to `staged=false`. `sync commit -m` gains a behavior split: with no `-a` flag, commits only staged; with `-a`, stages-all-then-commits (one-shot for single-shot edits).

### 12.4 Phase 3c — rich editor + binaries

**Ships**: Reader edit mode (3-mode switcher), Reader preview mode, markdown image rewriting with blob URLs, binary upload via drag-drop / file-picker, `ChangeType::CreateBinary`, IDB `blobs` store, GraphQL ↔ Git Data API size-threshold fallback (>10 MB).

### 12.5 Cross-cutting: Phase 4 Option B

Prerequisite for 3a. Deliverables (independent PR, lands before 3a):
- Rename `EncryptionInfo` → `AccessFilter`.
- Rename `wrapped_keys` → `recipients` (or similar; design decision in the rename PR).
- Remove `"AES-256-GCM"` string fields from any serialized metadata.
- README / help text updates — drop any "cryptographic" claim. Lock icon retained as "listed-recipients-only" hint.
- Existing read permission check in `filesystem.rs:410` continues to work with the renamed fields.

This is standalone — no dependency on Phase 3 state model — and should merge first.

---

## 13. Anti-Patterns (Explicit Rejections from WIP)

These WIP patterns are **not** adopted. Recording here so reviewers can cross-check implementation.

| WIP pattern | Why rejected |
|---|---|
| `PendingChanges` + `StagedChanges` as two separate collections | Sync-drift bug (staged can reference non-pending path). Replaced with unified `ChangeSet { entries: BTreeMap<Path, Entry { staged: bool }> }`. |
| `FsState::get()` that clones entire `VirtualFs` per call | Performance anti-pattern. Replaced with `Memo<Rc<VirtualFs>>` (ctx.view_fs). |
| `CommandResult` reshaped to `{ output, navigate_to, pending, staged }` (lost `exit_code`) | Breaks Phase 1 contract. Replaced with: Phase 1 shape preserved, mutations flow through extended `SideEffect`. |
| `execute_sync_commit` returns a stub message, actual commit fires from UI button independently | Two mutation paths, CLI is lying about what it did. Replaced with: async via `SideEffect::Commit` handler + `spawn_local`. |
| 14 `console::log_1` debug calls in save hot-path | Noise, can leak data. Replaced with: `console.error` for errors only; gated `log!` for dev. |
| `let _ = save_pending_changes(p)` silent error swallow | Silent failures. Replaced with: `QuotaExceededError` surfaces to user; other errors logged + surfaced. |
| Binary content in `localStorage` base64 | Quota bomb. Replaced with: IndexedDB `blobs` store, native Blob support. |
| Non-atomic per-file Contents API commits | Partial-failure leaves dirty repo. Replaced with: atomic GraphQL `createCommitOnBranch` (+ Git Data API fallback >10 MB). |
| No `expected_head` concurrency check | Silent clobber. Replaced with: `expectedHeadOid` CAS + Conflict error → `sync refresh`. |
| No rate-limit handling | Commits fail silently on 429. Replaced with: explicit `RateLimited` error with retry-after seconds surfaced to user. |
| No discard confirmation | Accidental data loss. Replaced with: `sync discard <path>` requires path (no bulk-discard without `--all`); UI discard has confirm dialog. |

---

## 14. Open Questions Intentionally Deferred

Items not decided in this design; to be resolved in the implementation plan or a follow-up design iteration:

1. **`edit` command modal UX details** — modal vs. in-place textarea in Reader; keyboard shortcuts (Ctrl+S = save?); mobile layout. Informed by 3c design pass.
2. **Preview mode pane split ratio** — 50/50 fixed vs. draggable. 3c decision.
3. **Debounce interval for IDB persistence** — 300ms chosen above but may tune during implementation.
4. **Binary size threshold for GraphQL → Git Data API fallback** — 10 MB is the hard limit; safe threshold may be lower (e.g., 5 MB) to leave room for query overhead. Decide at 3c implementation.
5. **Directory creation semantics** — GitHub has no empty directories. `mkdir foo` with no subsequent files: error at `sync commit` or accept and silently drop? Decide at 3a implementation.
6. **`sync commit --all`** — stages-all-then-commits one-shot shorthand. 3b nice-to-have, not required.
7. **Single-mount assumption in `ctx.changes` / `ctx.backend` / `ctx.remote_head`.** The design treats the context as if there is exactly one writable mount at a time (currently true: only `~`). Multi-mount writability — editing under two different writable mounts in one session — is not supported by the current shapes: a single `RwSignal<ChangeSet>` conflates cross-mount state, and `StoredValue<Option<Arc<dyn StorageBackend>>>` holds one backend. The forward path when this matters is to key by mount: `RwSignal<HashMap<MountId, ChangeSet>>`, `HashMap<MountId, Arc<dyn StorageBackend>>`, IDB `drafts` already keyed by `mount_id` (§7.2). Recorded here so the first multi-mount write feature revisits `AppContext` rather than working around it.

These do NOT block Phase 3 — the core architecture is stable without their answers.

---

## 15. References

- **Sveltia CMS** source: `src/lib/services/backends/git/github/commits.js` — reference for GraphQL `createCommitOnBranch` usage.
- **GitHub GraphQL `createCommitOnBranch`** — `https://docs.github.com/en/graphql/reference/mutations#createcommitonbranch`.
- **Phase 1 plan**: `docs/superpowers/plans/2026-04-20-phase1-mount-and-command-contracts.md`.
- **Phase 2 master**: `docs/superpowers/plans/2026-04-20-phase2-master.md` (decision log).
- **Phase 3 WIP analysis**: `docs/superpowers/plans/2026-04-20-phase3-wip-analysis.md`.
- **WIP branch**: `origin/wip/january-2026-restructure` — reference only, not a merge target.

---

## 16. Document Status

- §0 Purpose — ✅
- §1 Goals & non-goals — ✅
- §2 Architecture overview — ✅
- §3 State model — ✅
- §4 Storage abstraction — ✅
- §5 Reactive flow — ✅
- §6 Commands & async dispatch — ✅
- §7 Persistence — ✅
- §8 Admin & auth — ✅
- §9 UI layer — ✅
- §10 Error handling — ✅
- §11 Testing — ✅
- §12 Phasing — ✅
- §13 Anti-patterns — ✅
- §14 Deferred questions — ✅
- §15 References — ✅

Pending user review. After user approval → writing-plans skill to produce implementation plan.
