# Mempool Phase 3 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:test-driven-development. Steps use checkbox (`- [ ]`) syntax for tracking; flip each to `- [x]` as it ships.

**Goal:** Ship promotion — move a draft from `/mempool/<category>/<slug>.md` to the canonical chain at `/<category>/<slug>.md` (bundle source mount) via a two-commit transaction (bundle add → mempool drop), with an explicit confirmation modal, partial-failure recovery, deploy-hint banner, and post-commit bookkeeping that keeps `remote_heads` fresh.

**Architecture:** A new `promote.rs` module owns:
- pure helpers (`promote_target_path`, `promote_commit_messages`, `build_bundle_add_change_set`, `build_mempool_drop_change_set`, `preflight_promote`),
- async orchestration (`commit_bundle_add`, `commit_mempool_drop`, `apply_commit_outcome`, `promote_entry`, `retry_mempool_drop`),
- a `PromoteConfirmModal` Leptos component that surfaces state to the user.

`Mempool` gains an `author_mode: Memo<bool>` prop and a per-item `on_promote: Callback<MempoolEntry>` so `MempoolItem` renders a Promote button when in author mode. `LedgerPage` mounts `PromoteConfirmModal` and renders a `PromoteStatusBanner` (deploy-hint or partial-failure) above the mempool section.

**Tech Stack:** Rust + Leptos 0.8 (csr), wasm32. Reuses `ChangeSet`, `commit_backend`, `runtime::state::github_token_for_commit`, `ctx.runtime_state.github_token_present`, `ctx.remote_heads`, `runtime::reload_runtime`, and `apply_runtime_load`.

**Master plan:** [`docs/superpowers/specs/2026-04-28-mempool-master.md`](../specs/2026-04-28-mempool-master.md)
**Phase 3 design:** [`docs/superpowers/specs/2026-04-28-mempool-phase3-design.md`](../specs/2026-04-28-mempool-phase3-design.md)

---

## Prerequisites

- Phase 2 merged (compose/edit shipped — see master §10).
- A GitHub PAT with `contents:write` on **both** `0xwonj/websh` (bundle source) and `0xwonj/websh-mempool` (mempool) for live testing. Phase 3 itself does not exercise the live path in CI; tests are pure-helper integration.

---

## Task 1: Pure helpers (path mapping, commit messages, change sets, preflight)

**Files:**
- Create: `src/components/mempool/promote.rs` (skeleton + tests + helpers, no UI yet)
- Modify: `src/components/mempool/mod.rs` (add `mod promote;` + re-exports)

### Steps

- [ ] **1.1: Create `promote.rs` with the test module first.** TDD — write failing tests, then implement.

