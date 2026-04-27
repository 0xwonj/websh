//! Promotion: move a mempool draft onto the canonical chain.
//!
//! Two-commit transaction (sequential, not atomic):
//!   1. add the file under the bundle source mount at `/<category>/<slug>.md`
//!   2. delete the file from the mempool mount at `/mempool/<category>/<slug>.md`
//!
//! All non-async helpers in this module are pure and unit-testable. The
//! async pipeline lives in `promote_entry` and orchestrates the two
//! `commit_backend` calls plus post-commit bookkeeping.

use std::sync::Arc;

use leptos::prelude::{Update, With, WithUntracked};

use crate::app::AppContext;
use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::runtime::state::github_token_for_commit;
use crate::core::runtime::{commit_backend, reload_runtime};
use crate::core::storage::{CommitOutcome, StorageBackend};
use crate::models::{FileMetadata, RuntimeMount, VirtualPath};

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
/// Returns `Err(SourceNotInMempool)` if the source is not strictly nested
/// underneath `/mempool` (a path equal to `/mempool` is also rejected).
pub fn promote_target_path(source: &VirtualPath) -> Result<VirtualPath, PromoteError> {
    let mempool = mempool_root();
    let rel = source
        .strip_prefix(&mempool)
        .ok_or_else(|| PromoteError::SourceNotInMempool(source.as_str().to_string()))?;
    if rel.is_empty() {
        return Err(PromoteError::SourceNotInMempool(source.as_str().to_string()));
    }
    VirtualPath::from_absolute(format!("/{rel}"))
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

/// Synchronous preflight that runs before any commit. Returns the promotion
/// target on success so callers don't recompute the mapping.
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

/// Update `ctx.remote_heads` and persist the new HEAD to IDB so subsequent
/// `expected_head` lookups for the same mount reflect the just-committed
/// OID. Best-effort: a failed IDB write is logged but does not poison the
/// in-memory signal. Mirrors the bookkeeping the terminal `sync` flow does
/// after its commit, so author-driven flows do not drift.
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
        .unwrap_or_else(|| mount_id_fallback(mount_root));

    if let Ok(db) = crate::core::storage::idb::open_db().await {
        if let Err(error) = crate::core::storage::idb::save_metadata(
            &db,
            &format!("remote_head.{storage_id}"),
            &outcome.new_head,
        )
        .await
        {
            leptos::logging::warn!(
                "promote: persist remote_head for {} failed: {error}",
                mount_root.as_str()
            );
        }
    }
}

fn mount_id_fallback(root: &VirtualPath) -> String {
    if root.is_root() {
        "~".to_string()
    } else {
        root.as_str().trim_start_matches('/').replace('/', ":")
    }
}

/// Visible state of the promote flow. `LedgerPage` and `PromoteConfirmModal`
/// drive their banners and buttons off this enum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromoteState {
    Idle,
    Confirming {
        source: VirtualPath,
        target: VirtualPath,
    },
    Running {
        source: VirtualPath,
        target: VirtualPath,
    },
    PartialFailure {
        source: VirtualPath,
        target: VirtualPath,
        error: String,
    },
    Done {
        source: VirtualPath,
        target: VirtualPath,
    },
    Failed {
        source: VirtualPath,
        error: String,
    },
}

