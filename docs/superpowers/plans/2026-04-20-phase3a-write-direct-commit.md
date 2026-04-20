# Phase 3a — Write Capability (Direct Commit) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship end-to-end "edit markdown in browser → atomic commit to GitHub" for an authenticated admin, with IndexedDB-persisted drafts and compare-and-swap conflict detection.

**Architecture:** Phase 1 `CommandResult` / `SideEffect` / `dispatch_side_effect` contracts are preserved. Write ops flow via extended `SideEffect` variants; async (commit / refresh) is `spawn_local`-ed inside the dispatcher. One unified `ChangeSet` (path-keyed `BTreeMap`) replaces WIP's split pending/staged model; for 3a every entry is staged-by-default (staging UI lands in 3b). `GitHubBackend` implements a narrow `StorageBackend { commit, fetch_manifest }` trait using GraphQL `createCommitOnBranch` with `expectedHeadOid` CAS. `VirtualFs::serialize_manifest()` is added so every commit includes a byte-stable `manifest.json`. IndexedDB (via `idb` crate) is the canonical draft store.

**Tech Stack:** Rust → WASM, Leptos 0.8 (CSR), `serde` / `serde_json`, `idb` (IndexedDB), `gloo-timers`, `gloo-net` (already present), `base64` (already present), `js-sys`/`web-sys` (already present), `wasm-bindgen-futures::spawn_local`.

**Source of truth:** `docs/superpowers/specs/2026-04-20-phase3-write-design.md`. Section references in this plan point there (e.g., §3.2 = ChangeSet).

---

## Phase overview

- **Phase 0 — Rebrand prerequisite (3 tasks).** EncryptionInfo→AccessFilter; drop algorithm string; README.
- **Phase 1 — Data model foundation (9 tasks).** Mount::is_writable, ChangeSet, merge_view, admin, StorageBackend trait, StorageError, AppError::Storage, VirtualFs::serialize_manifest + golden.
- **Phase 2 — Storage I/O (7 tasks).** MockBackend, GraphQL body building, GitHubBackend with error mapping, IDB `drafts` + `metadata` stores, hydration + debounced persist effect.
- **Phase 3 — Reactive wiring (6 tasks).** AppContext extensions, initial backend wiring, SideEffect variants, dispatch_side_effect additions (sync + async).
- **Phase 4 — Commands (10 tasks).** Parser redirect, Touch/Mkdir/Rm/Rmdir/Edit/Sync variants, execute_command arms, autocomplete names + `sync` subcommand completion, help text.
- **Phase 5 — UI (3 tasks).** Minimal EditModal (textarea + Save/Cancel), triggered by `OpenEditor` side-effect.
- **Phase 6 — Integration & finalize (3 tasks).** End-to-end MockBackend test, README / CLAUDE.md updates, manual QA checklist.

Each phase ends with a green `cargo test` and a `trunk build --release` before the next phase begins.

---

## Phase 0 — Rebrand prerequisite (Option B)

Phase 4 Option B lands before Phase 3a per spec §12.5. Keep to a single rename + field drop; no semantic change.

### Task 0.1: Rename `EncryptionInfo` → `AccessFilter`, drop `algorithm` field

**Files:**
- Modify: `src/models/filesystem.rs:43-59`
- Modify: `src/models/mod.rs:23`
- Modify: `src/core/filesystem.rs:407-431` (permissions doc + read path)
- Modify: `src/core/filesystem.rs:720-780` (test fixtures that construct the struct)

- [ ] **Step 1: Rename type + fields in `models/filesystem.rs`**

Replace the `EncryptionInfo` + `WrappedKey` block (lines 43–59) with:

```rust
/// Access-control metadata for a file.
///
/// "Access" is advisory — it filters who the UI shows content to. Actual
/// cryptographic confidentiality is NOT provided in Phase 3/4 Option B.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccessFilter {
    /// Wallet addresses listed as recipients.
    pub recipients: Vec<Recipient>,
}

/// A single listed recipient.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Recipient {
    /// Wallet address (checksum or lowercase).
    pub address: String,
}
```

Also rename the `encryption: Option<EncryptionInfo>` field on `FileMetadata` (line 18) and on `FileEntry` (line 118) to `access: Option<AccessFilter>`. Update `is_encrypted(...)` (lines 22–25) to `is_restricted(...)` with body `self.access.is_some()`.

- [ ] **Step 2: Update re-exports**

In `src/models/mod.rs:23`, change:

```rust
pub use filesystem::{EncryptionInfo, FileEntry, WrappedKey};
```

to:

```rust
pub use filesystem::{AccessFilter, FileEntry, Recipient};
```

- [ ] **Step 3: Update the permission check**

In `src/core/filesystem.rs:410–431` replace the body with:

```rust
pub fn get_permissions(&self, entry: &FsEntry, wallet: &WalletState) -> DisplayPermissions {
    let is_dir = entry.is_directory();

    let read = match entry {
        FsEntry::Directory { .. } => true,
        FsEntry::File { meta, .. } => match &meta.access {
            None => true,
            Some(filter) => match wallet {
                WalletState::Connected { address, .. } => filter
                    .recipients
                    .iter()
                    .any(|r| r.address.eq_ignore_ascii_case(address)),
                _ => false,
            },
        },
    };

    let write = false;
    let execute = is_dir;

    DisplayPermissions { is_dir, read, write, execute }
}
```

Update the doc comment (lines 403–409) to remove the word "encrypted" — replace with "access-restricted files require wallet address in the recipients list." Do not mention AES or cryptographic anything.

- [ ] **Step 4: Fix test fixtures in `src/core/filesystem.rs:720-780`**

Replace the two test fixtures that build `EncryptionInfo { algorithm: "AES-256-GCM"... wrapped_keys: ... }` with the new shape:

```rust
use crate::models::AccessFilter;
// ...
access: Some(AccessFilter { recipients: vec![] }),
```

and for the with-recipient variant:

```rust
use crate::models::{AccessFilter, Recipient};
// ...
access: Some(AccessFilter {
    recipients: vec![Recipient {
        address: "0xABC...".to_string(),
    }],
}),
```

Update any `wrapped_keys` / `encrypted_key` / `algorithm` references inside this file accordingly.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test`
Expected: PASS. Any compilation errors at this point are callers outside `models/` and `core/filesystem.rs` — fix them in place (the grep in Task 0.2 will catch them).

- [ ] **Step 6: Commit**

```bash
git add src/models/filesystem.rs src/models/mod.rs src/core/filesystem.rs
git commit -m "refactor(4b): EncryptionInfo → AccessFilter, drop algorithm field"
```

### Task 0.2: Sweep remaining call sites

**Files:**
- Modify: `src/models/terminal.rs`, `src/models/route.rs`, `src/components/explorer/preview/**`, `src/components/explorer/file_list.rs`, `src/core/commands/execute.rs` — anywhere that references the old names.

- [ ] **Step 1: Find every stale reference**

Run via Grep tool (not bash):
- Pattern: `EncryptionInfo|WrappedKey|wrapped_keys|encrypted_key|is_encrypted|\.encryption\b|encryption:` across `src/**/*.rs`.

Expected: a bounded list. Replace each hit:
- `EncryptionInfo` → `AccessFilter`
- `WrappedKey` → `Recipient`
- `wrapped_keys` → `recipients`
- `encrypted_key` → remove (no analog; base64 key is gone)
- `is_encrypted` → `is_restricted`
- `.encryption` (field access) → `.access`

- [ ] **Step 2: Run tests + build**

Run: `cargo test && cargo build --target wasm32-unknown-unknown`
Expected: both green.

- [ ] **Step 3: Commit**

```bash
git add -u
git commit -m "refactor(4b): sweep remaining EncryptionInfo call sites"
```

### Task 0.3: Documentation honesty pass

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md` — the "Wallet Integration" section mentions "ECIES encryption for private content"
- Modify: `src/config.rs` if any help text strings reference encryption

- [ ] **Step 1: README sweep**

Search `README.md` for "encrypt", "ECIES", "AES", "cryptographic", "private". Rewrite any claim that implies content is cryptographically protected. Replacement language: "access-restricted" / "listed recipients" / "recipient filter." The lock icon stays but its tooltip text (if set anywhere) becomes "Restricted: listed recipients only."

- [ ] **Step 2: CLAUDE.md sweep**

In `CLAUDE.md`, replace the line "ECIES encryption for private content" with:

```
- Access filter for restricted content (advisory, non-cryptographic)
```

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md src/config.rs
git commit -m "docs(4b): remove cryptographic claims; document access filter as advisory"
```

**Phase 0 exit check:** `cargo test && cargo build --target wasm32-unknown-unknown && trunk build --release` all green. No string "AES" or "ECIES" remains in the repo (grep check).

---

## Phase 1 — Data model foundation

All data types and pure-logic helpers. No I/O, no signals, no reactive code. Every task is `cargo test` only — we do not touch `src/app.rs` or the wasm build in this phase.

### Task 1.1: `Mount::is_writable()` + `writable` field

**Files:**
- Modify: `src/models/mount.rs:17-45` (Mount enum), `47-156` (impl blocks)
- Modify: `src/config.rs` (the single `Mount::github_with_prefix(...)` call)

- [ ] **Step 1: Write failing test**

Append to `src/models/mount.rs:mod tests`:

```rust
#[test]
fn test_mount_is_writable_github_true() {
    let mount = Mount::GitHub {
        alias: "~".to_string(),
        base_url: "https://example.com".to_string(),
        content_prefix: None,
        writable: true,
    };
    assert!(mount.is_writable());
}

#[test]
fn test_mount_is_writable_github_false() {
    let mount = Mount::GitHub {
        alias: "~".to_string(),
        base_url: "https://example.com".to_string(),
        content_prefix: None,
        writable: false,
    };
    assert!(!mount.is_writable());
}

#[test]
fn test_mount_is_writable_ipfs_false() {
    let mount = Mount::ipfs("data", "QmXyz");
    assert!(!mount.is_writable());
}
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p websh --lib models::mount`
Expected: compilation error (`writable` field missing on `Mount::GitHub`, `is_writable` missing on `Mount`).

- [ ] **Step 3: Add the field + method**

In `src/models/mount.rs`, edit the `Mount::GitHub` variant (lines 18–26):

```rust
    /// GitHub raw content
    GitHub {
        /// URL alias (e.g., "~", "work")
        alias: String,
        /// Base URL for manifest and root
        base_url: String,
        /// Optional prefix for content paths (e.g., "~" if content is in ~/*)
        content_prefix: Option<String>,
        /// Whether this mount accepts write operations (commits).
        writable: bool,
    },
```

Update `Mount::github(...)` (line 50) and `Mount::github_with_prefix(...)` (line 59) constructors to set `writable: false` by default. Add a new constructor `github_writable`:

```rust
    /// Create a writable GitHub mount.
    pub fn github_writable(
        alias: impl Into<String>,
        base_url: impl Into<String>,
        content_prefix: impl Into<String>,
    ) -> Self {
        Self::GitHub {
            alias: alias.into(),
            base_url: base_url.into(),
            content_prefix: Some(content_prefix.into()),
            writable: true,
        }
    }
```

Append to the main `impl Mount` block (after `description()` around line 156):

```rust
    /// Whether this mount supports write operations.
    pub fn is_writable(&self) -> bool {
        match self {
            Self::GitHub { writable, .. } => *writable,
            _ => false,
        }
    }
```

- [ ] **Step 4: Update `config::mount_list()`**

In `src/config.rs`, replace the existing home-mount constructor call with `Mount::github_writable(...)`. Leave all other fields identical.

- [ ] **Step 5: Run tests — green**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/models/mount.rs src/config.rs
git commit -m "feat(mount): add writable flag + Mount::is_writable"
```

### Task 1.2: `ChangeSet` — types + pure methods

**Files:**
- Create: `src/core/changes.rs`
- Modify: `src/core/mod.rs` (add `pub mod changes;`)

- [ ] **Step 1: Write the failing tests first**

Create `src/core/changes.rs` with the test module at the bottom. Write the file with only the tests and empty type stubs so `cargo test` first produces a compile-error, then a failing test. Start with:

```rust
//! ChangeSet — unified tracker for in-progress filesystem edits.
//!
//! See `docs/superpowers/specs/2026-04-20-phase3-write-design.md` §3.2.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::models::{DirectoryMetadata, FileMetadata, VirtualPath};
use crate::utils::current_timestamp;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChangeType {
    CreateFile { content: String, meta: FileMetadata },
    CreateBinary { blob_id: String, mime: String, meta: FileMetadata },
    UpdateFile { content: String, description: Option<String> },
    DeleteFile,
    CreateDirectory { meta: DirectoryMetadata },
    DeleteDirectory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub change: ChangeType,
    pub staged: bool,
    pub timestamp: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    entries: BTreeMap<VirtualPath, Entry>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub creates_staged: usize,
    pub creates_unstaged: usize,
    pub updates_staged: usize,
    pub updates_unstaged: usize,
    pub deletes_staged: usize,
    pub deletes_unstaged: usize,
}

impl Summary {
    pub fn total(&self) -> usize {
        self.creates_staged + self.creates_unstaged
            + self.updates_staged + self.updates_unstaged
            + self.deletes_staged + self.deletes_unstaged
    }
    pub fn total_staged(&self) -> usize {
        self.creates_staged + self.updates_staged + self.deletes_staged
    }
}

impl ChangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert-or-replace a change at `path`. New entries default to `staged = true`
    /// in Phase 3a (this flips to `false` in Phase 3b — spec §12.2/§12.3).
    pub fn upsert(&mut self, path: VirtualPath, change: ChangeType) {
        let entry = Entry {
            change,
            staged: true,
            timestamp: current_timestamp(),
        };
        self.entries.insert(path, entry);
    }

    pub fn stage(&mut self, path: &VirtualPath) {
        if let Some(e) = self.entries.get_mut(path) {
            e.staged = true;
        }
    }

    pub fn unstage(&mut self, path: &VirtualPath) {
        if let Some(e) = self.entries.get_mut(path) {
            e.staged = false;
        }
    }

    pub fn discard(&mut self, path: &VirtualPath) {
        self.entries.remove(path);
    }

    pub fn stage_all(&mut self) {
        for e in self.entries.values_mut() {
            e.staged = true;
        }
    }

    pub fn unstage_all(&mut self) {
        for e in self.entries.values_mut() {
            e.staged = false;
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn get(&self, path: &VirtualPath) -> Option<&Entry> {
        self.entries.get(path)
    }

    pub fn is_staged(&self, path: &VirtualPath) -> bool {
        self.entries.get(path).is_some_and(|e| e.staged)
    }

    pub fn is_deleted(&self, path: &VirtualPath) -> bool {
        matches!(
            self.entries.get(path).map(|e| &e.change),
            Some(ChangeType::DeleteFile | ChangeType::DeleteDirectory)
        )
    }

    pub fn iter_all(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)> {
        self.entries.iter()
    }

    pub fn iter_staged(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)> {
        self.entries.iter().filter(|(_, e)| e.staged)
    }

    pub fn iter_unstaged(&self) -> impl Iterator<Item = (&VirtualPath, &Entry)> {
        self.entries.iter().filter(|(_, e)| !e.staged)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn summary(&self) -> Summary {
        let mut s = Summary::default();
        for (_, e) in self.iter_all() {
            let bucket = match &e.change {
                ChangeType::CreateFile { .. }
                | ChangeType::CreateBinary { .. }
                | ChangeType::CreateDirectory { .. } => {
                    if e.staged { &mut s.creates_staged } else { &mut s.creates_unstaged }
                }
                ChangeType::UpdateFile { .. } => {
                    if e.staged { &mut s.updates_staged } else { &mut s.updates_unstaged }
                }
                ChangeType::DeleteFile | ChangeType::DeleteDirectory => {
                    if e.staged { &mut s.deletes_staged } else { &mut s.deletes_unstaged }
                }
            };
            *bucket += 1;
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    fn create_file(content: &str) -> ChangeType {
        ChangeType::CreateFile {
            content: content.to_string(),
            meta: FileMetadata::default(),
        }
    }

    #[test]
    fn upsert_defaults_staged_true() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("hi"));
        assert!(cs.is_staged(&p("/a.md")));
        assert_eq!(cs.len(), 1);
    }

    #[test]
    fn unstage_then_stage_roundtrip() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("hi"));
        cs.unstage(&p("/a.md"));
        assert!(!cs.is_staged(&p("/a.md")));
        cs.stage(&p("/a.md"));
        assert!(cs.is_staged(&p("/a.md")));
    }

    #[test]
    fn discard_removes_entry() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("hi"));
        cs.discard(&p("/a.md"));
        assert!(cs.get(&p("/a.md")).is_none());
    }

    #[test]
    fn is_deleted_matches_delete_variants() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/gone.md"), ChangeType::DeleteFile);
        cs.upsert(p("/keep.md"), create_file("x"));
        assert!(cs.is_deleted(&p("/gone.md")));
        assert!(!cs.is_deleted(&p("/keep.md")));
    }

    #[test]
    fn iter_all_yields_sorted_order() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/z.md"), create_file("z"));
        cs.upsert(p("/a.md"), create_file("a"));
        cs.upsert(p("/m.md"), create_file("m"));
        let paths: Vec<_> = cs.iter_all().map(|(p, _)| p.as_str().to_string()).collect();
        assert_eq!(paths, vec!["/a.md", "/m.md", "/z.md"]);
    }

    #[test]
    fn iter_staged_filters_unstaged() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), create_file("a"));
        cs.upsert(p("/b.md"), create_file("b"));
        cs.unstage(&p("/b.md"));
        let staged: Vec<_> = cs.iter_staged().map(|(p, _)| p.as_str().to_string()).collect();
        assert_eq!(staged, vec!["/a.md"]);
    }

    #[test]
    fn summary_counts_buckets() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/new.md"), create_file("x"));
        cs.upsert(
            p("/upd.md"),
            ChangeType::UpdateFile { content: "y".into(), description: None },
        );
        cs.upsert(p("/del.md"), ChangeType::DeleteFile);
        cs.unstage(&p("/del.md"));
        let s = cs.summary();
        assert_eq!(s.creates_staged, 1);
        assert_eq!(s.updates_staged, 1);
        assert_eq!(s.deletes_unstaged, 1);
        assert_eq!(s.total(), 3);
        assert_eq!(s.total_staged(), 2);
    }
}
```