```rust
//! Promotion: move a mempool draft onto the canonical chain.
//!
//! Two-commit transaction (sequential, not atomic):
//!   1. add the file under the bundle source mount at `/<category>/<slug>.md`
//!   2. delete the file from the mempool mount at `/mempool/<category>/<slug>.md`
//!
//! All non-async helpers in this module are pure and unit-testable. The
//! async pipeline lives in `promote_entry` (Task 3) and orchestrates the
//! two `commit_backend` calls plus post-commit bookkeeping.

use std::sync::Arc;

use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::storage::StorageBackend;
use crate::models::{FileMetadata, VirtualPath};

use super::loader::mempool_root;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromoteError {
    SourceNotInMempool(String),
    MempoolEntryMissing(VirtualPath),
    BundleTargetCollision(VirtualPath),
    BackendMissingFor(VirtualPath),
    TokenMissing,
    BodyReadFailed(String),
    BundleCommitFailed(String),
    MempoolCommitFailed(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromoteCommitMessages {
    pub bundle_add: String,
    pub mempool_drop: String,
}

/// Map a mempool source path to its canonical-chain destination.
/// Returns `Err(SourceNotInMempool)` if the source is not under `/mempool`.
pub fn promote_target_path(source: &VirtualPath) -> Result<VirtualPath, PromoteError> {
    let mempool = mempool_root();
    let rel = source
        .strip_prefix(&mempool)
        .ok_or_else(|| PromoteError::SourceNotInMempool(source.as_str().to_string()))?;
    if rel.is_empty() {
        return Err(PromoteError::SourceNotInMempool(source.as_str().to_string()));
    }
    let target = format!("/{rel}");
    VirtualPath::from_absolute(target)
        .map_err(|_| PromoteError::SourceNotInMempool(source.as_str().to_string()))
}

/// Build the two commit messages used by promotion.
pub fn promote_commit_messages(source: &VirtualPath) -> Result<PromoteCommitMessages, PromoteError> {
    let target = promote_target_path(source)?;
    let rel = target
        .as_str()
        .trim_start_matches('/')
        .trim_end_matches(".md");
    Ok(PromoteCommitMessages {
        bundle_add: format!("promote: add {rel}"),
        mempool_drop: format!("mempool: drop {rel} (promoted)"),
    })
}

/// `ChangeSet` for the bundle-source-add commit. Uses the file body as-is.
pub fn build_bundle_add_change_set(target: &VirtualPath, body: &str) -> ChangeSet {
    let mut changes = ChangeSet::new();
    changes.upsert(
        target.clone(),
        ChangeType::CreateFile {
            content: body.to_string(),
            meta: FileMetadata::default(),
        },
    );
    changes
}

/// `ChangeSet` for the mempool delete commit.
pub fn build_mempool_drop_change_set(source: &VirtualPath) -> ChangeSet {
    let mut changes = ChangeSet::new();
    changes.upsert(source.clone(), ChangeType::DeleteFile);
    changes
}

/// Synchronous preflight checks that run before any commit. Returns the
/// promotion target on success so the caller can keep going without
/// recomputing the mapping.
pub fn preflight_promote_paths(
    source: &VirtualPath,
    source_exists: bool,
    target_exists: bool,
    bundle_backend_present: bool,
    mempool_backend_present: bool,
    token_present: bool,
) -> Result<VirtualPath, PromoteError> {
    let target = promote_target_path(source)?;
    if !source_exists {
        return Err(PromoteError::MempoolEntryMissing(source.clone()));
    }
    if target_exists {
        return Err(PromoteError::BundleTargetCollision(target));
    }
    if !bundle_backend_present {
        return Err(PromoteError::BackendMissingFor(VirtualPath::root()));
    }
    if !mempool_backend_present {
        return Err(PromoteError::BackendMissingFor(mempool_root()));
    }
    if !token_present {
        return Err(PromoteError::TokenMissing);
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> VirtualPath {
        VirtualPath::from_absolute(s).unwrap()
    }

    #[test]
    fn target_path_strips_mempool_prefix() {
        assert_eq!(
            promote_target_path(&p("/mempool/writing/foo.md")).unwrap(),
            p("/writing/foo.md"),
        );
    }

    #[test]
    fn target_path_preserves_nested_categories() {
        assert_eq!(
            promote_target_path(&p("/mempool/papers/q/foo.md")).unwrap(),
            p("/papers/q/foo.md"),
        );
    }

    #[test]
    fn target_path_rejects_non_mempool_source() {
        let err = promote_target_path(&p("/writing/foo.md")).unwrap_err();
        assert!(matches!(err, PromoteError::SourceNotInMempool(_)));
    }

    #[test]
    fn target_path_rejects_root_mempool_path() {
        let err = promote_target_path(&p("/mempool")).unwrap_err();
        assert!(matches!(err, PromoteError::SourceNotInMempool(_)));
    }

    #[test]
    fn commit_messages_use_relative_path_without_extension() {
        let msgs = promote_commit_messages(&p("/mempool/writing/foo.md")).unwrap();
        assert_eq!(msgs.bundle_add, "promote: add writing/foo");
        assert_eq!(msgs.mempool_drop, "mempool: drop writing/foo (promoted)");
    }

    #[test]
    fn bundle_add_change_set_has_one_create_file() {
        let target = p("/writing/foo.md");
        let cs = build_bundle_add_change_set(&target, "---\ntitle: foo\n---\n\nbody\n");
        let entries: Vec<_> = cs.iter_all().collect();
        assert_eq!(entries.len(), 1);
        let (path, entry) = entries[0];
        assert_eq!(path, &target);
        assert!(matches!(&entry.change, ChangeType::CreateFile { .. }));
    }

    #[test]
    fn mempool_drop_change_set_has_one_delete_file() {
        let source = p("/mempool/writing/foo.md");
        let cs = build_mempool_drop_change_set(&source);
        let entries: Vec<_> = cs.iter_all().collect();
        assert_eq!(entries.len(), 1);
        let (path, entry) = entries[0];
        assert_eq!(path, &source);
        assert!(matches!(&entry.change, ChangeType::DeleteFile));
    }

    #[test]
    fn preflight_happy_path_returns_target() {
        let target = preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            true, false, true, true, true,
        )
        .unwrap();
        assert_eq!(target, p("/writing/foo.md"));
    }

    #[test]
    fn preflight_flags_each_failure_mode() {
        // Source missing
        assert!(matches!(
            preflight_promote_paths(&p("/mempool/writing/foo.md"), false, false, true, true, true),
            Err(PromoteError::MempoolEntryMissing(_))
        ));
        // Target collision
        assert!(matches!(
            preflight_promote_paths(&p("/mempool/writing/foo.md"), true, true, true, true, true),
            Err(PromoteError::BundleTargetCollision(_))
        ));
        // Bundle backend missing
        assert!(matches!(
            preflight_promote_paths(&p("/mempool/writing/foo.md"), true, false, false, true, true),
            Err(PromoteError::BackendMissingFor(_))
        ));
        // Mempool backend missing
        assert!(matches!(
            preflight_promote_paths(&p("/mempool/writing/foo.md"), true, false, true, false, true),
            Err(PromoteError::BackendMissingFor(_))
        ));
        // Token missing
        assert!(matches!(
            preflight_promote_paths(&p("/mempool/writing/foo.md"), true, false, true, true, false),
            Err(PromoteError::TokenMissing)
        ));
    }

    #[test]
    fn preflight_errors_carry_the_relevant_path() {
        match preflight_promote_paths(
            &p("/mempool/writing/foo.md"), true, true, true, true, true,
        ) {
            Err(PromoteError::BundleTargetCollision(p)) => assert_eq!(p, super::super::loader::mempool_root().join("writing/foo.md").parent().unwrap().parent().unwrap_or(VirtualPath::root()).join("writing/foo.md")),
            other => panic!("expected BundleTargetCollision, got {other:?}"),
        }
    }
}

// Suppress unused warning until Task 3 introduces async orchestration that
// uses the backend trait.
#[allow(dead_code)]
fn _backend_arc_marker(_backend: Arc<dyn StorageBackend>) {}
```