/// Run the full two-commit promotion. On bundle-add failure: returns Failed
/// without touching the mempool. On mempool-drop failure after bundle-add
/// succeeded: returns PartialFailure so the UI can offer Retry.
pub async fn promote_entry(ctx: AppContext, source: VirtualPath) -> PromoteState {
    let bundle_root = VirtualPath::root();
    let mempool = mempool_root();

    let source_exists = ctx.view_global_fs.with(|fs| fs.exists(&source));
    let target_exists_path = match promote_target_path(&source) {
        Ok(t) => t,
        Err(err) => {
            return PromoteState::Failed {
                source,
                error: humanize_promote_error(&err),
            };
        }
    };
    let target_exists = ctx
        .view_global_fs
        .with(|fs| fs.exists(&target_exists_path));
    let bundle_backend = ctx.backend_for_path(&bundle_root);
    let mempool_backend = ctx.backend_for_path(&mempool);
    let token = github_token_for_commit();

    let target = match preflight_promote_paths(
        &source,
        source_exists,
        target_exists,
        bundle_backend.is_some(),
        mempool_backend.is_some(),
        token.is_some(),
    ) {
        Ok(t) => t,
        Err(err) => {
            return PromoteState::Failed {
                source,
                error: humanize_promote_error(&err),
            };
        }
    };
    let bundle_backend = bundle_backend.expect("preflight ensured present");
    let mempool_backend = mempool_backend.expect("preflight ensured present");
    let token = token.expect("preflight ensured present");

    let body = match ctx.read_text(&source).await {
        Ok(body) => body,
        Err(error) => {
            return PromoteState::Failed {
                source,
                error: humanize_promote_error(&PromoteError::BodyReadFailed(error.to_string())),
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

    let bundle_changes = build_bundle_add_change_set(&target, &body);
    let bundle_expected_head = ctx.remote_head_for_path(&bundle_root);
    let bundle_outcome = match commit_backend(
        bundle_backend,
        bundle_root.clone(),
        bundle_changes,
        messages.bundle_add.clone(),
        bundle_expected_head,
        Some(token.clone()),
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            return PromoteState::Failed {
                source,
                error: error.to_string(),
            };
        }
    };
    apply_commit_outcome(&ctx, &bundle_root, &bundle_outcome).await;

    match commit_mempool_drop(
        &ctx,
        &source,
        &mempool_backend,
        &messages.mempool_drop,
        &token,
    )
    .await
    {
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

/// Replay only the mempool-drop arm. Used by the modal's Retry button after
/// a partial failure.
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

async fn commit_mempool_drop(
    ctx: &AppContext,
    source: &VirtualPath,
    backend: &Arc<dyn StorageBackend>,
    message: &str,
    token: &str,
) -> Result<CommitOutcome, String> {
    let mempool = mempool_root();
    let changes = build_mempool_drop_change_set(source);
    let expected_head = ctx.remote_head_for_path(&mempool);
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

async fn reload_and_apply(ctx: &AppContext) {
    match reload_runtime().await {
        Ok(load) => ctx.apply_runtime_load(load),
        Err(error) => leptos::logging::warn!(
            "promote: runtime reload after commit failed: {error}"
        ),
    }
}

pub fn humanize_promote_error(err: &PromoteError) -> String {
    match err {
        PromoteError::SourceNotInMempool(p) => format!("{p} is not under /mempool"),
        PromoteError::MempoolEntryMissing(p) => {
            format!("mempool entry {} no longer exists", p.as_str())
        }
        PromoteError::BundleTargetCollision(p) => format!(
            "{} already exists in the canonical chain — cannot promote without overwriting",
            p.as_str()
        ),
        PromoteError::BackendMissingFor(p) => {
            format!("no backend configured for {}", p.as_str())
        }
        PromoteError::TokenMissing => "missing GitHub token".to_string(),
        PromoteError::BodyReadFailed(s) => format!("failed to read mempool body: {s}"),
        PromoteError::BundleCommitFailed(s) | PromoteError::MempoolCommitFailed(s) => s.clone(),
    }
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
        assert!(matches!(entry.change, ChangeType::DeleteFile));
    }

    #[test]
    fn preflight_happy_path_returns_target() {
        let target = preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            true,
            false,
            true,
            true,
            true,
        )
        .unwrap();
        assert_eq!(target, p("/writing/foo.md"));
    }

    #[test]
    fn preflight_flags_missing_source() {
        assert!(matches!(
            preflight_promote_paths(
                &p("/mempool/writing/foo.md"),
                false,
                false,
                true,
                true,
                true
            ),
            Err(PromoteError::MempoolEntryMissing(_))
        ));
    }

    #[test]
    fn preflight_flags_target_collision() {
        match preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            true,
            true,
            true,
            true,
            true,
        ) {
            Err(PromoteError::BundleTargetCollision(path)) => {
                assert!(path.as_str().ends_with("writing/foo.md"));
            }
            other => panic!("expected BundleTargetCollision, got {other:?}"),
        }
    }

    #[test]
    fn preflight_flags_missing_bundle_backend() {
        assert!(matches!(
            preflight_promote_paths(
                &p("/mempool/writing/foo.md"),
                true,
                false,
                false,
                true,
                true
            ),
            Err(PromoteError::BackendMissingFor(_))
        ));
    }

    #[test]
    fn preflight_flags_missing_mempool_backend() {
        assert!(matches!(
            preflight_promote_paths(
                &p("/mempool/writing/foo.md"),
                true,
                false,
                true,
                false,
                true
            ),
            Err(PromoteError::BackendMissingFor(_))
        ));
    }

    #[test]
    fn mount_id_fallback_handles_root_and_nested() {
        assert_eq!(mount_id_fallback(&VirtualPath::root()), "~");
        assert_eq!(
            mount_id_fallback(&p("/mempool")),
            "mempool"
        );
        assert_eq!(
            mount_id_fallback(&p("/db/notes")),
            "db:notes"
        );
    }

    #[test]
    fn preflight_flags_missing_token() {
        assert!(matches!(
            preflight_promote_paths(
                &p("/mempool/writing/foo.md"),
                true,
                false,
                true,
                true,
                false
            ),
            Err(PromoteError::TokenMissing)
        ));
    }
}
