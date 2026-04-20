//! End-to-end: staged changes → MockBackend records the commit call.
//! Simulates the dispatcher's logic inline (does not exercise Leptos).

use websh::core::VirtualFs;
use websh::core::changes::{ChangeSet, ChangeType};
use websh::core::merge::merge_view;
use websh::core::storage::{MockBackend, StorageBackend};
use websh::models::{FileMetadata, Manifest, VirtualPath};

#[tokio::test(flavor = "current_thread")]
async fn commit_path_records_staged_paths_plus_manifest() {
    let base = VirtualFs::empty();
    let mut cs = ChangeSet::new();
    let p = VirtualPath::from_absolute("/a.md").unwrap();
    cs.upsert(
        p.clone(),
        ChangeType::CreateFile {
            content: "hello".into(),
            meta: FileMetadata::default(),
        },
    );

    // Simulate the dispatcher: merge the working tree, then inject manifest.json.
    let merged = merge_view(&base, &cs);
    let manifest_body =
        serde_json::to_string_pretty(&merged.serialize_manifest()).unwrap();
    let mut with_manifest = cs.clone();
    let mpath = VirtualPath::from_absolute("/manifest.json").unwrap();
    with_manifest.upsert(
        mpath.clone(),
        ChangeType::UpdateFile {
            content: manifest_body,
            description: None,
        },
    );

    let backend = MockBackend::with_success(Manifest::default(), "sha-new");
    let outcome = backend
        .commit(&with_manifest, "test", Some("sha-old"))
        .await
        .unwrap();
    assert_eq!(outcome.new_head, "sha-new");

    let calls = backend.commit_calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].message, "test");
    let paths: Vec<&str> = calls[0].paths.iter().map(|p| p.as_str()).collect();
    assert!(paths.contains(&"/a.md"));
    assert!(paths.contains(&"/manifest.json"));
    assert_eq!(calls[0].expected_head.as_deref(), Some("sha-old"));
}