Note: the `preflight_errors_carry_the_relevant_path` test is fragile — keep it simpler: just assert the variant is `BundleTargetCollision(_)` and that the inner path string ends with `writing/foo.md`. (Refine in step 1.3.)

- [ ] **1.2: Run tests — confirm fail to compile / fail.**
- [ ] **1.3: Implement helpers exactly as above. Refine `preflight_errors_carry_the_relevant_path` to check `path.as_str().ends_with("writing/foo.md")`.**
- [ ] **1.4: Run tests — confirm green.** `cargo test --lib mempool::promote::tests`
- [ ] **1.5: Wire `mod.rs`** — add:
    ```rust
    mod promote;
    pub use promote::{
        PromoteCommitMessages, PromoteError, build_bundle_add_change_set,
        build_mempool_drop_change_set, preflight_promote_paths, promote_commit_messages,
        promote_target_path,
    };
    ```
- [ ] **1.6: Compile-check.** `cargo check --target wasm32-unknown-unknown --lib` clean.
- [ ] **1.7: Commit.** `git add -p src/components/mempool/promote.rs src/components/mempool/mod.rs` then:
    ```
    feat(mempool): add promote pure helpers (paths, messages, change sets, preflight)
    ```

---

## Task 2: Post-commit bookkeeping helper + Phase 2 fix

**Files:**
- Create: `src/components/mempool/promote.rs` (extend with `apply_commit_outcome` async helper)
- Modify: `src/components/mempool/compose.rs` (call `apply_commit_outcome` after the single commit so `remote_heads` stays fresh)

**Why this task is here:** The advisor flagged that Phase 2's `save_compose` discards `CommitOutcome.new_head`, leaving `ctx.remote_heads` stale. Phase 3 needs the same plumbing for both arms of its transaction. Centralizing the helper now and applying it to compose closes a latent bug while the area is open.

### Steps

- [ ] **2.1: Add `apply_commit_outcome` to `promote.rs`** (no new tests — the helper is thin and exercised by the integration test in Task 5):

```rust
use crate::app::AppContext;
use crate::core::storage::CommitOutcome;
use crate::models::RuntimeMount;

/// Update `ctx.remote_heads` and persist the new head to IDB so subsequent
/// `expected_head` lookups for the same mount are fresh. Best-effort: a
/// failed IDB write is logged but does not poison the in-memory signal.
pub async fn apply_commit_outcome(
    ctx: &AppContext,
    mount_root: &VirtualPath,
    outcome: &CommitOutcome,
) {
    ctx.remote_heads.update(|map| {
        map.insert(mount_root.clone(), outcome.new_head.clone());
    });

    let storage_id = ctx
        .runtime_mounts
        .with_untracked(|mounts| {
            mounts
                .iter()
                .find(|m| &m.root == mount_root)
                .map(RuntimeMount::storage_id)
        })
        .unwrap_or_else(|| mount_id_for_root(mount_root));

    if let Ok(db) = crate::core::storage::idb::open_db().await {
        if let Err(error) = crate::core::storage::idb::save_metadata(
            &db,
            &format!("remote_head.{storage_id}"),
            &outcome.new_head,
        )
        .await
        {
            leptos::logging::warn!(
                "promote: persist remote_head for {mount_root} failed: {error}"
            );
        }
    }
}

fn mount_id_for_root(root: &VirtualPath) -> String {
    if root.is_root() {
        "~".to_string()
    } else {
        root.as_str().trim_start_matches('/').replace('/', ":")
    }
}
```

(Re-use the same `mount_id_for_root` shape that `terminal.rs` uses. If terminal already exposes it publicly, prefer importing; otherwise duplicate the trivial helper here.)

- [ ] **2.2: Verify `mount_id_for_root` symmetry.** Grep `terminal.rs` for the existing helper:
    ```bash
    rg "fn mount_id_for_root" src/
    ```
    If it's already public somewhere, import it; otherwise the duplicate above is fine and small.

- [ ] **2.3: Update `compose.rs::save_compose`** so it applies the outcome:

  Replace:
  ```rust
  commit_backend(backend, root, changes, message, expected_head, Some(token))
      .await
      .map(|_outcome| ())
      .map_err(|err| err.to_string())
  ```
  with:
  ```rust
  let outcome = commit_backend(backend, root.clone(), changes, message, expected_head, Some(token))
      .await
      .map_err(|err| err.to_string())?;
  super::promote::apply_commit_outcome(&ctx, &root, &outcome).await;
  Ok(())
  ```

- [ ] **2.4: Compile-check.** `cargo check --target wasm32-unknown-unknown --lib` and `cargo test --lib` (existing compose tests should still pass — they use `build_change_set`/`commit_message`, not the async path).
- [ ] **2.5: Wire `mod.rs`** — re-export `apply_commit_outcome` for use by both promote and (already-internal) compose call sites:
    ```rust
    pub use promote::{..., apply_commit_outcome};
    ```
- [ ] **2.6: Commit.**
    ```
    feat(mempool): persist commit head after compose/promote (close stale remote_heads)
    ```

---

## Task 3: Async promote orchestration

**Files:**
- Modify: `src/components/mempool/promote.rs` (add `promote_entry` + `retry_mempool_drop` + `PromoteState` enum)

### Steps

- [ ] **3.1: Add `PromoteState` enum and async helpers.**