> Note: if `VirtualPath::from_absolute` and `VirtualPath::as_str` don't exist with exactly those signatures, use whatever exists today (grep for `impl VirtualPath` in `src/models/virtual_path.rs`). Adjust the test helper `p(...)` accordingly. Do NOT introduce new VirtualPath API here.

- [ ] **Step 2: Register the module**

In `src/core/mod.rs`, add the line: `pub mod changes;` adjacent to the other `pub mod ...` declarations (alphabetical).

- [ ] **Step 3: Run tests — expect pass**

Run: `cargo test -p websh --lib core::changes`
Expected: all 7 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/core/changes.rs src/core/mod.rs
git commit -m "feat(changes): ChangeSet with per-entry staged flag"
```

### Task 1.3: `merge_view(base, changes) -> VirtualFs`

**Files:**
- Create: `src/core/merge.rs`
- Modify: `src/core/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `src/core/merge.rs`:

```rust
//! Merge a `ChangeSet` overlay on top of a base `VirtualFs` to produce a
//! "current view" VirtualFs. Pure, no signals.
//!
//! See spec §3.3.

use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::filesystem::VirtualFs;
use crate::models::{FsEntry, VirtualPath};

pub fn merge_view(base: &VirtualFs, changes: &ChangeSet) -> VirtualFs {
    let mut merged = base.clone();
    for (path, entry) in changes.iter_all() {
        apply_change(&mut merged, path, &entry.change);
    }
    merged
}

fn apply_change(fs: &mut VirtualFs, path: &VirtualPath, change: &ChangeType) {
    match change {
        ChangeType::CreateFile { content, meta } => {
            fs.upsert_file(path.clone(), content.clone(), meta.clone());
        }
        ChangeType::CreateBinary { blob_id, mime, meta } => {
            fs.upsert_binary_placeholder(path.clone(), blob_id.clone(), mime.clone(), meta.clone());
        }
        ChangeType::UpdateFile { content, description } => {
            fs.update_file_content(path, content.clone(), description.clone());
        }
        ChangeType::DeleteFile => {
            fs.remove_entry(path);
        }
        ChangeType::CreateDirectory { meta } => {
            fs.upsert_directory(path.clone(), meta.clone());
        }
        ChangeType::DeleteDirectory => {
            fs.remove_subtree(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FileMetadata;

    fn base() -> VirtualFs {
        // Start with an empty VFS; tests seed via VirtualFs::empty().
        VirtualFs::empty()
    }

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    #[test]
    fn create_file_appears_in_merged() {
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/note.md"),
            ChangeType::CreateFile {
                content: "hi".into(),
                meta: FileMetadata::default(),
            },
        );
        let merged = merge_view(&base(), &cs);
        assert!(merged.get_entry(&p("/note.md")).is_some());
    }

    #[test]
    fn delete_removes_from_merged() {
        let mut fs = base();
        fs.upsert_file(p("/a.md"), "a".into(), FileMetadata::default());
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), ChangeType::DeleteFile);
        let merged = merge_view(&fs, &cs);
        assert!(merged.get_entry(&p("/a.md")).is_none());
    }

    #[test]
    fn update_replaces_content() {
        let mut fs = base();
        fs.upsert_file(p("/a.md"), "old".into(), FileMetadata::default());
        let mut cs = ChangeSet::new();
        cs.upsert(
            p("/a.md"),
            ChangeType::UpdateFile { content: "new".into(), description: None },
        );
        let merged = merge_view(&fs, &cs);
        let content = merged.read_file(&p("/a.md")).unwrap();
        assert_eq!(content, "new");
    }
}
```

> If `VirtualFs` does not yet expose `upsert_file` / `upsert_directory` / `remove_entry` / `remove_subtree` / `update_file_content` / `upsert_binary_placeholder` / `read_file`, add those as thin methods on `VirtualFs` in `src/core/filesystem.rs`. Implement by directly mutating the internal `BTreeMap<VirtualPath, FsEntry>` (or the equivalent — see the existing `fn get_entry(...)` to find the underlying storage). These are all O(log n) map operations. Keep each method under 15 lines.

- [ ] **Step 2: Add the required VirtualFs helpers**

Inspect `src/core/filesystem.rs` — find the inner map. Add the mutation helpers listed above next to existing `get_entry`. Each is a tiny map operation. If the internal representation does not support file content mutation directly (e.g. content is stored separately from the tree), add a small helper on `FsEntry` or on the storage map. Do not restructure existing code.

- [ ] **Step 3: Register module**

In `src/core/mod.rs`, add `pub mod merge;`.

- [ ] **Step 4: Run tests — green**

Run: `cargo test -p websh --lib core::merge`
Expected: 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/core/merge.rs src/core/filesystem.rs src/core/mod.rs
git commit -m "feat(merge): pure merge_view(base, changes) -> VirtualFs"
```

### Task 1.4: `admin.rs` — admin status + can_write_to

**Files:**
- Create: `src/core/admin.rs`
- Modify: `src/core/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `src/core/admin.rs`:

```rust
//! Admin eligibility. See spec §8.2.

use crate::models::{Mount, WalletState};

/// Hard-coded allowlist. Single-admin model per design.
///
/// Store lowercased. `is_admin` compares case-insensitively.
const ADMIN_ADDRESSES: &[&str] = &[
    // Filled by operator via config.rs; see `config::admin_addresses()` if you
    // prefer that. For Phase 3a, this constant is the source of truth.
    "0x0000000000000000000000000000000000000000", // placeholder — replace with real admin address
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdminStatus {
    NotConnected,
    Connected { address: String },
    Admin { address: String },
}

pub fn admin_status(wallet: &WalletState) -> AdminStatus {
    match wallet {
        WalletState::Connected { address, .. } => {
            let lower = address.to_ascii_lowercase();
            if ADMIN_ADDRESSES.iter().any(|a| a.eq_ignore_ascii_case(&lower)) {
                AdminStatus::Admin { address: address.clone() }
            } else {
                AdminStatus::Connected { address: address.clone() }
            }
        }
        _ => AdminStatus::NotConnected,
    }
}

pub fn can_write_to(wallet: &WalletState, mount: &Mount) -> bool {
    matches!(admin_status(wallet), AdminStatus::Admin { .. }) && mount.is_writable()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_is_not_admin() {
        assert_eq!(admin_status(&WalletState::Disconnected), AdminStatus::NotConnected);
    }

    #[test]
    fn non_allowlisted_connected_is_not_admin() {
        let w = WalletState::Connected {
            address: "0xdeadbeef".to_string(),
            ens_name: None,
            chain_id: 1,
        };
        assert!(matches!(admin_status(&w), AdminStatus::Connected { .. }));
    }

    #[test]
    fn allowlisted_is_admin() {
        let w = WalletState::Connected {
            address: ADMIN_ADDRESSES[0].to_ascii_uppercase(),
            ens_name: None,
            chain_id: 1,
        };
        assert!(matches!(admin_status(&w), AdminStatus::Admin { .. }));
    }

    #[test]
    fn can_write_requires_both() {
        let admin = WalletState::Connected {
            address: ADMIN_ADDRESSES[0].to_string(),
            ens_name: None,
            chain_id: 1,
        };
        let writable = Mount::github_writable("~", "https://x", "~");
        let readonly = Mount::github_with_prefix("ro", "https://y", "~");
        assert!(can_write_to(&admin, &writable));
        assert!(!can_write_to(&admin, &readonly));
    }
}
```

> If `WalletState::Connected`'s exact field list differs (e.g. `chain_id: u64`), match the real type verbatim. Don't invent fields.

- [ ] **Step 2: Register module**

In `src/core/mod.rs`, add `pub mod admin;`.

- [ ] **Step 3: Run tests — green**

Run: `cargo test -p websh --lib core::admin`
Expected: 4 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/core/admin.rs src/core/mod.rs
git commit -m "feat(admin): AdminStatus, admin_status, can_write_to"
```

### Task 1.5: `StorageError` + `StorageResult`

**Files:**
- Create: `src/core/storage/mod.rs`
- Create: `src/core/storage/error.rs`
- Modify: `src/core/mod.rs`

- [ ] **Step 1: Create the storage module skeleton**

Create `src/core/storage/mod.rs`:

```rust
//! Storage abstraction for write operations. See spec §4.

mod backend;
mod error;

pub use backend::{BoxFuture, CommitOutcome, StorageBackend};
pub use error::{StorageError, StorageResult};
```

Create `src/core/storage/error.rs`:

```rust
use std::fmt;

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageError {
    AuthFailed,
    Conflict { remote_head: String },
    NotFound(String),
    ValidationFailed(String),
    RateLimited { retry_after: Option<u64> },
    ServerError(u16),
    NetworkError(String),
    NoToken,
    BadRequest(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthFailed => write!(f, "token invalid or lacks permission"),
            Self::Conflict { remote_head } => {
                write!(f, "remote changed (now {}). run 'sync refresh'",
                    &remote_head[..remote_head.len().min(8)])
            }
            Self::NotFound(p) => write!(f, "path not found on remote: {p}"),
            Self::ValidationFailed(m) => write!(f, "rejected by remote: {m}"),
            Self::RateLimited { retry_after: Some(n) } => write!(f, "rate limited. try again in {n}s"),
            Self::RateLimited { retry_after: None } => write!(f, "rate limited"),
            Self::ServerError(c) => write!(f, "remote server error (HTTP {c})"),
            Self::NetworkError(m) => write!(f, "network error: {m}"),
            Self::NoToken => write!(f, "no GitHub token. run 'sync auth <token>'"),
            Self::BadRequest(m) => write!(f, "bad request: {m}"),
        }
    }
}

impl std::error::Error for StorageError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_conflict_truncates_sha_to_8() {
        let e = StorageError::Conflict { remote_head: "abcdef1234567890".to_string() };
        assert_eq!(e.to_string(), "remote changed (now abcdef12). run 'sync refresh'");
    }

    #[test]
    fn display_rate_limited_with_retry() {
        let e = StorageError::RateLimited { retry_after: Some(30) };
        assert_eq!(e.to_string(), "rate limited. try again in 30s");
    }
}
```

- [ ] **Step 2: Register + run tests**

In `src/core/mod.rs`, add `pub mod storage;`.

Run: `cargo test -p websh --lib core::storage::error`
Expected: 2 tests PASS. But this will fail compilation until Task 1.6 adds `backend.rs` — that's fine, skip this step's cargo test and move straight to 1.6. (Or temporarily create an empty `backend.rs` stub with `pub struct _Placeholder;`.)

Actually, just create the empty `backend.rs` stub now to keep the phase green:

```rust
// src/core/storage/backend.rs — stub; Task 1.6 fills in.
use std::future::Future;
use std::pin::Pin;
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;
pub struct CommitOutcome;
pub trait StorageBackend {}
```

Run: `cargo test -p websh --lib core::storage`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add src/core/storage/ src/core/mod.rs
git commit -m "feat(storage): StorageError, StorageResult, trait skeleton"
```

### Task 1.6: `StorageBackend` trait + `CommitOutcome`

**Files:**
- Modify: `src/core/storage/backend.rs`

- [ ] **Step 1: Replace the stub with the real trait**

Replace the contents of `src/core/storage/backend.rs`:

```rust
use std::future::Future;
use std::pin::Pin;

use crate::core::changes::ChangeSet;
use crate::models::{Manifest, VirtualPath};

use super::error::StorageResult;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub struct CommitOutcome {
    pub new_head: String,
    /// Some if the backend produced a manifest synchronously as part of the
    /// commit response. GitHubBackend returns `None` (GraphQL's
    /// `createCommitOnBranch` does not echo file contents); the dispatcher
    /// re-fetches via `fetch_manifest()` after a successful commit.
    pub manifest: Option<Manifest>,
    pub committed_paths: Vec<VirtualPath>,
}

pub trait StorageBackend {
    fn backend_type(&self) -> &'static str;

    /// Commit the staged subset of `changes` as one atomic batch.
    /// `expected_head` is the SHA the caller believed was current at draft-time.
    fn commit<'a>(
        &'a self,
        changes: &'a ChangeSet,
        message: &'a str,
        expected_head: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>>;

    fn fetch_manifest(&self) -> BoxFuture<'_, StorageResult<Manifest>>;
}
```

- [ ] **Step 2: Build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add src/core/storage/backend.rs
git commit -m "feat(storage): StorageBackend trait with commit/fetch_manifest"
```

### Task 1.7: `AppError::Storage` variant + From impl

**Files:**
- Modify: `src/core/error.rs`

- [ ] **Step 1: Write the failing test**

Append to `src/core/error.rs:mod tests`:

```rust
#[test]
fn app_error_from_storage_error() {
    let se = crate::core::storage::StorageError::NoToken;
    let ae: AppError = se.into();
    assert!(matches!(ae, AppError::Storage(_)));
}
```

Run: `cargo test -p websh --lib core::error`
Expected: FAIL — variant missing.

- [ ] **Step 2: Add the variant + From impl**

Find the `AppError` enum in `src/core/error.rs`. Add after the existing variants:

```rust
    Storage(crate::core::storage::StorageError),
```

and append:

```rust
impl From<crate::core::storage::StorageError> for AppError {
    fn from(e: crate::core::storage::StorageError) -> Self {
        Self::Storage(e)
    }
}
```

Also extend the `Display` impl (find where other variants are rendered) with an arm that forwards to the inner `StorageError`'s Display:

```rust
    Self::Storage(e) => write!(f, "storage: {e}"),
```

- [ ] **Step 3: Run tests — green**

Run: `cargo test -p websh --lib core::error`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add src/core/error.rs
git commit -m "feat(error): AppError::Storage variant + From impl"
```

### Task 1.8: `VirtualFs::serialize_manifest()` — byte-stable output

**Files:**
- Modify: `src/core/filesystem.rs`
- Modify: `src/models/filesystem.rs` (only if `FileEntry` serialization needs `#[serde(...)]` tightening — audit first)
- Create: `tests/fixtures/manifest_golden.json` (golden fixture)
- Create: `tests/manifest_roundtrip.rs` (integration test)

