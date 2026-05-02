//! End-to-end: staged changes → backend-private manifest regeneration.

use websh::core::changes::{ChangeSet, ChangeType};
use websh::core::runtime;
use websh::core::storage::{MockBackend, ScannedSubtree};
use websh::models::{EntryExtensions, NodeMetadata, VirtualPath};

#[tokio::test(flavor = "current_thread")]
async fn commit_path_records_staged_paths_and_merged_snapshot() {
    let mut cs = ChangeSet::new();
    let site_root = VirtualPath::root();
    let p = site_root.join("a.md");
    cs.upsert(
        p.clone(),
        ChangeType::CreateFile {
            content: "hello".into(),
            meta: NodeMetadata::default(),
            extensions: EntryExtensions::default(),
        },
    );

    let backend = std::sync::Arc::new(MockBackend::with_success(
        ScannedSubtree::default(),
        "sha-new",
    ));
    let outcome = runtime::commit_backend(
        backend.clone(),
        site_root,
        cs,
        "test".to_string(),
        Some("sha-old".to_string()),
        Some("qa-token".to_string()),
    )
    .await
    .unwrap();
    assert_eq!(outcome.new_head, "sha-new");

    let calls = backend.commit_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].message, "test");
    assert_eq!(calls[0].auth_token.as_deref(), Some("qa-token"));
    let paths: Vec<&str> = calls[0].paths.iter().map(|p| p.as_str()).collect();
    assert!(paths.contains(&"/a.md"));
    assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
    let snapshot_paths: Vec<_> = calls[0]
        .merged_snapshot
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    assert_eq!(snapshot_paths, vec!["a.md"]);
}