```rust
use crate::core::runtime::{commit_backend, reload_runtime};
use crate::core::runtime::state::github_token_for_commit;

/// Visible state of an in-progress / finished promotion. `LedgerPage` and
/// `PromoteConfirmModal` read this to drive their banners and buttons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromoteState {
    Idle,
    Confirming { source: VirtualPath, target: VirtualPath },
    Running { source: VirtualPath, target: VirtualPath },
    PartialFailure {
        source: VirtualPath,
        target: VirtualPath,
        error: String,
    },
    Done { source: VirtualPath, target: VirtualPath },
    Failed {
        source: VirtualPath,
        error: String,
    },
}

/// Run the full two-commit promotion. On bundle-add failure: returns a Failed
/// terminal state without touching the mempool. On mempool-drop failure after
/// bundle-add succeeded: returns PartialFailure so the UI can offer Retry.
pub async fn promote_entry(
    ctx: AppContext,
    source: VirtualPath,
) -> PromoteState {
    let target = match promote_target_path(&source) {
        Ok(t) => t,
        Err(err) => {
            return PromoteState::Failed {
                source,
                error: format!("{err:?}"),
            };
        }
    };

    let bundle_root = VirtualPath::root();
    let mempool = mempool_root();

    let source_exists = ctx.view_global_fs.with(|fs| fs.exists(&source));
    let target_exists = ctx.view_global_fs.with(|fs| fs.exists(&target));
    let bundle_backend = ctx.backend_for_path(&bundle_root);
    let mempool_backend = ctx.backend_for_path(&mempool);
    let token = github_token_for_commit();

    if let Err(err) = preflight_promote_paths(
        &source,
        source_exists,
        target_exists,
        bundle_backend.is_some(),
        mempool_backend.is_some(),
        token.is_some(),
    ) {
        return PromoteState::Failed {
            source,
            error: humanize_promote_error(&err),
        };
    }
    let bundle_backend = bundle_backend.expect("preflight ensured");
    let mempool_backend = mempool_backend.expect("preflight ensured");
    let token = token.expect("preflight ensured");

    // Re-read the mempool body fresh so we don't carry stale UI state.
    let body = match ctx.read_text(&source).await {
        Ok(body) => body,
        Err(error) => {
            return PromoteState::Failed {
                source,
                error: format!("{error}"),
            };
        }
    };

    let messages = match promote_commit_messages(&source) {
        Ok(m) => m,
        Err(err) => {
            return PromoteState::Failed {
                source,
                error: humanize_promote_error(&err),
            };
        }
    };

    // Commit #1: bundle source add.
    let bundle_changes = build_bundle_add_change_set(&target, &body);
    let bundle_expected_head = ctx.remote_head_for_path(&bundle_root);
    let bundle_outcome = commit_backend(
        bundle_backend,
        bundle_root.clone(),
        bundle_changes,
        messages.bundle_add.clone(),
        bundle_expected_head,
        Some(token.clone()),
    )
    .await;
    let bundle_outcome = match bundle_outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            return PromoteState::Failed {
                source,
                error: format!("{error}"),
            };
        }
    };
    apply_commit_outcome(&ctx, &bundle_root, &bundle_outcome).await;

    // Commit #2: mempool drop.
    match commit_mempool_drop(&ctx, &source, &mempool_backend, &messages.mempool_drop, &token).await {
        Ok(outcome) => {
            apply_commit_outcome(&ctx, &mempool, &outcome).await;
            // Refresh runtime so view_global_fs reflects both repo states.
            reload_and_apply(&ctx).await;
            PromoteState::Done { source, target }
        }
        Err(error) => PromoteState::PartialFailure {
            source,
            target,
            error,
        },
    }
}

/// The mempool-drop arm exposed separately so `retry_mempool_drop` can
/// re-invoke it after a partial failure.
async fn commit_mempool_drop(
    _ctx: &AppContext,
    source: &VirtualPath,
    backend: &Arc<dyn StorageBackend>,
    message: &str,
    token: &str,
) -> Result<CommitOutcome, String> {
    let mempool = mempool_root();
    let changes = build_mempool_drop_change_set(source);
    let expected_head = _ctx.remote_head_for_path(&mempool);
    commit_backend(
        backend.clone(),
        mempool,
        changes,
        message.to_string(),
        expected_head,
        Some(token.to_string()),
    )
    .await
    .map_err(|err| err.to_string())
}

/// Replay only the mempool-drop arm. Used by the retry button after a partial
/// failure. Returns Done on success and PartialFailure on continued failure.
pub async fn retry_mempool_drop(
    ctx: AppContext,
    source: VirtualPath,
    target: VirtualPath,
) -> PromoteState {
    let mempool = mempool_root();
    let backend = match ctx.backend_for_path(&mempool) {
        Some(b) => b,
        None => {
            return PromoteState::PartialFailure {
                source,
                target,
                error: "mempool backend not configured".to_string(),
            };
        }
    };
    let token = match github_token_for_commit() {
        Some(t) => t,
        None => {
            return PromoteState::PartialFailure {
                source,
                target,
                error: "missing GitHub token".to_string(),
            };
        }
    };

    let messages = match promote_commit_messages(&source) {
        Ok(m) => m,
        Err(err) => {
            return PromoteState::PartialFailure {
                source,
                target,
                error: humanize_promote_error(&err),
            };
        }
    };

    match commit_mempool_drop(&ctx, &source, &backend, &messages.mempool_drop, &token).await {
        Ok(outcome) => {
            apply_commit_outcome(&ctx, &mempool, &outcome).await;
            reload_and_apply(&ctx).await;
            PromoteState::Done { source, target }
        }
        Err(error) => PromoteState::PartialFailure {
            source,
            target,
            error,
        },
    }
}

async fn reload_and_apply(ctx: &AppContext) {
    match reload_runtime().await {
        Ok(load) => ctx.apply_runtime_load(load),
        Err(error) => leptos::logging::warn!(
            "promote: runtime reload after commit failed: {error}"
        ),
    }
}

fn humanize_promote_error(err: &PromoteError) -> String {
    match err {
        PromoteError::SourceNotInMempool(p) => format!("{p} is not under /mempool"),
        PromoteError::MempoolEntryMissing(p) => format!("mempool entry {p} no longer exists"),
        PromoteError::BundleTargetCollision(p) => format!(
            "{p} already exists in the canonical chain — cannot promote without overwriting"
        ),
        PromoteError::BackendMissingFor(p) => format!("no backend configured for {p}"),
        PromoteError::TokenMissing => "missing GitHub token".to_string(),
        PromoteError::BodyReadFailed(s) => format!("failed to read mempool body: {s}"),
        PromoteError::BundleCommitFailed(s) | PromoteError::MempoolCommitFailed(s) => s.clone(),
    }
}
```