- [ ] **Step 1: Confirm the actual Manifest schema — READ FIRST, CODE SECOND**

Before drafting any code below, open and read the real definitions — the code blocks in this task are illustrative, not authoritative:

1. `grep -rn "struct Manifest" src/models/` → find the exact file.
2. Read that file in full (typical candidates: `src/models/manifest.rs`, `src/models/filesystem.rs`).
3. Read `src/models/filesystem.rs` for `FileEntry` / `FsEntry` / `DirectoryMetadata`.
4. Write down, on paper or in scratch, the **complete** field list for `Manifest`, `FileEntry`, and anything `Manifest` directly owns. Note every `#[serde(...)]` attribute.

Only proceed once you can answer: "what exact struct will `serialize_manifest` return, and how is each field populated from the current `VirtualFs` state?" If any field can't be reconstructed losslessly from `VirtualFs` (because `VirtualFs` dropped the info at load time), that's a gap — surface it before writing code. Either extend `VirtualFs` to keep the field, or document the known-lossy field as a spec gap.

Then, in the same pass, audit for nondeterministic sources:

- `HashMap<K, V>` fields → replace with `BTreeMap<K, V>` or enforce sorted iteration before serialization.
- Optional `#[serde(skip_serializing_if = "Option::is_none")]` consistency — either all Options skip or none do; mixed is fine but commit to one choice per field documented at the type.
- Integer types: `f64` → don't use for anything persisted (fine for `size: u64` etc.).

If any field is a `HashMap`, leave it but document that `serialize_manifest` sorts before emission (next step).

- [ ] **Step 2: Implement `serialize_manifest`**

Add to `src/core/filesystem.rs` (near `from_manifest`):

```rust
impl VirtualFs {
    /// Re-serialize the current VFS state into a Manifest suitable for commit.
    ///
    /// **Byte-stable**: the same logical state must produce identical bytes
    /// across sessions/machines/rust versions. See spec §4.2.
    pub fn serialize_manifest(&self) -> crate::models::Manifest {
        // Walk internal storage in sorted key order. If internal map is already
        // a BTreeMap, iter() is sorted. If it's a HashMap, collect keys to a
        // Vec and sort().
        let mut files: Vec<crate::models::FileEntry> = self
            .iter_files()  // add this helper if it doesn't exist
            .map(|(path, entry)| crate::models::FileEntry::from_fs(path, entry))
            .collect();
        files.sort_by(|a, b| a.path.cmp(&b.path));

        // Similarly for directories if the manifest schema has them as a
        // separate field.
        crate::models::Manifest {
            files,
            // other manifest fields — keep current defaults / copy from self
        }
    }
}
```

> If `Manifest` has fields beyond `files`, populate them from whatever source you have (e.g. `self.root_metadata` etc.). If you don't have enough info to populate a field losslessly, that's a spec gap — document it in the plan's Open Questions section of the project tracker, but for now default to `Default::default()` only when the field is not round-trip-critical (safe for anything the reader treats as optional).
>
> Helpers to add on `VirtualFs`: `iter_files(&self) -> impl Iterator<Item = (&VirtualPath, &FsEntry)>` if not already present.
>
> Helper on `FileEntry`: `fn from_fs(path: &VirtualPath, entry: &FsEntry) -> FileEntry` — converts runtime representation back to manifest representation. Straightforward field copy.

- [ ] **Step 3: Write the round-trip test**

Create `tests/manifest_roundtrip.rs`:

```rust
//! Byte-stable round-trip: manifest.json → VirtualFs → serialize_manifest
//! → bytes. Same bytes in, same bytes out.

use websh::core::filesystem::VirtualFs;
use websh::models::Manifest;

#[test]
fn manifest_roundtrip_is_byte_stable() {
    let golden = include_str!("fixtures/manifest_golden.json");
    let manifest: Manifest = serde_json::from_str(golden).expect("golden parses");
    let fs = VirtualFs::from_manifest(manifest);

    let reserialized = fs.serialize_manifest();
    let out = serde_json::to_string_pretty(&reserialized).expect("serialize");

    // Trim trailing whitespace for robustness against editor newlines.
    assert_eq!(out.trim_end(), golden.trim_end());
}
```

- [ ] **Step 4: Create the golden fixture**

Create `tests/fixtures/manifest_golden.json` with a minimal but non-trivial fixture — at least two files in non-alphabetical order in the source array (so the test proves we sort before emit), one file with access metadata, one directory metadata entry:

```json
{
  "files": [
    {
      "path": "b.md",
      "title": "B",
      "size": 4,
      "modified": 1700000000,
      "tags": [],
      "access": null
    },
    {
      "path": "a.md",
      "title": "A",
      "size": 2,
      "modified": 1700000001,
      "tags": ["intro"],
      "access": {
        "recipients": [
          { "address": "0xabc" }
        ]
      }
    }
  ]
}
```

> Then **run the test once, fail, copy the actual output as the new golden, re-run, pass.** This bootstraps the fixture to match whatever our serializer actually produces — which is the definition of byte-stable for this codebase.

- [ ] **Step 5: Run the test**

Run: `cargo test --test manifest_roundtrip`
Expected: FAIL first (contents don't match sorted output), then after copying actual output into the fixture: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/core/filesystem.rs tests/manifest_roundtrip.rs tests/fixtures/manifest_golden.json
git commit -m "feat(fs): VirtualFs::serialize_manifest with byte-stable golden test"
```

### Task 1.9: Phase 1 exit check

- [ ] **Step 1: Full build + test**

Run: `cargo test && cargo build --target wasm32-unknown-unknown`
Expected: all green.

- [ ] **Step 2: Commit nothing (checkpoint only)**

If everything passes, Phase 1 is done. If anything fails, fix in place and amend the preceding commit (same phase, coherent step).

---

## Phase 2 — Storage I/O

Real backends now: a `MockBackend` for testing, a `GitHubBackend` for production, and IndexedDB persistence. This phase introduces the `idb` dependency.

### Task 2.1: `MockBackend` — for commit-path integration tests

**Files:**
- Create: `src/core/storage/mock.rs`
- Modify: `src/core/storage/mod.rs`

- [ ] **Step 1: Write the test that uses it first**

Append to `src/core/storage/mod.rs`:

```rust
#[cfg(test)]
mod mock;
#[cfg(test)]
pub use mock::MockBackend;
```

Create `src/core/storage/mock.rs`:

```rust
//! In-memory backend for commit-path tests. Not shipped in WASM build.

use std::cell::RefCell;

use crate::core::changes::ChangeSet;
use crate::models::{Manifest, VirtualPath};

use super::backend::{BoxFuture, CommitOutcome, StorageBackend};
use super::error::{StorageError, StorageResult};

#[derive(Default)]
pub struct MockBackend {
    pub commit_calls: RefCell<Vec<CommitRecord>>,
    pub next_outcome: RefCell<Option<StorageResult<CommitOutcome>>>,
    pub next_manifest: RefCell<Option<StorageResult<Manifest>>>,
}

pub struct CommitRecord {
    pub message: String,
    pub expected_head: Option<String>,
    pub paths: Vec<VirtualPath>,
}

impl MockBackend {
    pub fn with_success(manifest: Manifest, new_head: impl Into<String>) -> Self {
        let outcome = CommitOutcome {
            new_head: new_head.into(),
            manifest: manifest.clone(),
            committed_paths: vec![],
        };
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(Some(Ok(outcome))),
            next_manifest: RefCell::new(Some(Ok(manifest))),
        }
    }

    pub fn with_conflict(head: impl Into<String>) -> Self {
        Self {
            commit_calls: RefCell::new(vec![]),
            next_outcome: RefCell::new(Some(Err(StorageError::Conflict {
                remote_head: head.into(),
            }))),
            next_manifest: RefCell::new(Some(Ok(Manifest::default()))),
        }
    }
}

impl StorageBackend for MockBackend {
    fn backend_type(&self) -> &'static str { "mock" }

    fn commit<'a>(
        &'a self,
        changes: &'a ChangeSet,
        message: &'a str,
        expected_head: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
        let paths: Vec<VirtualPath> = changes
            .iter_staged()
            .map(|(p, _)| p.clone())
            .collect();
        self.commit_calls.borrow_mut().push(CommitRecord {
            message: message.to_string(),
            expected_head: expected_head.map(|s| s.to_string()),
            paths,
        });
        let outcome = self.next_outcome.borrow_mut().take()
            .unwrap_or_else(|| Err(StorageError::BadRequest("no outcome queued".into())));
        Box::pin(async move { outcome })
    }

    fn fetch_manifest(&self) -> BoxFuture<'_, StorageResult<Manifest>> {
        let m = self.next_manifest.borrow_mut().take()
            .unwrap_or_else(|| Ok(Manifest::default()));
        Box::pin(async move { m })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::changes::{ChangeSet, ChangeType};
    use crate::models::{FileMetadata, Manifest};

    #[tokio::test(flavor = "current_thread")]
    async fn mock_records_commit_args() {
        let mut cs = ChangeSet::new();
        let p = VirtualPath::from_absolute("/a.md").unwrap();
        cs.upsert(p.clone(), ChangeType::CreateFile {
            content: "x".into(),
            meta: FileMetadata::default(),
        });

        let backend = MockBackend::with_success(Manifest::default(), "sha-new");
        let out = backend.commit(&cs, "msg", Some("sha-old")).await.unwrap();
        assert_eq!(out.new_head, "sha-new");

        let calls = backend.commit_calls.borrow();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].message, "msg");
        assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
        assert_eq!(calls[0].paths, vec![p]);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mock_conflict_is_returned() {
        let cs = ChangeSet::new();
        let backend = MockBackend::with_conflict("sha-remote");
        let err = backend.commit(&cs, "m", None).await.unwrap_err();
        assert!(matches!(err, StorageError::Conflict { .. }));
    }
}
```

> The `tokio` dev-dep may not exist yet. If it doesn't, add it to `[dev-dependencies]` in `Cargo.toml`: `tokio = { version = "1", features = ["macros", "rt"] }`. Tokio is dev-only and does NOT bloat the WASM ship.

- [ ] **Step 2: Run tests — green**

Run: `cargo test -p websh --lib core::storage::mock`
Expected: 2 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add src/core/storage/mock.rs src/core/storage/mod.rs Cargo.toml
git commit -m "test(storage): MockBackend for commit-path integration tests"
```

### Task 2.2: GraphQL body building — pure function

**Files:**
- Create: `src/core/storage/github/mod.rs`
- Create: `src/core/storage/github/graphql.rs`
- Modify: `src/core/storage/mod.rs`

- [ ] **Step 1: Write the failing test first**

Create `src/core/storage/github/mod.rs`:

```rust
pub(crate) mod graphql;
// client.rs will come in Task 2.3
```

In `src/core/storage/mod.rs`, add `mod github;` (under existing mods; do not re-export yet).

Create `src/core/storage/github/graphql.rs`:

```rust
//! Pure building of GraphQL commit payloads. No HTTP, no signals.
//! See spec §4.2.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Serialize;

use crate::core::changes::{ChangeSet, ChangeType};
use crate::models::VirtualPath;

#[derive(Debug, Serialize)]
pub struct CreateCommitInput {
    pub branch: BranchRef,
    pub message: CommitMessage,
    #[serde(rename = "expectedHeadOid", skip_serializing_if = "Option::is_none")]
    pub expected_head_oid: Option<String>,
    #[serde(rename = "fileChanges")]
    pub file_changes: FileChanges,
}

#[derive(Debug, Serialize)]
pub struct BranchRef {
    #[serde(rename = "repositoryNameWithOwner")]
    pub repo_with_owner: String,
    #[serde(rename = "branchName")]
    pub branch_name: String,
}

#[derive(Debug, Serialize)]
pub struct CommitMessage {
    pub headline: String,
}

#[derive(Debug, Default, Serialize)]
pub struct FileChanges {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub additions: Vec<FileAddition>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deletions: Vec<FileDeletion>,
}

#[derive(Debug, Serialize)]
pub struct FileAddition {
    pub path: String,
    pub contents: String, // base64
}

#[derive(Debug, Serialize)]
pub struct FileDeletion {
    pub path: String,
}

/// Build the fileChanges payload from the STAGED subset of the ChangeSet.
///
/// `repo_prefix` is prepended to each VirtualPath before emission — GitHub
/// paths are repo-relative, but our VirtualPath is mount-relative, so callers
/// pass the mount's `content_prefix`.
pub fn build_file_changes(
    changes: &ChangeSet,
    repo_prefix: &str,
    serialized_manifest: Option<(&str, &str)>, // (repo_path, body_bytes_utf8)
) -> FileChanges {
    let mut fc = FileChanges::default();

    for (path, entry) in changes.iter_staged() {
        let repo_path = join_repo_path(repo_prefix, path);
        match &entry.change {
            ChangeType::CreateFile { content, .. }
            | ChangeType::UpdateFile { content, .. } => {
                fc.additions.push(FileAddition {
                    path: repo_path,
                    contents: B64.encode(content.as_bytes()),
                });
            }
            ChangeType::CreateBinary { .. } => {
                // 3c — not reachable in 3a
                continue;
            }
            ChangeType::DeleteFile => {
                fc.deletions.push(FileDeletion { path: repo_path });
            }
            ChangeType::CreateDirectory { .. } | ChangeType::DeleteDirectory => {
                // GitHub has no empty directories; implicit. Design §4.2.
                continue;
            }
        }
    }

    if let Some((path, body)) = serialized_manifest {
        fc.additions.push(FileAddition {
            path: path.to_string(),
            contents: B64.encode(body.as_bytes()),
        });
    }

    // Sort both lists by path for deterministic GraphQL bodies.
    fc.additions.sort_by(|a, b| a.path.cmp(&b.path));
    fc.deletions.sort_by(|a, b| a.path.cmp(&b.path));

    fc
}

fn join_repo_path(prefix: &str, path: &VirtualPath) -> String {
    let tail = path.as_str().trim_start_matches('/');
    if prefix.is_empty() {
        tail.to_string()
    } else {
        format!("{}/{}", prefix.trim_matches('/'), tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::changes::ChangeType;
    use crate::models::FileMetadata;

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    #[test]
    fn additions_are_sorted_and_base64() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/z.md"), ChangeType::CreateFile {
            content: "zz".into(), meta: FileMetadata::default(),
        });
        cs.upsert(p("/a.md"), ChangeType::CreateFile {
            content: "aa".into(), meta: FileMetadata::default(),
        });
        let fc = build_file_changes(&cs, "~", None);
        assert_eq!(fc.additions.len(), 2);
        assert_eq!(fc.additions[0].path, "~/a.md");
        assert_eq!(fc.additions[1].path, "~/z.md");
        assert_eq!(fc.additions[0].contents, B64.encode(b"aa"));
    }

    #[test]
    fn deletions_are_emitted() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/gone.md"), ChangeType::DeleteFile);
        let fc = build_file_changes(&cs, "", None);
        assert_eq!(fc.deletions.len(), 1);
        assert_eq!(fc.deletions[0].path, "gone.md");
        assert!(fc.additions.is_empty());
    }

    #[test]
    fn unstaged_is_excluded() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/a.md"), ChangeType::CreateFile {
            content: "a".into(), meta: FileMetadata::default(),
        });
        cs.unstage(&p("/a.md"));
        let fc = build_file_changes(&cs, "", None);
        assert!(fc.additions.is_empty());
    }

    #[test]
    fn manifest_is_appended_and_sorted_in() {
        let mut cs = ChangeSet::new();
        cs.upsert(p("/b.md"), ChangeType::CreateFile {
            content: "b".into(), meta: FileMetadata::default(),
        });
        let fc = build_file_changes(&cs, "", Some(("manifest.json", "{}")));
        let paths: Vec<_> = fc.additions.iter().map(|a| a.path.as_str()).collect();
        assert_eq!(paths, vec!["b.md", "manifest.json"]);
    }

    #[test]
    fn directory_creates_are_dropped() {
        use crate::models::DirectoryMetadata;
        let mut cs = ChangeSet::new();
        cs.upsert(p("/newdir"), ChangeType::CreateDirectory {
            meta: DirectoryMetadata::default(),
        });
        let fc = build_file_changes(&cs, "", None);
        assert!(fc.additions.is_empty());
        assert!(fc.deletions.is_empty());
    }
}
```

