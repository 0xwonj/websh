//! Browser persistence for runtime metadata.

use websh_core::domain::{RuntimeMount, VirtualPath};
use websh_core::ports::StorageResult;

use super::idb;

pub async fn persist_remote_head(storage_id: &str, head: &str) -> StorageResult<()> {
    let db = idb::open_db().await?;
    idb::save_metadata(&db, &format!("remote_head.{storage_id}"), head).await
}

pub async fn hydrate_remote_head(storage_id: &str) -> StorageResult<Option<String>> {
    let db = idb::open_db().await?;
    idb::load_metadata(&db, &format!("remote_head.{storage_id}")).await
}

pub fn storage_id_for_mount_root(mounts: &[RuntimeMount], root: &VirtualPath) -> String {
    mounts
        .iter()
        .find(|mount| &mount.root == root)
        .map(RuntimeMount::storage_id)
        .unwrap_or_else(|| fallback_storage_id(root))
}

fn fallback_storage_id(root: &VirtualPath) -> String {
    if root.is_root() {
        "~".to_string()
    } else {
        root.as_str().trim_start_matches('/').replace('/', ":")
    }
}