Note on imports: `use crate::core::storage::{StorageBackend, CommitOutcome};` (the previous Task 1 stub already imports `StorageBackend` for the marker). Drop the marker once `commit_mempool_drop` actually uses `Arc<dyn StorageBackend>`.

- [ ] **3.2: Compile-check.** `cargo check --target wasm32-unknown-unknown --lib` clean.
- [ ] **3.3: `mod.rs` re-exports.** Add `PromoteState`, `promote_entry`, `retry_mempool_drop`, `humanize_promote_error` to the existing `pub use promote::{...}` block.
- [ ] **3.4: Commit.**
    ```
    feat(mempool): wire two-commit promote orchestration with partial-failure recovery
    ```

---

## Task 4: PromoteConfirmModal component + Promote button on MempoolItem

**Files:**
- Modify: `src/components/mempool/promote.rs` (add `#[component] PromoteConfirmModal`)
- Create: `src/components/mempool/promote.module.css`
- Modify: `src/components/mempool/component.rs` (add `author_mode` prop + Promote button on `MempoolItem`)
- Modify: `src/components/mempool/mempool.module.css` (add `.mpActions`, `.mpPromote`)

### 4a. Modal

- [ ] **4.1: Implement `PromoteConfirmModal`.**

```rust
#[component]
pub fn PromoteConfirmModal(
    state: ReadSignal<PromoteState>,
    set_state: WriteSignal<PromoteState>,
    #[prop(into)] on_done: Callback<()>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let close = move || {
        set_state.set(PromoteState::Idle);
    };

    let on_confirm = {
        let ctx = ctx.clone();
        move |_| {
            let current = state.get_untracked();
            let (source, target) = match current {
                PromoteState::Confirming { source, target } => (source, target),
                _ => return,
            };
            set_state.set(PromoteState::Running {
                source: source.clone(),
                target: target.clone(),
            });
            let ctx = ctx.clone();
            spawn_local(async move {
                let next = promote_entry(ctx, source).await;
                let done = matches!(next, PromoteState::Done { .. });
                set_state.set(next);
                if done {
                    on_done.run(());
                }
            });
        }
    };

    let on_retry = {
        let ctx = ctx.clone();
        move |_| {
            let (source, target) = match state.get_untracked() {
                PromoteState::PartialFailure { source, target, .. } => (source, target),
                _ => return,
            };
            set_state.set(PromoteState::Running {
                source: source.clone(),
                target: target.clone(),
            });
            let ctx = ctx.clone();
            spawn_local(async move {
                let next = retry_mempool_drop(ctx, source, target).await;
                let done = matches!(next, PromoteState::Done { .. });
                set_state.set(next);
                if done {
                    on_done.run(());
                }
            });
        }
    };

    view! {
        <Show when=move || matches!(
            state.get(),
            PromoteState::Confirming { .. }
                | PromoteState::Running { .. }
                | PromoteState::PartialFailure { .. }
                | PromoteState::Failed { .. }
        )>
            <div class=css::backdrop on:click=move |_| close()>
                <div
                    class=css::panel
                    role="dialog"
                    aria-label="Promote mempool entry"
                    on:click=|ev: leptos::ev::MouseEvent| ev.stop_propagation()
                >
                    <header class=css::header>
                        <span class=css::title>"promote to canonical chain"</span>
                        <button class=css::close type="button" aria-label="Close"
                            on:click=move |_| close()>"\u{00d7}"</button>
                    </header>
                    <div class=css::body>
                        {move || match state.get() {
                            PromoteState::Confirming { source, target }
                            | PromoteState::Running { source, target }
                            | PromoteState::PartialFailure { source, target, .. }
                            | PromoteState::Done { source, target } => view! {
                                <PromoteRows source=source target=target />
                            }.into_any(),
                            _ => view! { <span></span> }.into_any(),
                        }}
                        {move || match state.get() {
                            PromoteState::Running { .. } => view! {
                                <div class=css::status>"committing… two commits, sequential"</div>
                            }.into_any(),
                            PromoteState::PartialFailure { error, .. } => view! {
                                <div class=css::partial role="alert">
                                    {format!("bundle commit OK, mempool delete failed: {error}")}
                                </div>
                            }.into_any(),
                            PromoteState::Failed { error, .. } => view! {
                                <div class=css::error role="alert">{error}</div>
                            }.into_any(),
                            _ => view! { <span></span> }.into_any(),
                        }}
                    </div>
                    <footer class=css::footer>
                        {move || match state.get() {
                            PromoteState::Confirming { .. } => view! {
                                <button class=css::cancel type="button" on:click=move |_| close()>"Cancel"</button>
                                <button class=css::confirm type="button" on:click=on_confirm>"Confirm promote"</button>
                            }.into_any(),
                            PromoteState::Running { .. } => view! {
                                <button class=css::cancel type="button" disabled=true>"Cancel"</button>
                                <button class=css::confirm type="button" disabled=true>"Promoting…"</button>
                            }.into_any(),
                            PromoteState::PartialFailure { .. } => view! {
                                <button class=css::cancel type="button" on:click=move |_| close()>"Dismiss"</button>
                                <button class=css::confirm type="button" on:click=on_retry>"Retry mempool delete"</button>
                            }.into_any(),
                            PromoteState::Failed { .. } => view! {
                                <button class=css::cancel type="button" on:click=move |_| close()>"Close"</button>
                            }.into_any(),
                            _ => view! { <span></span> }.into_any(),
                        }}
                    </footer>
                </div>
            </div>
        </Show>
    }
}

#[component]
fn PromoteRows(source: VirtualPath, target: VirtualPath) -> impl IntoView {
    view! {
        <div class=css::pathRow>
            <span class=css::pathKey>"from"</span>
            <code>{source.as_str().to_string()}</code>
        </div>
        <div class=css::pathRow>
            <span class=css::pathKey>"to"</span>
            <code>{target.as_str().to_string()}</code>
        </div>
        <div class=css::pathRow>
            <span class=css::pathKey>"commits"</span>
            <span>"2 (add bundle, drop mempool)"</span>
        </div>
    }
}
```

