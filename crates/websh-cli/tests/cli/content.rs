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

    let outputs = ["content/manifest.json", ATTESTATIONS_PATH];
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

    fs::write(
        root.join("content/writing/attachment.pdf"),
        "version https://git-lfs.github.com/spec/v1\r\noid sha256:0000000000000000000000000000000000000000000000000000000000000000\r\nsize 123\r\n",
    )
    .unwrap();
    let previous = fs::read(root.join("content/manifest.json")).unwrap();
    cli_fails(&root, &["sync"]);
    assert_eq!(
        fs::read(root.join("content/manifest.json")).unwrap(),
        previous
    );
}

#[test]
fn manifest_projects_home_and_catalog_from_one_content_only_tree() {
    let root = temp_dir("content-projections");
    write_site_fixture(&root);
    fs::write(root.join("content/writing/note.txt"), "a public attachment").unwrap();
    fs::write(
        root.join("content/writing/note.txt.meta.json"),
        r#"{"title":"Attachment"}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("content/projects/group")).unwrap();
    fs::write(
        root.join("content/projects/group/_index.dir.json"),
        r#"{"kind":"directory","group":true,"authored":{"title":"Project"}}"#,
    )
    .unwrap();
    fs::write(
        root.join("content/projects/group/readme.md"),
        "Project body",
    )
    .unwrap();
    cli(&root, &["sync"]);
    let body = fs::read(root.join("content/manifest.json")).unwrap();
    let manifest = websh_core::publication::Manifest::from_bytes(&body).unwrap();
    let release = manifest.release.as_ref().unwrap();
    assert_eq!(release.sequence, 0);
    assert_eq!(release.issued_at, 0);
    assert_eq!(release.home.profile.name, "Fixture Author");
    assert_eq!(release.home.now.items[0].text, "fixture");
    assert_eq!(
        release.publications,
        vec!["projects/group", "writing/note.md", "writing/note.txt"]
    );
    assert!(!root.join("content/manifest.sig").exists());
    assert!(!root.join("assets").exists());
    for entry in manifest
        .entries
        .iter()
        .filter(|entry| !entry.metadata.kind.is_directory_like())
    {
        manifest
            .verify_file(
                &entry.path,
                &fs::read(root.join("content").join(&entry.path)).unwrap(),
            )
            .unwrap();
    }
    assert!(manifest.integrity("writing/note.txt.meta.json").is_ok());
    fs::write(
        root.join("content/.site/now.toml"),
        "[[items]]\ndate = \"2026-10-06\"\ntext = \"Updated Now\"\n",
    )
    .unwrap();
    cli(&root, &["sync"]);
    let changed = websh_core::publication::Manifest::from_bytes(
        &fs::read(root.join("content/manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        changed.release.as_ref().unwrap().home.now.items[0].text,
        "Updated Now"
    );
    assert_eq!(
        changed.integrity("writing/note.md").unwrap(),
        manifest.integrity("writing/note.md").unwrap()
    );
}
