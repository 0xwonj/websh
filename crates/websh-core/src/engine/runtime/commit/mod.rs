use crate::domain::{ChangeSet, VirtualPath};
use crate::ports::{CommitOutcome, StorageBackendRef, StorageResult};

mod delta;
mod prepare;
#[cfg(test)]
mod tests;

use prepare::prepare_commit;

pub async fn commit_backend(
    backend: StorageBackendRef,
    mount_root: VirtualPath,
    changes: ChangeSet,
    message: String,
    expected_head: Option<String>,
    auth_token: Option<String>,
) -> StorageResult<CommitOutcome> {
    let request = prepare_commit(
        &backend,
        &mount_root,
        &changes,
        message,
        expected_head,
        auth_token,
    )
    .await?;
    backend.commit(&request).await
}
