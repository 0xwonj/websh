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
use crate::app::RuntimeServices;
use websh_core::domain::{ChangeSet, ChangeType, VirtualPath};
use websh_core::mempool::{MempoolManifestState, build_mempool_manifest_state, mempool_root};

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
    if !ctx.mount_is_loaded(root) {
        let reason = match ctx.mount_status_for(root) {
            Some(crate::runtime::MountLoadStatus::Loading { .. }) => {
                "mempool mount is still loading — try again in a moment".to_string()
            }
            Some(crate::runtime::MountLoadStatus::Failed { error, .. }) => {
                format!("mempool mount is unavailable — {error}")
            }
            Some(crate::runtime::MountLoadStatus::Loaded { .. }) => unreachable!(),
            None => "mempool mount is not loaded".to_string(),
        };
        return Err(reason);
    }
    if ctx.backend_for_mount_root(root).is_none() {
        return Err("mempool mount is not registered — check that \
         content/.websh/mounts/mempool.mount.json exists and \
         content/manifest.json is up to date"
            .to_string());
    }

    let services = RuntimeServices::new(ctx);
    let token = services
        .github_token_for_commit()
        .ok_or_else(|| "missing GitHub token for mempool commit".to_string())?;

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
    changes.upsert_at(path, change, crate::platform::current_timestamp());

    let outcome = services
        .commit_changes(root.clone(), changes, message, Some(token))
        .await?;
    services.record_commit_outcome(root, &outcome).await;

    match services.reload_runtime_mount(root.clone()).await {
        Ok(()) => {}
        Err(error) => {
            leptos::logging::warn!("mempool: runtime reload after save failed: {error}")
        }
    }
    Ok(())
}