- [ ] **Step 2: Ensure `base64` is a dep**

Check `Cargo.toml` — `base64` should already be in `[dependencies]` per existing usage (ECIES was using it). If not, add:

```toml
base64 = "0.22"
```

- [ ] **Step 3: Run tests — green**

Run: `cargo test -p websh --lib core::storage::github::graphql`
Expected: 5 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/core/storage/github/ src/core/storage/mod.rs
git commit -m "feat(github): build_file_changes — pure GraphQL payload builder"
```

### Task 2.3: `GitHubBackend` — HTTP + error mapping

**Files:**
- Create: `src/core/storage/github/client.rs`
- Modify: `src/core/storage/github/mod.rs`
- Modify: `src/core/storage/mod.rs` (export `GitHubBackend`)

- [ ] **Step 1: Write the failing error-mapping tests first**

Create `src/core/storage/github/client.rs`:

```rust
//! GitHub backend — GraphQL createCommitOnBranch + manifest fetch.
//! See spec §4.2 / §4.3.

use serde::{Deserialize, Serialize};

use crate::core::changes::ChangeSet;
use crate::core::storage::{BoxFuture, CommitOutcome, StorageBackend, StorageError, StorageResult};
use crate::models::Manifest;

use super::graphql::{
    build_file_changes, BranchRef, CommitMessage, CreateCommitInput,
};

pub struct GitHubBackend {
    pub repo_with_owner: String,  // "0xwonj/db"
    pub branch: String,           // "main"
    pub content_prefix: String,   // mount's content_prefix, e.g., "~"
    pub manifest_url: String,     // full URL to manifest.json (raw.githubusercontent.com)
    token: String,
}

impl GitHubBackend {
    pub fn new(
        repo_with_owner: impl Into<String>,
        branch: impl Into<String>,
        content_prefix: impl Into<String>,
        manifest_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            repo_with_owner: repo_with_owner.into(),
            branch: branch.into(),
            content_prefix: content_prefix.into(),
            manifest_url: manifest_url.into(),
            token: token.into(),
        }
    }
}

#[derive(Serialize)]
struct GraphQLRequest<'a> {
    query: &'static str,
    variables: GraphQLVariables<'a>,
}

#[derive(Serialize)]
struct GraphQLVariables<'a> {
    input: &'a CreateCommitInput,
}

#[derive(Deserialize)]
struct GraphQLResponse {
    data: Option<GraphQLData>,
    #[serde(default)]
    errors: Vec<GraphQLErrorItem>,
}

#[derive(Deserialize)]
struct GraphQLData {
    #[serde(rename = "createCommitOnBranch")]
    create_commit_on_branch: Option<CreateCommitResult>,
}

#[derive(Deserialize)]
struct CreateCommitResult {
    commit: CommitOid,
}

#[derive(Deserialize)]
struct CommitOid { oid: String }

#[derive(Deserialize)]
struct GraphQLErrorItem {
    message: String,
    #[serde(rename = "type", default)]
    err_type: Option<String>,
}

const MUTATION: &str = "\
mutation ($input: CreateCommitOnBranchInput!) {
  createCommitOnBranch(input: $input) {
    commit { oid }
  }
}
";

const GRAPHQL_ENDPOINT: &str = "https://api.github.com/graphql";

pub fn map_graphql_error(errors: &[GraphQLErrorItem]) -> StorageError {
    for e in errors {
        let msg = e.message.to_lowercase();
        if msg.contains("expected") && msg.contains("head") {
            return StorageError::Conflict { remote_head: extract_sha(&e.message).unwrap_or_default() };
        }
        if msg.contains("not authorized") || msg.contains("must have push access") {
            return StorageError::AuthFailed;
        }
        if msg.contains("could not resolve") || msg.contains("not found") {
            return StorageError::NotFound(e.message.clone());
        }
    }
    StorageError::ValidationFailed(
        errors.first().map(|e| e.message.clone()).unwrap_or_else(|| "unknown error".into()),
    )
}

pub fn map_http_status(status: u16, retry_after: Option<u64>) -> StorageError {
    match status {
        401 | 403 => StorageError::AuthFailed,
        404 => StorageError::NotFound(String::new()),
        409 => StorageError::Conflict { remote_head: String::new() },
        422 => StorageError::ValidationFailed(String::new()),
        429 => StorageError::RateLimited { retry_after },
        500..=599 => StorageError::ServerError(status),
        _ => StorageError::ServerError(status),
    }
}

fn extract_sha(msg: &str) -> Option<String> {
    msg.split_whitespace()
        .find(|w| w.len() == 40 && w.chars().all(|c| c.is_ascii_hexdigit()))
        .map(String::from)
}

impl StorageBackend for GitHubBackend {
    fn backend_type(&self) -> &'static str { "github" }