The `on_done` callback is wired in Task 5 to flip `LedgerPage::deploy_hint` and trigger `mempool_refresh.update`.

The component reads `Done` only via `on_done`; Done state never renders inside the modal because the caller closes it on success. PartialFailure states keep the modal open for retry.

- [ ] **4.2: Add `stylance::import_crate_style!`** at the top:
    ```rust
    stylance::import_crate_style!(css, "src/components/mempool/promote.module.css");
    ```

- [ ] **4.3: Write `promote.module.css`** matching the compose modal idiom (smaller panel since the form is two rows). Style classes: `backdrop`, `panel`, `header`, `title`, `close`, `body`, `pathRow`, `pathKey`, `status`, `partial`, `error`, `footer`, `cancel`, `confirm`. See `compose.module.css` for tokens.

### 4b. MempoolItem button

- [ ] **4.4: Add Promote button to `MempoolItem`.** Modify `Mempool` to take an `author_mode: Memo<bool>` and `on_promote: Callback<MempoolEntry>`; thread both into `MempoolItem`. Inside `MempoolItem`, render a button gated on `author_mode.get()`:

    ```rust
    <Show when=move || author_mode.get()>
        <button
            class=css::mpPromote
            type="button"
            aria-label="Promote to canonical chain"
            on:click=move |ev: leptos::ev::MouseEvent| {
                ev.stop_propagation();
                on_promote.run(entry_for_promote.clone());
            }
        >
            "promote ↗"
        </button>
    </Show>
    ```

    `event.stop_propagation()` is critical: without it, the button click also fires the row click (which opens the editor in author mode).

- [ ] **4.5: Add `.mpActions`, `.mpPromote` rules to `mempool.module.css`.** Subtle border, accent color on hover.

- [ ] **4.6: Compile-check.** `cargo check --target wasm32-unknown-unknown --lib` clean.

- [ ] **4.7: Commit.**
    ```
    feat(mempool): add PromoteConfirmModal + per-item Promote button
    ```

---

## Task 5: LedgerPage wiring (modal mount, deploy-hint banner, partial-failure banner)

**Files:**
- Modify: `src/components/ledger_page.rs`
- Modify: `src/components/ledger_page.module.css` (add `.deployHint`, `.partialBanner`)

### Steps

- [ ] **5.1: Add new signals in `LedgerPage`:**

```rust
let (promote_state, set_promote_state) = signal(PromoteState::Idle);
let (deploy_hint, set_deploy_hint) = signal(None::<DeployHint>);
let (partial_warning, set_partial_warning) = signal(None::<PartialWarning>);

#[derive(Clone, Debug)]
struct DeployHint {
    target: VirtualPath,
}
#[derive(Clone, Debug)]
struct PartialWarning {
    source: VirtualPath,
    target: VirtualPath,
    error: String,
}
```

- [ ] **5.2: Plumb the per-item Promote callback** so clicking opens the confirm modal:

```rust
let on_promote = Callback::new(move |entry: MempoolEntry| {
    let target = match promote_target_path(&entry.path) {
        Ok(t) => t,
        Err(err) => {
            leptos::logging::warn!("promote: invalid source {}: {err:?}", entry.path.as_str());
            return;
        }
    };
    set_promote_state.set(PromoteState::Confirming { source: entry.path, target });
});
```

- [ ] **5.3: Watch `promote_state` for terminal transitions** to drive the banners:

