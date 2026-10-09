//! Shared ports and DTOs for edge adapters.

mod manifest;
mod storage;

pub use manifest::{
    ManifestPathError, ManifestSnapshotError, ManifestSnapshotResult, manifest_snapshot,
    parse_manifest_snapshot, serialize_manifest_snapshot,
};
pub use storage::{
    LocalBoxFuture, ScannedDirectory, ScannedFile, ScannedSubtree, StorageBackend,
    StorageBackendRef, StorageError, StorageResult,
};