    fn commit<'a>(
        &'a self,
        changes: &'a ChangeSet,
        message: &'a str,
        expected_head: Option<&'a str>,
    ) -> BoxFuture<'a, StorageResult<CommitOutcome>> {
        Box::pin(async move {
            // 1. Build manifest body from merged view (caller must pre-merge; here we
            //    assume changes-only manifest injection is handled by the dispatcher).
            //    For Phase 3a the dispatcher passes a pre-serialized manifest via... ah,
            //    that plumbing lives in Task 3.5 (commit side-effect handler). This
            //    method accepts what it's given — a caller-prepared serialized manifest
            //    is NOT passed via this trait; instead, `commit` is called with a
            //    ChangeSet that ALREADY contains an UpdateFile entry for "manifest.json"
            //    at the mount-relative root. See spec §4.2.
            //
            //    TL;DR: the dispatcher upserts ChangeType::UpdateFile for
            //    "/manifest.json" BEFORE calling commit.
            let file_changes = build_file_changes(changes, &self.content_prefix, None);

            let input = CreateCommitInput {
                branch: BranchRef {
                    repo_with_owner: self.repo_with_owner.clone(),
                    branch_name: self.branch.clone(),
                },
                message: CommitMessage { headline: message.to_string() },
                expected_head_oid: expected_head.map(String::from),
                file_changes,
            };

            let body = GraphQLRequest { query: MUTATION, variables: GraphQLVariables { input: &input } };
            let body_json = serde_json::to_string(&body)
                .map_err(|e| StorageError::BadRequest(e.to_string()))?;

            let resp = gloo_net::http::Request::post(GRAPHQL_ENDPOINT)
                .header("Authorization", &format!("bearer {}", self.token))
                .header("Content-Type", "application/json")
                .header("User-Agent", "websh/0.1")
                .body(body_json)
                .map_err(|e| StorageError::BadRequest(e.to_string()))?
                .send()
                .await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;

            let status = resp.status();
            if !(200..300).contains(&status) {
                let retry_after = resp.headers().get("Retry-After")
                    .and_then(|v| v.parse::<u64>().ok());
                return Err(map_http_status(status, retry_after));
            }

            let gql: GraphQLResponse = resp.json().await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;

            if !gql.errors.is_empty() {
                return Err(map_graphql_error(&gql.errors));
            }

            let new_head = gql.data
                .and_then(|d| d.create_commit_on_branch)
                .map(|c| c.commit.oid)
                .ok_or_else(|| StorageError::ValidationFailed("empty data".into()))?;

            // Post-commit manifest refresh handled by caller (see §5.2). We return a
            // placeholder manifest here; the caller re-fetches via fetch_manifest().
            // Committed paths = staged paths.
            let committed_paths: Vec<_> = changes.iter_staged().map(|(p, _)| p.clone()).collect();
            Ok(CommitOutcome {
                new_head,
                manifest: Manifest::default(),
                committed_paths,
            })
        })
    }

    fn fetch_manifest(&self) -> BoxFuture<'_, StorageResult<Manifest>> {
        Box::pin(async move {
            let resp = gloo_net::http::Request::get(&self.manifest_url)
                .send().await
                .map_err(|e| StorageError::NetworkError(e.to_string()))?;
            if !(200..300).contains(&resp.status()) {
                return Err(map_http_status(resp.status(), None));
            }
            resp.json::<Manifest>().await
                .map_err(|e| StorageError::ValidationFailed(e.to_string()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_401_maps_auth_failed() {
        assert_eq!(map_http_status(401, None), StorageError::AuthFailed);
        assert_eq!(map_http_status(403, None), StorageError::AuthFailed);
    }

    #[test]
    fn http_429_preserves_retry_after() {
        assert_eq!(
            map_http_status(429, Some(30)),
            StorageError::RateLimited { retry_after: Some(30) }
        );
    }

    #[test]
    fn graphql_error_conflict_detected() {
        let e = vec![GraphQLErrorItem {
            message: "expected head oid abc123def456abc123def456abc123def4567890 was not current".into(),
            err_type: None,
        }];
        let mapped = map_graphql_error(&e);
        assert!(matches!(mapped, StorageError::Conflict { .. }));
    }

    #[test]
    fn graphql_error_auth_detected() {
        let e = vec![GraphQLErrorItem {
            message: "must have push access".into(),
            err_type: None,
        }];
        assert_eq!(map_graphql_error(&e), StorageError::AuthFailed);
    }
}
```

> **Important invariants for callers** (documented in the dispatcher task, but restated here so no one wires the commit path wrong): before calling `commit()`, the dispatcher must have already upserted a staged `ChangeType::UpdateFile` for `/manifest.json` reflecting the post-merge state. Phase 3a does NOT inject the manifest inside `commit`; the dispatcher owns manifest assembly. This keeps `commit` a pure "batch-apply" primitive.

- [ ] **Step 2: Register module + export**

In `src/core/storage/github/mod.rs`:

```rust
pub(crate) mod graphql;
mod client;
pub use client::GitHubBackend;
```

In `src/core/storage/mod.rs`, re-export: `pub use github::GitHubBackend;`

- [ ] **Step 3: Run tests**

Run: `cargo test -p websh --lib core::storage::github::client`
Expected: 4 tests PASS.

- [ ] **Step 4: Wasm build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green (this is the first task that uses `gloo-net`; confirm it's a dep).

- [ ] **Step 5: Commit**

```bash
git add src/core/storage/github/
git commit -m "feat(github): GitHubBackend with createCommitOnBranch + error mapping"
```

### Task 2.4: Add `idb` dependency + DB schema module

**Files:**
- Modify: `Cargo.toml`
- Create: `src/core/storage/idb.rs`
- Modify: `src/core/storage/mod.rs`

- [ ] **Step 1: Add dep**

In `Cargo.toml` under `[dependencies]`:

```toml
idb = "0.6"
gloo-timers = { version = "0.3", features = ["futures"] }
```

`gloo-timers` is used for the debounce in Task 2.6; add now to batch.

- [ ] **Step 2: Create the DB module**

Create `src/core/storage/idb.rs`:

```rust
//! IndexedDB persistence for drafts and metadata. See spec §7.

use idb::{Database, Factory, ObjectStoreParams, TransactionMode};
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;

use crate::core::changes::ChangeSet;
use crate::core::storage::{StorageError, StorageResult};

const DB_NAME: &str = "websh-state";
const DB_VERSION: u32 = 1;
pub const STORE_DRAFTS: &str = "drafts";
pub const STORE_METADATA: &str = "metadata";

#[derive(Serialize, Deserialize)]
struct DraftRecord {
    mount_id: String,
    #[serde(flatten)]
    changes: ChangeSet,
}

#[derive(Serialize, Deserialize)]
struct MetadataRecord {
    key: String,
    value: String,
}

pub async fn open_db() -> StorageResult<Database> {
    let factory = Factory::new().map_err(idb_err)?;
    let mut req = factory.open(DB_NAME, Some(DB_VERSION)).map_err(idb_err)?;

    req.on_upgrade_needed(|event| {
        let db = event.database().expect("upgrade db");
        if !db.store_names().iter().any(|n| n == STORE_DRAFTS) {
            db.create_object_store(STORE_DRAFTS, ObjectStoreParams::new()
                .key_path(Some(idb::KeyPath::new_single("mount_id")))
            ).expect("create drafts store");
        }
        if !db.store_names().iter().any(|n| n == STORE_METADATA) {
            db.create_object_store(STORE_METADATA, ObjectStoreParams::new()
                .key_path(Some(idb::KeyPath::new_single("key")))
            ).expect("create metadata store");
        }
    });

    req.await.map_err(idb_err)
}

pub async fn save_draft(db: &Database, mount_id: &str, changes: &ChangeSet) -> StorageResult<()> {
    let tx = db.transaction(&[STORE_DRAFTS], TransactionMode::ReadWrite).map_err(idb_err)?;
    let store = tx.object_store(STORE_DRAFTS).map_err(idb_err)?;
    let record = DraftRecord { mount_id: mount_id.to_string(), changes: changes.clone() };
    let value = serde_wasm_bindgen::to_value(&record)
        .map_err(|e| StorageError::BadRequest(format!("serialize: {e}")))?;
    store.put(&value, None).map_err(idb_err)?.await.map_err(idb_err)?;
    tx.commit().map_err(idb_err)?.await.map_err(idb_err)?;
    Ok(())
}

pub async fn load_draft(db: &Database, mount_id: &str) -> StorageResult<Option<ChangeSet>> {
    let tx = db.transaction(&[STORE_DRAFTS], TransactionMode::ReadOnly).map_err(idb_err)?;
    let store = tx.object_store(STORE_DRAFTS).map_err(idb_err)?;
    let value: Option<JsValue> = store.get(JsValue::from_str(mount_id))
        .map_err(idb_err)?
        .await
        .map_err(idb_err)?;
    match value {
        None => Ok(None),
        Some(v) => {
            let record: DraftRecord = serde_wasm_bindgen::from_value(v)
                .map_err(|e| StorageError::BadRequest(format!("deserialize: {e}")))?;
            Ok(Some(record.changes))
        }
    }
}

pub async fn save_metadata(db: &Database, key: &str, value: &str) -> StorageResult<()> {
    let tx = db.transaction(&[STORE_METADATA], TransactionMode::ReadWrite).map_err(idb_err)?;
    let store = tx.object_store(STORE_METADATA).map_err(idb_err)?;
    let record = MetadataRecord { key: key.to_string(), value: value.to_string() };
    let js = serde_wasm_bindgen::to_value(&record)
        .map_err(|e| StorageError::BadRequest(format!("serialize: {e}")))?;
    store.put(&js, None).map_err(idb_err)?.await.map_err(idb_err)?;
    tx.commit().map_err(idb_err)?.await.map_err(idb_err)?;
    Ok(())
}

pub async fn load_metadata(db: &Database, key: &str) -> StorageResult<Option<String>> {
    let tx = db.transaction(&[STORE_METADATA], TransactionMode::ReadOnly).map_err(idb_err)?;
    let store = tx.object_store(STORE_METADATA).map_err(idb_err)?;
    let value: Option<JsValue> = store.get(JsValue::from_str(key))
        .map_err(idb_err)?
        .await
        .map_err(idb_err)?;
    match value {
        None => Ok(None),
        Some(v) => {
            let record: MetadataRecord = serde_wasm_bindgen::from_value(v)
                .map_err(|e| StorageError::BadRequest(format!("deserialize: {e}")))?;
            Ok(Some(record.value))
        }
    }
}

fn idb_err<E: std::fmt::Display>(e: E) -> StorageError {
    let s = e.to_string().to_lowercase();
    if s.contains("quotaexceeded") {
        StorageError::BadRequest("local draft storage full. discard or commit to free space".into())
    } else {
        StorageError::NetworkError(format!("idb: {e}"))
    }
}
```

> `serde-wasm-bindgen` must be a dependency. Add to `Cargo.toml` if missing:
> ```toml
> serde-wasm-bindgen = "0.6"
> ```

- [ ] **Step 3: Register + build**

In `src/core/storage/mod.rs`, add: `pub mod idb;`.

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green. If `idb`'s API in 0.6 differs from the sketch above, adjust method names — consult `https://docs.rs/idb/0.6/idb/`. The shape is intentional: open_db, save_draft, load_draft, save_metadata, load_metadata.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/core/storage/idb.rs src/core/storage/mod.rs
git commit -m "feat(idb): drafts + metadata stores via idb crate"
```

### Task 2.5: IDB wasm-only round-trip test

**Files:**
- Create: `tests/idb_roundtrip.rs`

- [ ] **Step 1: Write the wasm_bindgen_test**

Create `tests/idb_roundtrip.rs`:

```rust
//! WASM-only IDB round-trip. Run with:
//!   wasm-pack test --chrome --headless
//! or:
//!   cargo install wasm-bindgen-cli && wasm-bindgen-test-runner ...
//!
//! Gated behind #[cfg(target_arch = "wasm32")] so it's skipped in `cargo test`.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

use websh::core::changes::{ChangeSet, ChangeType};
use websh::core::storage::idb::{load_draft, open_db, save_draft};
use websh::models::{FileMetadata, VirtualPath};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn save_then_load_draft_preserves_content() {
    let db = open_db().await.expect("open db");
    let mut cs = ChangeSet::new();
    let p = VirtualPath::from_absolute("/rt.md").unwrap();
    cs.upsert(p.clone(), ChangeType::CreateFile {
        content: "roundtrip".into(),
        meta: FileMetadata::default(),
    });

    save_draft(&db, "test-mount", &cs).await.expect("save");
    let loaded = load_draft(&db, "test-mount").await.expect("load").expect("exists");

    let entry = loaded.get(&p).expect("entry present");
    match &entry.change {
        ChangeType::CreateFile { content, .. } => assert_eq!(content, "roundtrip"),
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: Ensure `wasm-bindgen-test` dev-dep**

In `Cargo.toml` under `[dev-dependencies]`:

```toml
wasm-bindgen-test = "0.3"
```

- [ ] **Step 3: Build — wasm (do not attempt to run)**

Run: `cargo build --target wasm32-unknown-unknown --tests`
Expected: green. Running the test requires a browser harness (manual per-release).

- [ ] **Step 4: Commit**

```bash
git add tests/idb_roundtrip.rs Cargo.toml Cargo.lock
git commit -m "test(idb): wasm_bindgen_test for draft round-trip"
```

### Task 2.6: Debounced persist helper

**Files:**
- Create: `src/core/storage/persist.rs`
- Modify: `src/core/storage/mod.rs`

- [ ] **Step 1: Write the helper**

Create `src/core/storage/persist.rs`:

```rust
//! Debounced IDB persistence for ChangeSet. Called from a Leptos Effect
//! in Phase 3. The scheduling logic lives here so it's testable and
//! the reactive layer stays thin.
//!
//! Spec §7.4. Debounce interval is 300ms.

use std::cell::RefCell;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;

use crate::core::changes::ChangeSet;

use super::idb;

pub const DEBOUNCE_MS: u32 = 300;

/// A debounce handle. Call `schedule(changes)` on every mutation; the inner
/// task waits `DEBOUNCE_MS` and persists the latest snapshot. Rapid successive
/// calls reset the timer.
pub struct DraftPersister {
    mount_id: String,
    pending: Rc<RefCell<Option<ChangeSet>>>,
    task_running: Rc<RefCell<bool>>,
}

impl DraftPersister {
    pub fn new(mount_id: impl Into<String>) -> Self {
        Self {
            mount_id: mount_id.into(),
            pending: Rc::new(RefCell::new(None)),
            task_running: Rc::new(RefCell::new(false)),
        }
    }

    pub fn schedule(&self, changes: ChangeSet) {
        *self.pending.borrow_mut() = Some(changes);

        if *self.task_running.borrow() {
            return;  // existing task will pick up the newer snapshot
        }
        *self.task_running.borrow_mut() = true;

        let pending = self.pending.clone();
        let running = self.task_running.clone();
        let mount_id = self.mount_id.clone();

        spawn_local(async move {
            TimeoutFuture::new(DEBOUNCE_MS).await;
            let snapshot = pending.borrow_mut().take();
            *running.borrow_mut() = false;

            if let Some(cs) = snapshot {
                match idb::open_db().await {
                    Ok(db) => {
                        if let Err(e) = idb::save_draft(&db, &mount_id, &cs).await {
                            web_sys::console::error_1(
                                &format!("draft persist failed: {e}").into()
                            );
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(
                            &format!("idb open failed: {e}").into()
                        );
                    }
                }
            }
        });
    }
}
```

- [ ] **Step 2: Register + build**

In `src/core/storage/mod.rs`: `pub mod persist;`

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add src/core/storage/persist.rs src/core/storage/mod.rs
git commit -m "feat(idb): DraftPersister with 300ms debounce"
```

### Task 2.7: Phase 2 exit check

- [ ] **Step 1: Full build + test**

Run: `cargo test && cargo build --target wasm32-unknown-unknown && trunk build --release`
Expected: all green. Ship size increased by `idb` + `gloo-timers`; compare before/after.

---

## Phase 3 — Reactive wiring

Extend `AppContext`, wire hydration, and widen `SideEffect`. No new commands yet (Phase 4); this phase makes sure the plumbing exists.

### Task 3.1: Extend `AppContext`

**Files:**
- Modify: `src/app.rs` (AppContext definition, its constructor)
- Modify: `src/components/terminal/terminal.rs` (only if it reads AppContext fields that change)

- [ ] **Step 1: Add the new fields**

Find the `pub struct AppContext { ... }` in `src/app.rs`. Add:

```rust
    // Phase 3
    pub changes: leptos::prelude::RwSignal<crate::core::changes::ChangeSet>,
    pub view_fs: leptos::prelude::Memo<std::rc::Rc<crate::core::filesystem::VirtualFs>>,
    pub backend: leptos::prelude::StoredValue<
        Option<std::sync::Arc<dyn crate::core::storage::StorageBackend>>
    >,
    pub remote_head: leptos::prelude::StoredValue<Option<String>>,
```

> We do NOT add `sync: SyncUiState` in 3a — that's 3b. Spec §3.5 lists it for the end-of-Phase-3 shape.

- [ ] **Step 2: Initialize them in the constructor**

Find where `AppContext` is constructed (grep for `AppContext {` in `src/app.rs`). Add:

```rust
    let changes = RwSignal::new(ChangeSet::new());
    let fs_signal = fs;  // existing RwSignal<VirtualFs>
    let view_fs = Memo::new(move |_| {
        Rc::new(fs_signal.with(|base| {
            changes.with(|cs| crate::core::merge::merge_view(base, cs))
        }))
    });
    let backend = StoredValue::new(None::<Arc<dyn StorageBackend>>);
    let remote_head = StoredValue::new(None::<String>);
```

Add the imports at top of `src/app.rs`:

```rust
use std::rc::Rc;
use std::sync::Arc;

use crate::core::changes::ChangeSet;
use crate::core::merge;
use crate::core::storage::StorageBackend;
use leptos::prelude::{Memo, StoredValue};
```

Return the extended struct.

- [ ] **Step 3: Confirm `AppContext` is still `Copy`**

All new fields are signal handles (`RwSignal`, `Memo`, `StoredValue`) — these are `Copy` in Leptos 0.8. The struct's `#[derive(Copy, Clone)]` should still compile.

Run: `cargo check --target wasm32-unknown-unknown`
Expected: green. If `Arc<dyn StorageBackend>` breaks the `Copy` derive (it shouldn't — it's wrapped in StoredValue which is Copy), wrap the field access site rather than removing Copy.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): extend AppContext with changes, view_fs, backend, remote_head"
```

### Task 3.2: Initial backend + hydration effect

**Files:**
- Modify: `src/app.rs` (the boot path that sets up mounts and fetches initial manifest)
- New helper: `src/core/storage/boot.rs` (extract boot logic for testability)

- [ ] **Step 1: Write the boot helper**

Create `src/core/storage/boot.rs`:

```rust
//! One-shot boot helpers: construct the writable mount's backend, load
//! any persisted draft ChangeSet, and seed remote_head.

use std::sync::Arc;

use crate::core::changes::ChangeSet;
use crate::core::storage::{GitHubBackend, StorageBackend, StorageResult};
use crate::models::Mount;

use super::idb;

pub fn build_backend_for_mount(mount: &Mount, token: Option<&str>) -> Option<Arc<dyn StorageBackend>> {
    if !mount.is_writable() {
        return None;
    }
    let token = token?;
    match mount {
        Mount::GitHub { base_url, content_prefix, .. } => {
            let repo = parse_repo_from_base_url(base_url)?;
            let branch = parse_branch_from_base_url(base_url).unwrap_or_else(|| "main".to_string());
            let prefix = content_prefix.clone().unwrap_or_default();
            let manifest_url = format!("{}/manifest.json", base_url);
            Some(Arc::new(GitHubBackend::new(repo, branch, prefix, manifest_url, token)))
        }
        _ => None,
    }
}

/// Parse "owner/repo" from `https://raw.githubusercontent.com/owner/repo/branch/...`
fn parse_repo_from_base_url(url: &str) -> Option<String> {
    let tail = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut parts = tail.splitn(3, '/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some(format!("{owner}/{repo}"))
}

fn parse_branch_from_base_url(url: &str) -> Option<String> {
    let tail = url.strip_prefix("https://raw.githubusercontent.com/")?;
    let mut parts = tail.splitn(4, '/');
    let _owner = parts.next()?;
    let _repo = parts.next()?;
    let branch = parts.next()?;
    Some(branch.to_string())
}

pub async fn hydrate_drafts(mount_id: &str) -> StorageResult<ChangeSet> {
    let db = idb::open_db().await?;
    Ok(idb::load_draft(&db, mount_id).await?.unwrap_or_default())
}

pub async fn hydrate_remote_head(mount_id: &str) -> StorageResult<Option<String>> {
    let db = idb::open_db().await?;
    let key = format!("remote_head.{mount_id}");
    idb::load_metadata(&db, &key).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_repo_from_raw_url() {
        assert_eq!(
            parse_repo_from_base_url("https://raw.githubusercontent.com/0xwonj/db/main/content"),
            Some("0xwonj/db".to_string())
        );
    }

    #[test]
    fn parse_branch_from_raw_url() {
        assert_eq!(
            parse_branch_from_base_url("https://raw.githubusercontent.com/0xwonj/db/main/content"),
            Some("main".to_string())
        );
    }

    #[test]
    fn build_backend_refuses_readonly() {
        let mount = Mount::github_with_prefix(
            "ro", "https://raw.githubusercontent.com/x/y/main", "~",
        );
        assert!(build_backend_for_mount(&mount, Some("t")).is_none());
    }

    #[test]
    fn build_backend_refuses_missing_token() {
        let mount = Mount::github_writable(
            "~", "https://raw.githubusercontent.com/0xwonj/db/main", "~",
        );
        assert!(build_backend_for_mount(&mount, None).is_none());
    }
}
```

- [ ] **Step 2: Register**

In `src/core/storage/mod.rs`: `pub mod boot;`

- [ ] **Step 3: Wire into boot**

In `src/app.rs`, after `AppContext` construction and after initial manifest load, add:

```rust
use wasm_bindgen_futures::spawn_local;

// read token from sessionStorage via a utility — see Task 4.6 for the util
let token = crate::utils::session::get_gh_token();

// Home mount is writable (set in 0.1/1.1)
let home = crate::config::mounts().home();
let initial_backend = crate::core::storage::boot::build_backend_for_mount(home, token.as_deref());
ctx.backend.set_value(initial_backend);

// Hydrate drafts asynchronously
let mount_id = home.alias().to_string();
let changes_signal = ctx.changes;
let head_store = ctx.remote_head;
spawn_local(async move {
    match crate::core::storage::boot::hydrate_drafts(&mount_id).await {
        Ok(cs) if !cs.is_empty() => changes_signal.set(cs),
        Ok(_) => {}
        Err(e) => web_sys::console::error_1(&format!("hydrate drafts: {e}").into()),
    }
    match crate::core::storage::boot::hydrate_remote_head(&mount_id).await {
        Ok(h) => head_store.set_value(h),
        Err(e) => web_sys::console::error_1(&format!("hydrate head: {e}").into()),
    }
});
```

> `crate::utils::session::get_gh_token` is referenced; it's introduced in Task 4.6. For this task, create a stub at `src/utils/session.rs` that returns `None` unconditionally:
> ```rust
> //! Session-scoped storage helpers. See spec §8.3.
> pub fn get_gh_token() -> Option<String> { None }
> pub fn set_gh_token(_t: &str) {}
> pub fn clear_gh_token() {}
> ```
> Task 4.6 replaces the bodies.

- [ ] **Step 4: Register utils/session**

In `src/utils/mod.rs`, add: `pub mod session;` if not already present.

- [ ] **Step 5: Build + test**

Run: `cargo test && cargo build --target wasm32-unknown-unknown`
Expected: green. (4 boot tests pass; session stub compiles.)

- [ ] **Step 6: Commit**

```bash
git add src/core/storage/boot.rs src/core/storage/mod.rs src/app.rs src/utils/session.rs src/utils/mod.rs
git commit -m "feat(app): hydrate drafts + backend at boot"
```

### Task 3.3: Debounced persist effect

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: Register the persist effect**

In `src/app.rs`, after the hydrate block, add:

```rust
use crate::core::storage::persist::DraftPersister;

let persister = std::rc::Rc::new(DraftPersister::new(home.alias()));
let persister_for_effect = persister.clone();
leptos::prelude::Effect::new(move |_| {
    let snapshot = ctx.changes.get();
    persister_for_effect.schedule(snapshot);
});
// DraftPersister is kept alive by capturing it in the effect closure via Rc.
```

Note: `Effect::new` closure runs once synchronously, so the initial empty ChangeSet will trigger one noop persist. Acceptable.

- [ ] **Step 2: Build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green.

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): debounced IDB persist effect for changes"
```

### Task 3.4: Extend `SideEffect` enum

**Files:**
- Modify: `src/core/commands/result.rs`

- [ ] **Step 1: Add the new variants**

Find the `pub enum SideEffect { ... }` in `src/core/commands/result.rs`. Add:

```rust
    // Phase 3
    ApplyChange    { path: crate::models::VirtualPath, change: crate::core::changes::ChangeType },
    StageChange    { path: crate::models::VirtualPath },
    UnstageChange  { path: crate::models::VirtualPath },
    DiscardChange  { path: crate::models::VirtualPath },
    StageAll,
    UnstageAll,
    Commit         { message: String, expected_head: Option<String> },
    RefreshManifest,
    SetAuthToken   { token: String },
    ClearAuthToken,
    OpenEditor     { path: crate::models::VirtualPath },
```

- [ ] **Step 2: Build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: build passes; the match in `dispatch_side_effect` may warn about non-exhaustive match — that's fixed in 3.5.

- [ ] **Step 3: Commit**

```bash
git add src/core/commands/result.rs
git commit -m "feat(sideeffect): Phase 3 write variants (Apply/Stage/Commit/...)"
```

### Task 3.5: Extend `dispatch_side_effect`

**Files:**
- Modify: `src/components/terminal/terminal.rs` (around lines 174–185 per the summary; locate via grep for `dispatch_side_effect`)

- [ ] **Step 1: Add arms for sync variants**

In `dispatch_side_effect`, after the existing arms, add:

```rust
    SideEffect::ApplyChange { path, change } => {
        ctx.changes.update(|cs| cs.upsert(path, change));
    }
    SideEffect::StageChange { path } => {
        ctx.changes.update(|cs| cs.stage(&path));
    }
    SideEffect::UnstageChange { path } => {
        ctx.changes.update(|cs| cs.unstage(&path));
    }
    SideEffect::DiscardChange { path } => {
        ctx.changes.update(|cs| cs.discard(&path));
    }
    SideEffect::StageAll => {
        ctx.changes.update(|cs| cs.stage_all());
    }
    SideEffect::UnstageAll => {
        ctx.changes.update(|cs| cs.unstage_all());
    }
    SideEffect::SetAuthToken { token } => {
        crate::utils::session::set_gh_token(&token);
        // Rebuild backend with the new token.
        let home = crate::config::mounts().home();
        let backend = crate::core::storage::boot::build_backend_for_mount(home, Some(&token));
        ctx.backend.set_value(backend);
    }
    SideEffect::ClearAuthToken => {
        crate::utils::session::clear_gh_token();
        ctx.backend.set_value(None);
    }
    SideEffect::OpenEditor { path } => {
        // Phase 5 wires this to the EditModal; 3a uses an ad-hoc trigger.
        // For now, signal the open via a dedicated RwSignal on AppContext if needed.
        // Placeholder: emit a terminal line noting the open.
        ctx.terminal.output.update(|rb| {
            rb.push(crate::models::OutputLine::info(format!("edit: opening {path}", path = path)));
        });
    }
```

- [ ] **Step 2: Add async arms for Commit + RefreshManifest**

Continuing the match:

```rust
    SideEffect::Commit { message, expected_head } => {
        let backend = ctx.backend.get_value();
        let Some(backend) = backend else {
            ctx.terminal.output.update(|rb| rb.push(
                crate::models::OutputLine::error("sync: no backend (not authenticated?)".into())
            ));
            return;
        };
        let changes_signal = ctx.changes;
        let fs_signal = ctx.fs;
        let head_store = ctx.remote_head;
        let output = ctx.terminal.output;
        let home = crate::config::mounts().home();
        let mount_id = home.alias().to_string();

        wasm_bindgen_futures::spawn_local(async move {
            // Snapshot staged entries.
            let staged_snapshot = changes_signal.with_untracked(|cs| cs.clone());

            // Inject post-merge manifest.json as a staged UpdateFile.
            let merged = fs_signal.with_untracked(|base| {
                crate::core::merge::merge_view(base, &staged_snapshot)
            });
            let new_manifest = merged.serialize_manifest();
            let manifest_body = serde_json::to_string_pretty(&new_manifest)
                .unwrap_or_default();

            let mut snapshot_with_manifest = staged_snapshot.clone();
            let manifest_path = crate::models::VirtualPath::from_absolute("/manifest.json")
                .expect("valid path");
            snapshot_with_manifest.upsert(
                manifest_path.clone(),
                crate::core::changes::ChangeType::UpdateFile {
                    content: manifest_body,
                    description: None,
                },
            );

            match backend.commit(&snapshot_with_manifest, &message, expected_head.as_deref()).await {
                Ok(outcome) => {
                    // Re-fetch manifest for authoritative view.
                    match backend.fetch_manifest().await {
                        Ok(manifest) => fs_signal.set(
                            crate::core::filesystem::VirtualFs::from_manifest(manifest)
                        ),
                        Err(e) => output.update(|rb| rb.push(
                            crate::models::OutputLine::warn(format!("sync: commit ok, refresh failed: {e}"))
                        )),
                    }

                    // Clear committed entries.
                    let committed = outcome.committed_paths.clone();
                    changes_signal.update(|cs| {
                        for p in committed.iter() {
                            cs.discard(p);
                        }
                        cs.discard(&manifest_path); // if user had one queued
                    });

                    head_store.set_value(Some(outcome.new_head.clone()));
                    // Persist remote_head to IDB metadata
                    let head_val = outcome.new_head.clone();
                    let mid = mount_id.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(db) = crate::core::storage::idb::open_db().await {
                            let _ = crate::core::storage::idb::save_metadata(
                                &db, &format!("remote_head.{mid}"), &head_val,
                            ).await;
                        }
                    });

                    output.update(|rb| rb.push(
                        crate::models::OutputLine::info(format!(
                            "sync: committed {} files (HEAD now {}).",
                            outcome.committed_paths.len(),
                            &outcome.new_head[..outcome.new_head.len().min(8)]
                        ))
                    ));
                }
                Err(e) => {
                    output.update(|rb| rb.push(
                        crate::models::OutputLine::error(format!("sync: {e}"))
                    ));
                }
            }
        });
    }
    SideEffect::RefreshManifest => {
        let backend = ctx.backend.get_value();
        let Some(backend) = backend else {
            ctx.terminal.output.update(|rb| rb.push(
                crate::models::OutputLine::error("sync refresh: no backend".into())
            ));
            return;
        };
        let fs_signal = ctx.fs;
        let output = ctx.terminal.output;
        wasm_bindgen_futures::spawn_local(async move {
            match backend.fetch_manifest().await {
                Ok(manifest) => {
                    fs_signal.set(crate::core::filesystem::VirtualFs::from_manifest(manifest));
                    output.update(|rb| rb.push(
                        crate::models::OutputLine::info("sync: manifest refreshed.".into())
                    ));
                }
                Err(e) => output.update(|rb| rb.push(
                    crate::models::OutputLine::error(format!("sync refresh: {e}"))
                )),
            }
        });
    }
```

> `OutputLine::info / warn / error` are assumed constructor shortcuts. If they don't exist, construct `OutputLine` directly per the existing pattern in that file — grep for `OutputLine::` to find the actual constructors.

- [ ] **Step 2: Build + test**

Run: `cargo test && cargo build --target wasm32-unknown-unknown`
Expected: green. The match must be exhaustive now.

- [ ] **Step 3: Commit**

```bash
git add src/components/terminal/terminal.rs
git commit -m "feat(dispatch): handlers for ApplyChange/Commit/RefreshManifest/..."
```

### Task 3.6: Phase 3 exit check

- [ ] **Step 1: Full build + test**

Run: `cargo test && cargo build --target wasm32-unknown-unknown && trunk build --release`
Expected: green. Load the page; check that boot doesn't panic and drafts still pull from IDB.

---

## Phase 4 — Commands

Adds the user-facing surface: `touch`, `mkdir`, `rm`, `rmdir`, `edit`, `sync <sub>`. Also adds `echo 'body' > path` redirection to the parser.

### Task 4.1: `Command` enum variants

**Files:**
- Modify: `src/core/commands/mod.rs`

- [ ] **Step 1: Add variants and the `SyncSubcommand` enum**

In `src/core/commands/mod.rs`, find the `pub enum Command { ... }` and append:

```rust
    Touch { path: PathArg },
    Mkdir { path: PathArg },
    Rm    { path: PathArg, recursive: bool },
    Rmdir { path: PathArg },
    Edit  { path: PathArg },
    Sync  (SyncSubcommand),
```

Add (in the same file, below `Command`):

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncSubcommand {
    Status,
    Commit { message: String },
    Refresh,
    Auth(AuthAction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthAction {
    Set { token: String },
    Clear,
}
```

> Phase 3a does not ship `Add`, `Reset`, `Discard` — those are 3b. Keep the enum minimal.

- [ ] **Step 2: Extend `Command::names()`**

Find `Command::names()` and add the new names (including `sync`):

```rust
    "touch", "mkdir", "rm", "rmdir", "edit", "sync",
```

Maintain the existing alphabetical / grouping convention of that array.

- [ ] **Step 3: Build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: build fails on non-exhaustive `match` in execute — that's fixed in 4.4.

- [ ] **Step 4: Commit**

```bash
git add src/core/commands/mod.rs
git commit -m "feat(cmd): Touch/Mkdir/Rm/Rmdir/Edit/Sync variants"
```

### Task 4.2: Parser — new command forms

**Files:**
- Modify: `src/core/commands/mod.rs` (Command::parse)

- [ ] **Step 1: Write the failing tests**

In `src/core/commands/mod.rs:mod tests` (or wherever `parse` tests live; grep `fn parse` in same file):

```rust
#[test]
fn parse_touch() {
    let cmd = Command::parse("touch /tmp/a.md").unwrap();
    assert!(matches!(cmd, Command::Touch { .. }));
}

#[test]
fn parse_rm_recursive() {
    let cmd = Command::parse("rm -r /tmp/dir").unwrap();
    match cmd {
        Command::Rm { recursive, .. } => assert!(recursive),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_rm_non_recursive() {
    let cmd = Command::parse("rm /tmp/a.md").unwrap();
    match cmd {
        Command::Rm { recursive, .. } => assert!(!recursive),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_sync_status() {
    let cmd = Command::parse("sync status").unwrap();
    assert!(matches!(cmd, Command::Sync(SyncSubcommand::Status)));
}

#[test]
fn parse_sync_commit_with_message() {
    let cmd = Command::parse("sync commit -m \"hello world\"").unwrap();
    match cmd {
        Command::Sync(SyncSubcommand::Commit { message }) => {
            assert_eq!(message, "hello world");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_sync_auth_set() {
    let cmd = Command::parse("sync auth gh_pat_abc123").unwrap();
    match cmd {
        Command::Sync(SyncSubcommand::Auth(AuthAction::Set { token })) => {
            assert_eq!(token, "gh_pat_abc123");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_sync_auth_clear() {
    let cmd = Command::parse("sync auth clear").unwrap();
    assert!(matches!(cmd, Command::Sync(SyncSubcommand::Auth(AuthAction::Clear))));
}

#[test]
fn parse_edit() {
    let cmd = Command::parse("edit /tmp/a.md").unwrap();
    assert!(matches!(cmd, Command::Edit { .. }));
}
```

Run: `cargo test -p websh --lib core::commands` — expect FAIL on each new case.

- [ ] **Step 2: Extend `Command::parse()`**

Locate the match on the command name token. Add arms. Use the same PathArg pattern as existing single-path commands (Cat, Cd, etc.). For `rm`, check for a leading `-r` or `-rf` flag on the token array. For `sync`, dispatch on the second token:

```rust
    "touch" => {
        let path = tokens.next_path_arg().ok_or(ParseError::MissingOperand("touch"))?;
        Ok(Command::Touch { path })
    }
    "mkdir" => {
        let path = tokens.next_path_arg().ok_or(ParseError::MissingOperand("mkdir"))?;
        Ok(Command::Mkdir { path })
    }
    "rmdir" => {
        let path = tokens.next_path_arg().ok_or(ParseError::MissingOperand("rmdir"))?;
        Ok(Command::Rmdir { path })
    }
    "rm" => {
        let mut recursive = false;
        let mut path: Option<PathArg> = None;
        while let Some(tok) = tokens.next() {
            if tok == "-r" || tok == "-rf" || tok == "-R" {
                recursive = true;
            } else {
                path = Some(PathArg::from(tok));
                break;
            }
        }
        let path = path.ok_or(ParseError::MissingOperand("rm"))?;
        Ok(Command::Rm { path, recursive })
    }
    "edit" => {
        let path = tokens.next_path_arg().ok_or(ParseError::MissingOperand("edit"))?;
        Ok(Command::Edit { path })
    }
    "sync" => parse_sync(tokens),
```

Add a `parse_sync(tokens) -> Result<Command, ParseError>` free function or inline helper:

```rust
fn parse_sync(mut tokens: Tokens<'_>) -> Result<Command, ParseError> {
    let sub = tokens.next().ok_or(ParseError::MissingOperand("sync"))?;
    match sub {
        "status" => Ok(Command::Sync(SyncSubcommand::Status)),
        "refresh" => Ok(Command::Sync(SyncSubcommand::Refresh)),
        "commit" => {
            // expect -m <msg>
            let flag = tokens.next().ok_or(ParseError::MissingOperand("sync commit -m"))?;
            if flag != "-m" {
                return Err(ParseError::UnexpectedToken(flag.to_string()));
            }
            let msg = tokens.rest_as_quoted_string()
                .ok_or(ParseError::MissingOperand("sync commit -m"))?;
            Ok(Command::Sync(SyncSubcommand::Commit { message: msg }))
        }
        "auth" => {
            let arg = tokens.next().ok_or(ParseError::MissingOperand("sync auth"))?;
            if arg == "clear" {
                Ok(Command::Sync(SyncSubcommand::Auth(AuthAction::Clear)))
            } else {
                Ok(Command::Sync(SyncSubcommand::Auth(AuthAction::Set {
                    token: arg.to_string(),
                })))
            }
        }
        other => Err(ParseError::UnknownSubcommand(other.to_string())),
    }
}
```

> `Tokens::next_path_arg`, `rest_as_quoted_string`, `ParseError::*` — use whichever API exists in the current parser. Grep `enum ParseError` + `struct Tokens` (or similar) in `src/core/parser.rs` / `src/core/commands/mod.rs`. Don't invent; adapt. The quoted-string helper may already exist for `echo` / `grep`; if not, add one — it scans the remaining tokens, requires either a single `"..."`-quoted string or a bare word, and returns the string content.

- [ ] **Step 3: Run tests — green**

Run: `cargo test -p websh --lib core::commands`
Expected: all parse tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/core/commands/mod.rs
git commit -m "feat(parser): touch/mkdir/rm/rmdir/edit/sync parsing"
```

### Task 4.3: Parser — `echo 'body' > path` redirection

**Files:**
- Modify: `src/core/parser.rs` (pipe / top-level token splitter) OR `src/core/commands/mod.rs` (`echo` arm), depending on where redirection fits.

- [ ] **Step 1: Decide approach**

Read `src/core/parser.rs` for the pipe/command split. Redirection is a **top-level** concern (like pipe `|`), not an `echo`-only feature, but in 3a only `echo > path` is supported. Simplest path: handle `>` inside the `echo` arm of `Command::parse` — reject if encountered in any other command arm.

- [ ] **Step 2: Write failing tests — including the quote-trap**

In `src/core/commands/mod.rs:mod tests`:

```rust
#[test]
fn parse_echo_redirect_creates_command() {
    let cmd = Command::parse("echo hello > /tmp/a.md").unwrap();
    // Representation: we reuse Command::Echo but add an Option<PathArg> target,
    // OR we introduce Command::EchoRedirect. For 3a choose the latter for clarity.
    match cmd {
        Command::EchoRedirect { body, path } => {
            assert_eq!(body, "hello");
            assert_eq!(path.as_str(), "/tmp/a.md");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_echo_redirect_quoted_body() {
    let cmd = Command::parse("echo \"hello world\" > /tmp/a.md").unwrap();
    match cmd {
        Command::EchoRedirect { body, .. } => assert_eq!(body, "hello world"),
        _ => panic!("wrong variant"),
    }
}

// CRITICAL quote-trap: a `>` inside a quoted body must NOT be treated as the
// redirect operator. A naive `rsplit_once('>')` would split `"a>b" > /tmp/x`
// at the WRONG `>`, corrupting the file contents.
#[test]
fn parse_echo_redirect_ignores_gt_inside_quotes() {
    let cmd = Command::parse("echo \"a > b\" > /tmp/x.md").unwrap();
    match cmd {
        Command::EchoRedirect { body, path } => {
            assert_eq!(body, "a > b");
            assert_eq!(path.as_str(), "/tmp/x.md");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_echo_redirect_ignores_gt_inside_single_quotes() {
    let cmd = Command::parse("echo 'x>y' > /tmp/x.md").unwrap();
    match cmd {
        Command::EchoRedirect { body, .. } => assert_eq!(body, "x>y"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn parse_echo_multiword_without_redirect_is_plain_echo() {
    let cmd = Command::parse("echo hello world").unwrap();
    assert!(matches!(cmd, Command::Echo { .. }));
}
```

- [ ] **Step 3: Add the variant**

In `Command`:

```rust
    EchoRedirect { body: String, path: PathArg },
```

- [ ] **Step 4: Parse — quote-aware**

In the `echo` arm, find the redirect `>` while respecting quotes. **Do NOT use `rsplit_once('>')`** — it splits inside quoted bodies and corrupts content. Scan left-to-right, tracking quote state, and split at the first unquoted `>`:

```rust
    "echo" => {
        let raw_tail = tokens.rest_raw();  // original substring after "echo "

        // Scan for an unquoted top-level `>`. Returns byte index of the `>`
        // or None if all `>` are inside quotes (or there are none).
        fn find_unquoted_redirect(s: &str) -> Option<usize> {
            let mut in_single = false;
            let mut in_double = false;
            let mut escaped = false;
            for (i, ch) in s.char_indices() {
                if escaped { escaped = false; continue; }
                match ch {
                    '\\' if in_double => escaped = true,
                    '\'' if !in_double => in_single = !in_single,
                    '"'  if !in_single => in_double = !in_double,
                    '>'  if !in_single && !in_double => return Some(i),
                    _ => {}
                }
            }
            None
        }

        if let Some(gt) = find_unquoted_redirect(raw_tail) {
            let body_part = raw_tail[..gt].trim();
            let path_part = raw_tail[gt + 1..].trim();
            if path_part.is_empty() {
                return Err(ParseError::MissingOperand("echo > "));
            }
            // Reject a second unquoted `>` (we only support single redirect in 3a).
            if find_unquoted_redirect(path_part).is_some() {
                return Err(ParseError::UnsupportedSyntax("multiple '>' not supported"));
            }
            let body = unquote(body_part).to_string();
            Ok(Command::EchoRedirect {
                body,
                path: PathArg::from(path_part),
            })
        } else {
            // existing echo path
            Ok(Command::Echo { ... })
        }
    }
```

Where `unquote(s)` strips a single pair of surrounding single or double quotes, passes through otherwise. If `unquote` doesn't exist, implement as a 10-line helper. For `"..."` bodies, also interpret `\"` and `\\` escapes consistent with `find_unquoted_redirect`'s escape handling — otherwise round-tripping is inconsistent.

> If the existing tokenizer already strips quotes and splits `>` as its own token, prefer that: iterate tokens, collect body tokens until an unquoted `>` token, then the next token is the path. Use this handwritten scan only if the existing tokenizer doesn't preserve quote boundaries (many shell-lite parsers don't).

- [ ] **Step 5: Run tests**

Run: `cargo test -p websh --lib core::commands`
Expected: 3 new tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/mod.rs
git commit -m "feat(parser): echo 'body' > path redirection"
```

### Task 4.4: Execute — write arms

**Files:**
- Modify: `src/core/commands/execute.rs`

- [ ] **Step 1: Touch / Mkdir / Rm / Rmdir**

Inside `execute_command`, add arms. These must be PURE — they read context (wallet, view_fs, mount) and emit `CommandResult { side_effect: Some(SideEffect::ApplyChange {...}) }`. No direct signal mutation.

```rust
    Command::Touch { path } => {
        let abs = resolve_path(&ctx, &path);
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("touch: permission denied", 1);
        }
        if ctx.view_fs.get_entry(&abs).is_some() {
            return CommandResult::err(
                &format!("touch: path already exists: {abs}"), 1,
            );
        }
        CommandResult::with_side_effect(SideEffect::ApplyChange {
            path: abs,
            change: ChangeType::CreateFile {
                content: String::new(),
                meta: FileMetadata::default(),
            },
        })
    }
    Command::Mkdir { path } => {
        let abs = resolve_path(&ctx, &path);
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("mkdir: permission denied", 1);
        }
        if ctx.view_fs.get_entry(&abs).is_some() {
            return CommandResult::err(
                &format!("mkdir: path already exists: {abs}"), 1,
            );
        }
        CommandResult::with_side_effect(SideEffect::ApplyChange {
            path: abs,
            change: ChangeType::CreateDirectory {
                meta: DirectoryMetadata::default(),
            },
        })
    }
    Command::Rm { path, recursive } => {
        let abs = resolve_path(&ctx, &path);
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("rm: permission denied", 1);
        }
        let entry = match ctx.view_fs.get_entry(&abs) {
            Some(e) => e,
            None => return CommandResult::err(
                &format!("rm: no such path: {abs}"), 1,
            ),
        };
        if entry.is_directory() && !recursive {
            return CommandResult::err(
                &format!("rm: {abs}: is a directory (use -r)"), 1,
            );
        }
        let change = if entry.is_directory() {
            ChangeType::DeleteDirectory
        } else {
            ChangeType::DeleteFile
        };
        CommandResult::with_side_effect(SideEffect::ApplyChange { path: abs, change })
    }
    Command::Rmdir { path } => {
        let abs = resolve_path(&ctx, &path);
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("rmdir: permission denied", 1);
        }
        let entry = match ctx.view_fs.get_entry(&abs) {
            Some(e) => e,
            None => return CommandResult::err(
                &format!("rmdir: no such directory: {abs}"), 1,
            ),
        };
        if !entry.is_directory() {
            return CommandResult::err(
                &format!("rmdir: not a directory: {abs}"), 1,
            );
        }
        // Non-empty check — iterate view_fs for any child prefix
        if ctx.view_fs.has_children(&abs) {
            return CommandResult::err(
                &format!("rmdir: directory not empty: {abs}"), 1,
            );
        }
        CommandResult::with_side_effect(SideEffect::ApplyChange {
            path: abs,
            change: ChangeType::DeleteDirectory,
        })
    }
```

> `CommandResult::err` and `CommandResult::with_side_effect` are assumed constructors. If they don't exist, build a `CommandResult { output: vec![OutputLine::error(...)], exit_code, side_effect: None }` directly (matching the Phase 1 shape).
>
> `current_mount(&ctx)` — helper that returns the mount for the current working directory. Adapt to existing ctx accessors; grep `fn current_mount` or infer from how `execute_command` already does path resolution today.
>
> `ctx.view_fs.has_children(&path)` — add this helper on `VirtualFs` if missing: iterate keys, return true if any starts with `path.as_str() + "/"`.

- [ ] **Step 2: Edit**

```rust
    Command::Edit { path } => {
        let abs = resolve_path(&ctx, &path);
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("edit: permission denied", 1);
        }
        if !matches!(ctx.view_fs.get_entry(&abs), Some(FsEntry::File { .. })) {
            return CommandResult::err(
                &format!("edit: not a regular file: {abs}"), 1,
            );
        }
        CommandResult::with_side_effect(SideEffect::OpenEditor { path: abs })
    }
```

- [ ] **Step 3: EchoRedirect**

```rust
    Command::EchoRedirect { body, path } => {
        let abs = resolve_path(&ctx, &path);
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("echo: permission denied", 1);
        }
        let change = match ctx.view_fs.get_entry(&abs) {
            Some(FsEntry::Directory { .. }) => {
                return CommandResult::err(
                    &format!("echo: {abs} is a directory"), 1,
                );
            }
            Some(FsEntry::File { .. }) => ChangeType::UpdateFile {
                content: body.clone(),
                description: None,
            },
            None => ChangeType::CreateFile {
                content: body.clone(),
                meta: FileMetadata::default(),
            },
        };
        CommandResult::with_side_effect(SideEffect::ApplyChange { path: abs, change })
    }
```

- [ ] **Step 4: Build + run all parse & execute tests**

Run: `cargo test -p websh --lib core::commands`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src/core/commands/execute.rs src/core/filesystem.rs
git commit -m "feat(exec): write-command arms (touch/mkdir/rm/rmdir/edit/echo>)"
```

### Task 4.5: Execute — `sync` subcommands

**Files:**
- Modify: `src/core/commands/execute.rs`

- [ ] **Step 1: Status**

```rust
    Command::Sync(SyncSubcommand::Status) => {
        let cs = ctx.changes_snapshot();  // helper: read-only snapshot
        if cs.is_empty() {
            return CommandResult::ok_info("nothing to commit (working tree clean)");
        }
        let summary = cs.summary();
        let mut lines: Vec<OutputLine> = vec![];
        lines.push(OutputLine::info(format!(
            "{} changes ({} staged, {} unstaged).",
            summary.total(),
            summary.total_staged(),
            summary.total() - summary.total_staged(),
        )));
        if summary.total_staged() > 0 {
            lines.push(OutputLine::info("Staged:".into()));
            for (path, entry) in cs.iter_staged() {
                let tag = change_tag(&entry.change);
                lines.push(OutputLine::info(format!("  {tag}  {path}")));
            }
        }
        if summary.total() > summary.total_staged() {
            lines.push(OutputLine::info("Unstaged:".into()));
            for (path, entry) in cs.iter_unstaged() {
                let tag = change_tag(&entry.change);
                lines.push(OutputLine::info(format!("  {tag}  {path}")));
            }
        }
        CommandResult { output: lines, exit_code: 0, side_effect: None }
    }
```

Add `fn change_tag(c: &ChangeType) -> &'static str` helper: "A", "M", "D".

`ctx.changes_snapshot()` — add a method on whatever `ctx` type execute_command takes that returns `ChangeSet` (clone). If `execute_command` takes a pre-snapshotted context struct (it should, for test purity), add a `changes: ChangeSet` field to that struct, populated at dispatch time.

> If `execute_command` currently reads signals directly, that's a lurking Phase-1 contract violation. Either: (a) accept the clone for now and document as tech-debt, or (b) refactor the caller to pass a snapshot struct. For 3a, option (a) is acceptable — mark in code: `// TODO: snapshot at dispatch time`.

- [ ] **Step 2: Commit / Refresh / Auth**

```rust
    Command::Sync(SyncSubcommand::Commit { message }) => {
        let mount = current_mount(&ctx);
        if !crate::core::admin::can_write_to(&ctx.wallet_state, &mount) {
            return CommandResult::err("sync commit: permission denied", 1);
        }
        let cs = ctx.changes_snapshot();
        if cs.iter_staged().next().is_none() {
            return CommandResult::err("sync commit: nothing staged", 1);
        }
        let expected_head = ctx.remote_head_snapshot();
        CommandResult {
            output: vec![OutputLine::info("Committing...".into())],
            exit_code: 0,
            side_effect: Some(SideEffect::Commit { message, expected_head }),
        }
    }
    Command::Sync(SyncSubcommand::Refresh) => CommandResult {
        output: vec![OutputLine::info("Refreshing manifest...".into())],
        exit_code: 0,
        side_effect: Some(SideEffect::RefreshManifest),
    },
    Command::Sync(SyncSubcommand::Auth(AuthAction::Set { token })) => {
        // validate format: GitHub PATs start with ghp_ or github_pat_
        if !(token.starts_with("ghp_") || token.starts_with("github_pat_")) {
            return CommandResult::err("sync auth: token format not recognized (expected ghp_ / github_pat_)", 2);
        }
        CommandResult {
            output: vec![OutputLine::info("Storing token for session.".into())],
            exit_code: 0,
            side_effect: Some(SideEffect::SetAuthToken { token }),
        }
    }
    Command::Sync(SyncSubcommand::Auth(AuthAction::Clear)) => CommandResult {
        output: vec![OutputLine::info("Clearing token.".into())],
        exit_code: 0,
        side_effect: Some(SideEffect::ClearAuthToken),
    },
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p websh --lib core::commands`
Expected: green, and `sync status` / `sync commit` execute tests (you should add 2-3 smoke tests that assert the `side_effect` field).

- [ ] **Step 4: Commit**

```bash
git add src/core/commands/execute.rs
git commit -m "feat(exec): sync status/commit/refresh/auth arms"
```

### Task 4.6: Real `utils::session` token storage

**Files:**
- Modify: `src/utils/session.rs`

- [ ] **Step 1: Replace stubs with real sessionStorage access**

Replace the file content:

```rust
//! sessionStorage-scoped token. XSS exposure acknowledged (spec §8.3).

const KEY: &str = "websh.gh_token";

pub fn get_gh_token() -> Option<String> {
    let storage = web_sys::window()?.session_storage().ok()??;
    storage.get_item(KEY).ok()?
}

pub fn set_gh_token(token: &str) {
    if let Some(Ok(Some(s))) = web_sys::window().map(|w| w.session_storage()) {
        let _ = s.set_item(KEY, token);
    }
}

pub fn clear_gh_token() {
    if let Some(Ok(Some(s))) = web_sys::window().map(|w| w.session_storage()) {
        let _ = s.remove_item(KEY);
    }
}
```

> `web-sys` must already be a dep (it is — ECIES used it). If `session_storage` feature flag isn't enabled, add to `Cargo.toml`'s `web-sys` features: `"Storage"`, `"Window"`.

- [ ] **Step 2: Build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green. If feature errors, add them to the `web-sys` feature list in Cargo.toml.

- [ ] **Step 3: Commit**

```bash
git add src/utils/session.rs Cargo.toml
git commit -m "feat(session): sessionStorage-backed GitHub token"
```

### Task 4.7: Autocomplete — new command names + `sync` subcommand completion

**Files:**
- Modify: `src/core/autocomplete.rs`

- [ ] **Step 1: Classify commands**

Find `DIR_COMMANDS` / `FILE_COMMANDS` (const arrays). Add:
- `FILE_COMMANDS`: `touch`, `rm`, `edit` (operate on files)
- `DIR_COMMANDS`: `mkdir`, `rmdir` (operate on dirs)
- Neither (or both): `sync`

If `sync` should complete its subcommand args (not paths), handle separately — see Step 2.

- [ ] **Step 2: Custom completion for `sync <tab>`**

Find the top-level completer in `autocomplete.rs`. When the first token is exactly `sync`, return the subcommand list:

```rust
    if first_token == "sync" && tokens.len() <= 2 {
        let subs = ["status", "commit", "refresh", "auth"];
        return subs.iter().filter(|s| s.starts_with(prefix)).map(|s| s.to_string()).collect();
    }
```

If `sync auth <tab>` — completions: `clear`. (Only `clear` is a known subword; the token value is typed.)

```rust
    if tokens[0] == "sync" && tokens.get(1) == Some(&"auth") && tokens.len() <= 3 {
        return ["clear"].iter().filter(|s| s.starts_with(prefix)).map(|s| s.to_string()).collect();
    }
```

- [ ] **Step 3: Write tests**

Append to `src/core/autocomplete.rs:mod tests`:

```rust
#[test]
fn autocomplete_top_level_includes_sync() {
    let completions = complete_command("sy");
    assert!(completions.iter().any(|s| s == "sync"));
}

#[test]
fn autocomplete_sync_subcommands() {
    let completions = complete_inputs("sync c");
    // Adapt to the actual top-level completer function name
    assert!(completions.iter().any(|s| s == "commit"));
}
```

> `complete_command` / `complete_inputs` are placeholders for whatever the existing API is called — adapt.

- [ ] **Step 4: Run**

Run: `cargo test -p websh --lib core::autocomplete`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add src/core/autocomplete.rs
git commit -m "feat(autocomplete): sync + write commands"
```

### Task 4.8: Help text

**Files:**
- Modify: `src/config.rs` (help text constants) or wherever help content lives — grep `help` in config/commands.

- [ ] **Step 1: Add help lines for new commands**

In the help text, insert (alphabetical):

```
edit <path>                Open file in editor
mkdir <path>               Create directory
rm [-r] <path>             Remove file (or directory with -r)
rmdir <path>               Remove empty directory
touch <path>               Create empty file
sync status                Show staged/unstaged changes
sync commit -m <msg>       Commit staged changes to remote
sync refresh               Re-fetch manifest from remote
sync auth <token>          Store GitHub token for session
sync auth clear            Clear stored token
```

- [ ] **Step 2: Commit**

```bash
git add src/config.rs
git commit -m "docs(help): add write + sync command help text"
```

### Task 4.9: Terminal integration test — MockBackend end-to-end

**Files:**
- Create: `tests/commit_integration.rs`

- [ ] **Step 1: Write the integration test**

Create `tests/commit_integration.rs`:

```rust
//! End-to-end: `sync commit -m "msg"` with staged changes → MockBackend
//! records the call with the staged paths, new manifest includes the
//! injected `manifest.json`.
//!
//! This test does NOT exercise Leptos dispatch (that requires WASM); it
//! simulates the dispatcher's logic inline. Treat this as a seam test: it
//! verifies the commit-path logic in isolation from the reactive runtime.

use websh::core::changes::{ChangeSet, ChangeType};
use websh::core::filesystem::VirtualFs;
use websh::core::merge::merge_view;
use websh::core::storage::{MockBackend, StorageBackend};
use websh::models::{FileMetadata, Manifest, VirtualPath};

#[tokio::test(flavor = "current_thread")]
async fn commit_path_records_staged_paths_plus_manifest() {
    let base = VirtualFs::empty();
    let mut cs = ChangeSet::new();
    let p = VirtualPath::from_absolute("/a.md").unwrap();
    cs.upsert(p.clone(), ChangeType::CreateFile {
        content: "hello".into(),
        meta: FileMetadata::default(),
    });

    // Simulate dispatcher: inject manifest.json.
    let merged = merge_view(&base, &cs);
    let manifest_body = serde_json::to_string_pretty(&merged.serialize_manifest()).unwrap();
    let mut with_manifest = cs.clone();
    let mpath = VirtualPath::from_absolute("/manifest.json").unwrap();
    with_manifest.upsert(mpath.clone(), ChangeType::UpdateFile {
        content: manifest_body,
        description: None,
    });

    let backend = MockBackend::with_success(Manifest::default(), "sha-new");
    let outcome = backend.commit(&with_manifest, "test", Some("sha-old")).await.unwrap();
    assert_eq!(outcome.new_head, "sha-new");

    let calls = backend.commit_calls.borrow();
    assert_eq!(calls.len(), 1);
    let paths: Vec<&str> = calls[0].paths.iter().map(|p| p.as_str()).collect();
    assert!(paths.contains(&"/a.md"));
    assert!(paths.contains(&"/manifest.json"));
    assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
}
```

- [ ] **Step 2: Run**

Run: `cargo test --test commit_integration`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/commit_integration.rs
git commit -m "test(integration): commit path records staged + manifest"
```

### Task 4.10: Phase 4 exit check

- [ ] **Step 1: Full suite**

Run: `cargo test && cargo build --target wasm32-unknown-unknown && trunk build --release`
Expected: all green.

- [ ] **Step 2: Manual smoke (no commit yet — editor is Phase 5)**

In dev server (`trunk serve`), run in the terminal:
- `sync status` — should show "nothing to commit".
- `touch /tmp/foo.md` — admin gate blocks unless wallet + admin set.
- After connecting admin wallet + `sync auth <test-token>`: `touch /tmp/foo.md` then `sync status` — should list one staged create.

Not all of this is green yet (editor UI lands in Phase 5), but the command-path works end-to-end.

---

## Phase 5 — Minimal EditModal UI

`edit <path>` opens a textarea modal. Save dispatches `ApplyChange`; Cancel closes without mutation. No preview, no syntax highlighting, no keyboard shortcuts beyond Esc/Ctrl+Enter. The richer editor is Phase 3c.

### Task 5.1: Wire `OpenEditor` to an `editor_open: RwSignal<Option<VirtualPath>>`

**Files:**
- Modify: `src/app.rs`
- Modify: `src/components/terminal/terminal.rs` (the `OpenEditor` arm in dispatch)

- [ ] **Step 1: Add the signal to AppContext**

In `src/app.rs`, in `AppContext`:

```rust
    pub editor_open: leptos::prelude::RwSignal<Option<crate::models::VirtualPath>>,
```

And in its constructor:

```rust
    let editor_open = RwSignal::new(None);
```

- [ ] **Step 2: Wire the OpenEditor arm**

Replace the placeholder body of `SideEffect::OpenEditor` in `dispatch_side_effect`:

```rust
    SideEffect::OpenEditor { path } => {
        ctx.editor_open.set(Some(path));
    }
```

- [ ] **Step 3: Build**

Run: `cargo build --target wasm32-unknown-unknown`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add src/app.rs src/components/terminal/terminal.rs
git commit -m "feat(app): editor_open signal + wire OpenEditor side-effect"
```

### Task 5.2: `EditModal` component

**Files:**
- Create: `src/components/editor/mod.rs`
- Create: `src/components/editor/modal.rs`
- Create: `src/components/editor/modal.module.css`
- Modify: `src/components/mod.rs`
- Modify: `src/app.rs` (render `<EditModal />` at root)

- [ ] **Step 1: Create the component**

Create `src/components/editor/modal.rs`:

```rust
use leptos::prelude::*;

use crate::app::AppContext;
use crate::core::changes::ChangeType;
use crate::core::commands::result::SideEffect;
use crate::models::VirtualPath;

stylance::import_style!(styles, "modal.module.css");

#[component]
pub fn EditModal() -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext");
    let content = RwSignal::new(String::new());

    // When editor_open changes to Some(path), seed the textarea with current content.
    Effect::new(move |_| {
        if let Some(path) = ctx.editor_open.get() {
            let initial = ctx.view_fs.with(|fs| {
                fs.read_file(&path).unwrap_or_default()
            });
            content.set(initial);
        }
    });

    // Spec §9.1: ALL state transitions route through `dispatch_side_effect`.
    // The modal is a UI surface, not a mutation source — it emits
    // `SideEffect::ApplyChange` and lets the dispatcher update `ctx.changes`,
    // triggering persistence, autosave debounce, and any future hooks in one
    // place. Do NOT call `ctx.changes.update(...)` from here.
    let on_save = move |_| {
        if let Some(path) = ctx.editor_open.get_untracked() {
            let body = content.get_untracked();
            let is_existing = ctx.view_fs.with_untracked(|fs| fs.read_file(&path).is_some());
            let change = if is_existing {
                ChangeType::UpdateFile { content: body, description: None }
            } else {
                ChangeType::CreateFile { content: body, meta: Default::default() }
            };
            crate::components::terminal::terminal::dispatch_side_effect(
                &ctx,
                SideEffect::ApplyChange { path, change },
            );
            ctx.editor_open.set(None);
        }
    };

    let on_cancel = move |_| {
        ctx.editor_open.set(None);
    };

    view! {
        <Show when=move || ctx.editor_open.get().is_some() fallback=|| view! { <></> }>
            <div class=styles::backdrop on:click=on_cancel.clone()>
                <div class=styles::modal on:click=|ev| ev.stop_propagation()>
                    <header class=styles::header>
                        <span class=styles::path>{move || {
                            ctx.editor_open.get()
                                .map(|p| p.as_str().to_string())
                                .unwrap_or_default()
                        }}</span>
                    </header>
                    <textarea
                        class=styles::textarea
                        prop:value=move || content.get()
                        on:input=move |ev| content.set(event_target_value(&ev))
                    />
                    <footer class=styles::footer>
                        <button class=styles::cancel on:click=on_cancel>"Cancel"</button>
                        <button class=styles::save on:click=on_save>"Save"</button>
                    </footer>
                </div>
            </div>
        </Show>
    }
}
```

Create `src/components/editor/modal.module.css`:

```css
.backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}

.modal {
    background: var(--bg-primary, #111);
    color: var(--fg-primary, #eee);
    border: 1px solid var(--border, #333);
    width: min(800px, 90vw);
    height: min(600px, 85vh);
    display: flex;
    flex-direction: column;
    font-family: var(--font-mono, monospace);
}

.header {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border, #333);
    font-size: 0.9em;
}

.path {
    opacity: 0.7;
}

.textarea {
    flex: 1;
    padding: 12px;
    background: transparent;
    color: inherit;
    border: none;
    resize: none;
    font-family: inherit;
    font-size: 0.95em;
    outline: none;
}

.footer {
    padding: 8px 12px;
    border-top: 1px solid var(--border, #333);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
}

.save, .cancel {
    padding: 4px 12px;
    background: transparent;
    color: inherit;
    border: 1px solid var(--border, #333);
    font-family: inherit;
    cursor: pointer;
}

.save {
    border-color: var(--accent, #0af);
    color: var(--accent, #0af);
}
```

Create `src/components/editor/mod.rs`:

```rust
pub mod modal;
pub use modal::EditModal;
```

- [ ] **Step 2: Register module**

In `src/components/mod.rs`: `pub mod editor;`

- [ ] **Step 3: Mount the modal at app root**

In the root `view! { ... }` in `src/app.rs` (grep for the root App component's view macro), add `<crate::components::editor::EditModal />` next to the other top-level children (Terminal/Explorer/StatusBar).

- [ ] **Step 4: Build**

Run: `cargo build --target wasm32-unknown-unknown && trunk build --release`
Expected: green. If `event_target_value` is unresolved, import from `leptos::ev::*` or whatever path your Leptos version exposes (grep existing `on:input` usages).

- [ ] **Step 5: Commit**

```bash
git add src/components/editor/ src/components/mod.rs src/app.rs
git commit -m "feat(ui): EditModal with textarea + Save/Cancel"
```

### Task 5.3: Manual UI smoke test

- [ ] **Step 1: Dev server**

Run: `trunk serve` (foreground, user needs to open browser).

- [ ] **Step 2: Checklist — golden path**

In the browser, verify in order:
1. `touch /tmp/foo.md` — expect admin-required message (wallet not admin).
2. Connect wallet (allowlisted address). `touch /tmp/foo.md` — expect success, `sync status` shows 1 staged create.
3. `edit /tmp/foo.md` — modal opens, empty textarea. Type "hello", click Save. Modal closes.
4. `sync status` — still 1 staged (upsert collapses create+edit into latest content).
5. `sync auth ghp_testtoken` — expect "Storing token for session." Check DevTools → Application → Session Storage: key `websh.gh_token` is present.
6. Close tab, reopen — drafts persist via IDB; `sync status` still shows the foo.md entry.
7. `edit /tmp/foo.md` → Cancel — no change to ChangeSet.

- [ ] **Step 3: Checklist — edge cases**

1. `rm -r /home/wonjae/blog/post.md` — non-recursive rejects dirs; recursive accepts both.
2. `rmdir /home/wonjae/empty` on a non-existent dir — error.
3. Open EditModal, click backdrop outside the modal — closes without save.
4. `sync auth garbage` — "token format not recognized" error.

If any step fails, fix inline (commit a patch). Do not mark Phase 5 complete until all checklist items pass.

- [ ] **Step 4: Commit (if any patches applied)**

```bash
git add -u
git commit -m "fix(ui): address Phase 5 smoke-test findings"
```

---

## Phase 6 — Integration & finalize

### Task 6.1: README / CLAUDE.md updates

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`

- [ ] **Step 1: README — add a "Write capability" section**

After the "Features" or equivalent section, add:

```markdown
### Writing to the filesystem (Phase 3a)

Admins (wallets listed in the allowlist) can edit `~` and commit atomically to GitHub.

Commands:
- `touch <path>`, `mkdir <path>`, `rm [-r] <path>`, `rmdir <path>`, `edit <path>`
- `echo "body" > <path>` — write-or-replace file content
- `sync status` — show drafted changes
- `sync commit -m "<msg>"` — push staged changes atomically
- `sync refresh` — re-fetch remote manifest
- `sync auth <github_pat>` / `sync auth clear` — session-scoped token

Drafts persist in IndexedDB across reloads. Commits use GraphQL
`createCommitOnBranch` with `expectedHeadOid` compare-and-swap, so if the
remote moved since you started drafting, the commit fails with
"remote changed — run `sync refresh`" rather than clobbering.

**Security caveat:** the GitHub PAT lives in sessionStorage. Any injected
script can read it. See Phase 5 (CSP hardening) before wider admin rollout.
```

- [ ] **Step 2: CLAUDE.md — update "Public APIs added in Phase 2" block**

Append under its own heading:

```markdown
### Public APIs added in Phase 3a

- `core::changes::{ChangeSet, Entry, ChangeType, Summary}` — single-source-of-truth for in-progress edits.
- `core::merge::merge_view(base, changes) -> VirtualFs` — pure overlay.
- `core::admin::{admin_status, can_write_to, AdminStatus}` — allowlist gate.
- `core::storage::{StorageBackend, StorageError, CommitOutcome, GitHubBackend, idb, persist, boot}` — commit-path abstraction.
- `VirtualFs::serialize_manifest()` — byte-stable re-emission; covered by `tests/manifest_roundtrip.rs`.
- `Mount::is_writable()`, `Mount::github_writable(...)` — writability metadata.
- New `Command` variants: `Touch`, `Mkdir`, `Rm`, `Rmdir`, `Edit`, `EchoRedirect`, `Sync(SyncSubcommand)`.
- New `SideEffect` variants: `ApplyChange`, `StageChange`, `UnstageChange`, `DiscardChange`, `StageAll`, `UnstageAll`, `Commit`, `RefreshManifest`, `SetAuthToken`, `ClearAuthToken`, `OpenEditor`.
- `AppError::Storage(StorageError)` — commit-path errors surface through the existing error funnel.
- `AppContext` gains: `changes`, `view_fs`, `backend`, `remote_head`, `editor_open`.
```

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: Phase 3a write-capability public API surface"
```

### Task 6.2: Manual-QA checklist file

**Files:**
- Create: `docs/superpowers/checklists/2026-04-20-phase3a-manual-qa.md`

- [ ] **Step 1: Write the checklist**

Create the file with the Phase 5 smoke-test checklist PLUS a "commit against a real throwaway repo" section:

```markdown
# Phase 3a Manual QA Checklist

Run before tagging 3a complete.

## Dev-server smoke (no real commits)

- [ ] `trunk serve`, open in browser.
- [ ] Golden path: wallet connect → `touch` → `edit` → Save → `sync status` → reload → drafts persist.
- [ ] Edge: `rm -r` accepts dir, `rm` alone rejects dir.
- [ ] Edge: EditModal Cancel discards.
- [ ] Edge: `sync auth garbage` rejects.

## Real-commit smoke (throwaway repo + burnable PAT)

- [ ] Configure `config::mount_list()` to point at a test repo (NOT `0xwonj/db`).
- [ ] `sync auth <real-ghp-token-with-repo-scope>`
- [ ] `echo "hello" > /tmp/test.md`
- [ ] `sync commit -m "phase 3a smoke"`
- [ ] Verify on github.com: commit exists, author is the configured admin, `manifest.json` updated.
- [ ] `sync status` shows clean.
- [ ] Reload browser: ctx.fs reflects the new remote state.

## Conflict smoke

- [ ] Draft an edit (don't commit).
- [ ] From github.com UI, commit an unrelated change to the branch.
- [ ] `sync commit -m "..."` — expect `Conflict` error citing new SHA.
- [ ] `sync refresh` — base refreshes; drafts preserved.
- [ ] `sync commit -m "..."` again — succeeds.

## Rate-limit smoke (optional)

- [ ] With a legitimate PAT, make ~5 commits in quick succession.
- [ ] Verify no auto-retry loops.
- [ ] If rate-limited is hit: error surfaces with retry-after seconds.
```

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/checklists/2026-04-20-phase3a-manual-qa.md
git commit -m "docs(phase3a): manual QA checklist"
```

### Task 6.3: Phase 3a exit check — full suite + build + lint

- [ ] **Step 1: SHIP BLOCKER — replace the admin placeholder address**

`src/core/admin.rs` currently contains:

```rust
const ADMIN_ADDRESSES: &[&str] = &[
    "0x0000000000000000000000000000000000000000", // placeholder
];
```

The zero address is unspendable and unreachable — shipping with only this entry means nobody can actually write. Before tagging 3a:

1. Ask the operator for their real admin wallet address.
2. Replace the placeholder in `ADMIN_ADDRESSES` with that address (lowercased).
3. `grep -n "0x0000000000000000000000000000000000000000" src/core/admin.rs` → must return **no matches** after the edit (if the placeholder is still there, the release is not ready).
4. Commit:

```bash
git add src/core/admin.rs
git commit -m "chore(admin): wire real admin address for Phase 3a launch"
```

Do not proceed to Step 2 until this is done.

- [ ] **Step 2: Full suite**

Run: `cargo test && cargo clippy --target wasm32-unknown-unknown -- -D warnings && cargo build --target wasm32-unknown-unknown && trunk build --release`
Expected: all green.

- [ ] **Step 3: Run the manual QA checklist**

Work through `docs/superpowers/checklists/2026-04-20-phase3a-manual-qa.md` end-to-end. Do not declare 3a complete until all boxes are checked on an actual browser + actual throwaway repo.

- [ ] **Step 4: Final commit — update phase status docs**

If there's a master decision log for phase tracking (e.g. `docs/superpowers/plans/2026-04-20-phase-roadmap.md` or similar — grep), mark 3a complete there:

```bash
git add docs/superpowers/plans/2026-04-20-phase-roadmap.md
git commit -m "docs(roadmap): mark Phase 3a complete"
```

---

## Notes on flexibility vs. rigidity

**Rigid (follow exactly):**
- The `ChangeSet` shape (`BTreeMap<VirtualPath, Entry>`, inlined `staged: bool`).
- The `StorageBackend` trait's two methods.
- `expectedHeadOid` compare-and-swap on every commit.
- `VirtualFs::serialize_manifest()` must be byte-stable — the golden test enforces this.
- Manifest is injected by the dispatcher BEFORE calling `commit()`, not inside `GitHubBackend::commit`.

**Flexible (adapt to what's actually there):**
- Exact parser API (`Tokens::next_path_arg` etc.) — match whatever the current codebase exposes.
- `OutputLine` constructor names — may be `::info/::error/::warn` or may be direct struct literals.
- Leptos import paths — `leptos::prelude::*` vs. individual imports.
- CSS styling (Phase 5 modal) — minimal; improve if ugly, don't block on polish.

If you hit a conflict between this plan and reality, trust reality. If the conflict looks structural (e.g., "the parser can't express this"), surface it before committing — it may be a signal the plan missed something, not a signal to fight the codebase.