```rust
Effect::new(move |_| {
    match promote_state.get() {
        PromoteState::Done { source: _, target } => {
            set_deploy_hint.set(Some(DeployHint { target: target.clone() }));
            set_partial_warning.set(None);
            mempool_refresh.update(|n| *n += 1);
            set_promote_state.set(PromoteState::Idle);
        }
        PromoteState::PartialFailure { source, target, error } => {
            // Modal stays open; if the user dismisses without retrying,
            // we still want a sticky banner. The dismiss path lives in the
            // modal's Cancel handler — when state is Idle and we have a
            // pending PartialWarning, the banner renders.
            set_partial_warning.set(Some(PartialWarning {
                source: source.clone(),
                target: target.clone(),
                error: error.clone(),
            }));
        }
        _ => {}
    }
});

let on_promote_done = Callback::new(move |_| {
    // Empty hook — the Effect above handles the transitions.
});
```

(Alternative: skip the `Effect` and let `PromoteConfirmModal::on_done` set both signals directly. Pick whichever reads cleaner; the plan uses the Effect to keep state-transition logic in one place.)

- [ ] **5.4: Render banners** between `LedgerFilterBar` and the mempool section:

```rust
view! {
    <PromoteStatusBanner
        deploy_hint=deploy_hint
        set_deploy_hint=set_deploy_hint
        partial_warning=partial_warning
        set_partial_warning=set_partial_warning
    />
}
```

…and define:

```rust
#[component]
fn PromoteStatusBanner(
    deploy_hint: ReadSignal<Option<DeployHint>>,
    set_deploy_hint: WriteSignal<Option<DeployHint>>,
    partial_warning: ReadSignal<Option<PartialWarning>>,
    set_partial_warning: WriteSignal<Option<PartialWarning>>,
) -> impl IntoView {
    view! {
        {move || partial_warning.get().map(|w| view! {
            <div class=css::partialBanner role="alert">
                <span>{format!(
                    "{} promoted to bundle but mempool delete failed: {}",
                    w.target.as_str(), w.error,
                )}</span>
                <button type="button" class=css::dismiss
                    on:click=move |_| set_partial_warning.set(None)>
                    "dismiss"
                </button>
            </div>
        })}
        {move || deploy_hint.get().map(|h| view! {
            <div class=css::deployHint>
                <span>{format!("✓ promoted {}. run `just pin` to publish.", h.target.as_str())}</span>
                <button type="button" class=css::dismiss
                    on:click=move |_| set_deploy_hint.set(None)>
                    "dismiss"
                </button>
            </div>
        })}
    }
}
```

- [ ] **5.5: Mount `PromoteConfirmModal`** at the page root alongside `MempoolPreviewModal` and `ComposeModal`:

```rust
<PromoteConfirmModal
    state=promote_state
    set_state=set_promote_state
    on_done=on_promote_done
/>
```

- [ ] **5.6: Update the `Mempool` component invocation** so it receives `author_mode` and `on_promote`:

```rust
<Mempool
    model=mempool_model
    author_mode=author_mode
    on_select=on_select
    on_promote=on_promote
/>
```

- [ ] **5.7: Add `.deployHint`, `.partialBanner`, `.dismiss` styles** to `ledger_page.module.css`. Use existing tokens (`--accent`, `--danger`, `--bg-elevated`).

- [ ] **5.8: Compile-check.** `cargo check --target wasm32-unknown-unknown --lib` clean.

- [ ] **5.9: Commit.**
    ```
    feat(mempool): plumb promote modal + deploy-hint / partial-failure banners
    ```

---

## Task 6: Integration tests for the promote flow

**Files:**
- Create: `tests/mempool_promote.rs`

The pure helpers in `promote.rs` already have unit tests. The integration test mirrors `tests/mempool_compose.rs` style: covers the public surface from a downstream consumer's POV, exercises path mapping, commit messages, change-set shapes, and the preflight matrix.

### Steps

- [ ] **6.1: Write `tests/mempool_promote.rs`:**

