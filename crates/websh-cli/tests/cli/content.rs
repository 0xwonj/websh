use std::fs;

use websh_site::ATTESTATIONS_PATH;

use crate::support::{cli, cli_fails, temp_dir, write_site_fixture};

#[test]
fn sync_rebuilds_current_sources_and_check_reports_drift_without_writes() {
    let root = temp_dir("content-sync");
    write_site_fixture(&root);
    fs::create_dir_all(root.join("content/writing")).unwrap();
    let source = root.join("content/writing/note.md");
    fs::write(&source, "---\ntitle: First title\n---\nbody\n").unwrap();
    cli(&root, &["sync"]);
    cli(&root, &["check"]);

    let outputs = [
        "content/manifest.json",
        "content/.websh/ledger.json",
        ATTESTATIONS_PATH,
    ];
    let before: Vec<_> = outputs
        .iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .collect();
    assert!(!root.join("content/writing/note.meta.json").exists());
    fs::write(&source, "body\n").unwrap();
    cli_fails(&root, &["check"]);
    for (path, bytes) in outputs.iter().zip(&before) {
        assert_eq!(&fs::read(root.join(path)).unwrap(), bytes);
    }

    cli(&root, &["sync"]);
    cli(&root, &["check"]);
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("content/manifest.json")).unwrap()).unwrap();
    let note = manifest["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == "writing/note.md")
        .unwrap();
    assert!(note["metadata"]["authored"].as_object().unwrap().is_empty());
    assert_eq!(note["metadata"]["derived"]["title"], "note");
    let synced: Vec<_> = outputs
        .iter()
        .map(|path| fs::read(root.join(path)).unwrap())
        .collect();
    cli(&root, &["sync"]);
    for (path, bytes) in outputs.iter().zip(synced) {
        assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
    }
}
