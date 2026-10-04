use std::fs;

use websh_site::ATTESTATIONS_PATH;

use crate::support::{cli, temp_dir};

#[test]
fn content_manifest_generates_manifest_without_attestation() {
    let root = temp_dir("content-manifest");
    fs::create_dir_all(root.join("content/writing")).unwrap();
    fs::create_dir_all(root.join("content/talks")).unwrap();
    fs::write(
        root.join("content/writing/hello.md"),
        "---\ntitle: Hello Manifest\ndate: 2026-04-20\ntags: [notes, websh]\n---\n# Ignored\nbody",
    )
    .unwrap();
    fs::write(root.join("content/talks/slides.pdf"), b"%PDF").unwrap();
    fs::write(
        root.join("content/talks/slides.meta.json"),
        r#"{"kind":"document","authored":{"title":"ZK Talk","date":"2026-04-24","tags":["talk","zk"]},"derived":{}}"#,
    )
    .unwrap();

    let output = cli(&root, &["content", "manifest"]);
    assert!(output.contains("sidecars refreshed"));
    assert!(!root.join(ATTESTATIONS_PATH).exists());

    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("content/manifest.json")).unwrap())
            .unwrap();
    let entries = manifest["entries"].as_array().unwrap();
    let file_entries: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| e["metadata"]["kind"] != "directory")
        .collect();
    assert_eq!(file_entries.len(), 2);

    let hello = file_entries
        .iter()
        .find(|e| e["path"] == "writing/hello.md")
        .unwrap();
    assert_eq!(hello["metadata"]["authored"]["title"], "Hello Manifest");
    assert_eq!(hello["metadata"]["authored"]["date"], "2026-04-20");

    let slides = file_entries
        .iter()
        .find(|e| e["path"] == "talks/slides.pdf")
        .unwrap();
    assert_eq!(slides["metadata"]["authored"]["title"], "ZK Talk");
    assert_eq!(slides["metadata"]["authored"]["date"], "2026-04-24");
    assert_eq!(slides["metadata"]["authored"]["tags"][0], "talk");
}