```rust
//! Integration tests for mempool promote helpers. Pure-helper coverage: no
//! backend, no Leptos runtime. Live transaction is exercised manually when
//! `0xwonj/websh-mempool` is provisioned (deferred per master §10).

use websh::components::mempool::{
    PromoteError, build_bundle_add_change_set, build_mempool_drop_change_set,
    preflight_promote_paths, promote_commit_messages, promote_target_path,
};
use websh::core::changes::ChangeType;
use websh::models::VirtualPath;

fn p(s: &str) -> VirtualPath {
    VirtualPath::from_absolute(s).unwrap()
}

#[test]
fn target_path_strips_mempool_prefix_and_preserves_category() {
    assert_eq!(
        promote_target_path(&p("/mempool/writing/foo.md")).unwrap(),
        p("/writing/foo.md"),
    );
    assert_eq!(
        promote_target_path(&p("/mempool/papers/series/foo.md")).unwrap(),
        p("/papers/series/foo.md"),
    );
}

#[test]
fn target_path_rejects_paths_outside_mempool() {
    assert!(matches!(
        promote_target_path(&p("/writing/foo.md")),
        Err(PromoteError::SourceNotInMempool(_))
    ));
}

#[test]
fn commit_messages_format_relative_path_without_extension() {
    let msgs = promote_commit_messages(&p("/mempool/writing/on-slow.md")).unwrap();
    assert_eq!(msgs.bundle_add, "promote: add writing/on-slow");
    assert_eq!(msgs.mempool_drop, "mempool: drop writing/on-slow (promoted)");
}

#[test]
fn bundle_add_change_set_creates_one_file_with_body() {
    let target = p("/writing/foo.md");
    let body = "---\ntitle: foo\n---\n\nbody\n";
    let cs = build_bundle_add_change_set(&target, body);
    let entries: Vec<_> = cs.iter_all().collect();
    assert_eq!(entries.len(), 1);
    let (path, entry) = entries[0];
    assert_eq!(path, &target);
    match &entry.change {
        ChangeType::CreateFile { content, .. } => assert_eq!(content, body),
        other => panic!("expected CreateFile, got {other:?}"),
    }
}

#[test]
fn mempool_drop_change_set_deletes_one_file() {
    let source = p("/mempool/writing/foo.md");
    let cs = build_mempool_drop_change_set(&source);
    let entries: Vec<_> = cs.iter_all().collect();
    assert_eq!(entries.len(), 1);
    let (path, entry) = entries[0];
    assert_eq!(path, &source);
    assert!(matches!(entry.change, ChangeType::DeleteFile));
}

#[test]
fn preflight_returns_target_when_all_inputs_valid() {
    let target = preflight_promote_paths(
        &p("/mempool/writing/foo.md"),
        true, false, true, true, true,
    )
    .unwrap();
    assert_eq!(target, p("/writing/foo.md"));
}

#[test]
fn preflight_flags_missing_source() {
    assert!(matches!(
        preflight_promote_paths(&p("/mempool/writing/foo.md"), false, false, true, true, true),
        Err(PromoteError::MempoolEntryMissing(_))
    ));
}

#[test]
fn preflight_flags_target_collision() {
    assert!(matches!(
        preflight_promote_paths(&p("/mempool/writing/foo.md"), true, true, true, true, true),
        Err(PromoteError::BundleTargetCollision(_))
    ));
}

#[test]
fn preflight_flags_missing_bundle_backend() {
    assert!(matches!(
        preflight_promote_paths(&p("/mempool/writing/foo.md"), true, false, false, true, true),
        Err(PromoteError::BackendMissingFor(_))
    ));
}

#[test]
fn preflight_flags_missing_mempool_backend() {
    assert!(matches!(
        preflight_promote_paths(&p("/mempool/writing/foo.md"), true, false, true, false, true),
        Err(PromoteError::BackendMissingFor(_))
    ));
}

#[test]
fn preflight_flags_missing_token() {
    assert!(matches!(
        preflight_promote_paths(&p("/mempool/writing/foo.md"), true, false, true, true, false),
        Err(PromoteError::TokenMissing)
    ));
}
```

- [ ] **6.2: Run tests.** `cargo test --test mempool_promote`. All green.

- [ ] **6.3: Commit.**
    ```
    test(mempool): integration tests for promote helpers
    ```

---

## Task 7: Final verification + reviewer agent + master plan update

### Steps

- [ ] **7.1: Run full local verification.**

```bash
cargo test --lib
cargo test --test mempool_model
cargo test --test mempool_compose
cargo test --test mempool_promote
cargo check --target wasm32-unknown-unknown --lib
```

All four green. (Visual QA is **skipped** per user direction; deferred to first natural opportunity together with Phase 2 visual QA, per master §10.)

- [ ] **7.2: Dispatch `superpowers:code-reviewer`** with full Phase 3 diff. Pass: design doc + plan + diff. Address any CRITICAL / HIGH findings before declaring done.

- [ ] **7.3: Update master plan §4** — Phase 3 status `Complete`, V1 done.

- [ ] **7.4: Update master plan §6** — Phase 3 design Approved, plan Complete.

- [ ] **7.5: Append to §10 Decision Log:** something like
    ```
    | 2026-04-28 | Phase 3 (promotion) complete: 6 feat/test/fix commits + master update; visual QA deferred together with Phase 2 until repo provisioned. | §4 |
    ```

- [ ] **7.6: Commit master + plan close.**
    ```
    docs(mempool): mark Phase 3 complete in master plan
    ```

---

## Self-Review

The plan covers Phase 3 design §1 (scope), §2 (anchors — encoded in helper signatures), §3 (transaction sequence — Tasks 1+3), §4 (UX — Tasks 4+5), §5 (component tree — Tasks 4+5), §6 (files — Tasks 1+2+3+4+5+6), §7 (test strategy — Tasks 1, 6), §8 (risks — addressed via PartialFailure recovery + preflight), §9 (acceptance — Task 7), §10 (open questions — resolved in §3.4 of design).

Type signatures are consistent: `PromoteError`, `PromoteCommitMessages`, `PromoteState` defined in Tasks 1 + 3; consumed in Task 4 (modal) and Task 5 (page wiring).

Code-bearing steps include their full snippets except for CSS (left to "match compose.module.css idiom" — fully discretionary). The integration test in Task 6 compiles directly from this plan.

Risks I'm watching while implementing:
- `Effect::new` in 5.3 vs explicit on_done callback: both work; pick whichever Leptos 0.8 reads cleanly. If `Effect::new` causes infinite re-runs because it writes to the signal it reads, fall back to the explicit callback path.
- The `_ctx` underscore in `commit_mempool_drop` is intentional (it currently only uses `&AppContext` for `remote_head_for_path`); `cargo check` will warn but not fail.
- If `cargo check --target wasm32-unknown-unknown --lib` complains about `&Arc<dyn StorageBackend>` lifetimes on the trait method, switch the inner helper to take `Arc<dyn StorageBackend>` by value (cloning is cheap).
