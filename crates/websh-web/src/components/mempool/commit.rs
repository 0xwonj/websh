//! Async commit handlers for mempool authoring (browser write path).
//!
//! `save_raw` is the single browser-side mempool write path. It serves
//! both reader-page New (`is_new=true`) and Edit (`is_new=false`).
//! Validation and frontmatter parsing are advisory: the page calls
//! `derive_new_path` for new drafts; existing edits trust the user's
//! typed bytes. The manifest's authored + derived + mempool block are
//! always recomputed from the bytes via
//! `build_mempool_manifest_state`, so a status edit lands in the
//! manifest without going through compose's structured form.

use leptos::prelude::*;

use crate::app::AppContext;
use websh_core::domain::changes::{ChangeSet, ChangeType};
use websh_core::runtime::commit_backend;
use websh_core::runtime::state::github_token_for_commit;
use websh_core::storage::CommitOutcome;
use websh_core::mempool::manifest_entry::{MempoolManifestState, build_mempool_manifest_state};
use websh_core::mempool::path::mempool_root;
use websh_core::domain::{RuntimeMount, VirtualPath};

/// Save raw markdown bytes (frontmatter included) to the mempool repo.
///
/// `is_new` controls whether the change emitted is `CreateFile` (new
/// entry) or `UpdateFile` (in-place edit). Both branches recompute the
/// manifest's authored + derived + mempool block from the bytes via
/// `build_mempool_manifest_state` — so a status edit, frontmatter title
/// change, or any other authored mutation lands in the manifest without
/// going through a structured compose form.
pub async fn save_raw(
    ctx: AppContext,
    path: VirtualPath,
    body: String,
    message: String,
    is_new: bool,
) -> Result<(), String> {
    if is_new {
        let collides = ctx.view_global_fs.with_untracked(|fs| fs.exists(&path));
        if collides {
            return Err(format!(
                "draft already exists at {} — pick a different slug",
                path.as_str()
            ));
        }
    }

    let MempoolManifestState { meta, extensions } = build_mempool_manifest_state(&body, &path);

    let root = mempool_root();
    let backend = ctx.backend_for_mount_root(root).ok_or_else(|| {
        "mempool mount is not registered — check that \
         content/.websh/mounts/mempool.mount.json exists and \
         content/manifest.json is up to date"
            .to_string()
    })?;
    let token = github_token_for_commit()
        .ok_or_else(|| "missing GitHub token for mempool commit".to_string())?;
    let expected_head = ctx.remote_head_for_path(root);

    let mut changes = ChangeSet::new();
    let change = if is_new {
        ChangeType::CreateFile {
            content: body,
            meta,
            extensions,
        }
    } else {
        ChangeType::UpdateFile {
            content: body,
            meta: Some(meta),
            extensions: Some(extensions),
        }
    };
    changes.upsert(path, change);

    let outcome = commit_backend(
        backend,
        root.clone(),
        changes,
        message,
        expected_head,
        Some(token),
    )
    .await
    .map_err(|err| err.to_string())?;
    apply_commit_outcome(&ctx, root, &outcome).await;

    match websh_core::runtime::reload_runtime().await {
        Ok(load) => ctx.apply_runtime_load(load),
        Err(error) => {
            leptos::logging::warn!("mempool: runtime reload after save failed: {error}")
        }
    }
    Ok(())
}

/// Apply the post-commit bookkeeping after a successful UI-driven commit:
/// update `ctx.remote_heads` so subsequent `expected_head` lookups reflect
/// the just-committed OID, and persist the new HEAD to IDB so the next
/// session boots with it. Best-effort — an IDB write failure is logged but
/// does not poison the in-memory signal.
async fn apply_commit_outcome(ctx: &AppContext, mount_root: &VirtualPath, outcome: &CommitOutcome) {
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

    if let Ok(db) = websh_core::storage::idb::open_db().await
        && let Err(error) = websh_core::storage::idb::save_metadata(
            &db,
            &format!("remote_head.{storage_id}"),
            &outcome.new_head,
        )
        .await
    {
        leptos::logging::warn!(
            "mempool: persist remote_head for {} failed: {error}",
            mount_root.as_str()
        );
    }
}

fn mount_id_fallback(root: &VirtualPath) -> String {
    if root.is_root() {
        "~".to_string()
    } else {
        root.as_str().trim_start_matches('/').replace('/', ":")
    }
}
