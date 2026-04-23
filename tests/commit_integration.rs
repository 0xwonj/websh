//! End-to-end: staged changes → backend-private manifest regeneration.

use websh::core::changes::{ChangeSet, ChangeType};
use websh::core::storage::{MockBackend, ScannedSubtree, StorageBackend};
use websh::models::{FileMetadata, VirtualPath};

#[tokio::test(flavor = "current_thread")]
async fn commit_path_records_staged_paths_plus_manifest() {
    let mut cs = ChangeSet::new();
    let site_root = VirtualPath::from_absolute("/site").unwrap();
    let p = site_root.join("a.md");
    cs.upsert(
        p.clone(),
        ChangeType::CreateFile {
            content: "hello".into(),
            meta: FileMetadata::default(),
        },
    );

    let backend = MockBackend::with_success(ScannedSubtree::default(), "sha-new");
    let outcome = backend.commit(&cs, "test", Some("sha-old")).await.unwrap();
    assert_eq!(outcome.new_head, "sha-new");

    let calls = backend.commit_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].message, "test");
    let paths: Vec<&str> = calls[0].paths.iter().map(|p| p.as_str()).collect();
    assert!(paths.contains(&"/site/a.md"));
    assert!(paths.contains(&"/manifest.json"));
    assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
}
