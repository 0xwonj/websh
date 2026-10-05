use std::fs;
use std::process::Command;

use websh_core::domain::{ContentManifestDocument, Fields, MempoolStatus};

use crate::support::{cli_fails, cli_with_env, temp_dir};

const DRAFT: &str = "---\r\ntitle: 'A title: with punctuation'\r\nstatus: review\r\nmodified: 2026-10-05\r\ntags:\r\n  - 'a, b'\r\n  - rust\r\n---\r\n\r\n# Draft\r\nBody remains intact.\r\n";

#[test]
fn sync_uses_authored_yaml_and_preserves_the_last_manifest_on_validation_failure() {
    let checkout = temp_dir("mempool-sync");
    fs::create_dir(checkout.join("writing")).unwrap();
    fs::write(checkout.join("writing/example.md"), DRAFT).unwrap();
    let args = ["mempool", "sync", checkout.to_str().unwrap()];
    cli_with_env(&checkout, &args, &[("PATH", "")]);
    let encoded = fs::read(checkout.join("manifest.json")).unwrap();
    let document: ContentManifestDocument = serde_json::from_slice(&encoded).unwrap();
    let entry = &document.entries[0];
    assert_eq!(entry.path, "writing/example.md");
    assert_eq!(
        entry.metadata.authored.title.as_deref(),
        Some("A title: with punctuation")
    );
    assert_eq!(
        entry.metadata.authored.tags.as_ref().unwrap(),
        &["a, b", "rust"]
    );
    assert_eq!(entry.metadata.derived.size_bytes, Some(DRAFT.len() as u64));
    assert_eq!(entry.metadata.derived.word_count, Some(5));
    assert_eq!(
        entry.mempool.as_ref().unwrap().status,
        MempoolStatus::Review
    );
    cli_with_env(&checkout, &args, &[("PATH", "")]);
    assert_eq!(fs::read(checkout.join("manifest.json")).unwrap(), encoded);
    assert_eq!(
        fs::read_to_string(checkout.join("writing/example.md")).unwrap(),
        DRAFT
    );

    fs::write(
        checkout.join("writing/invalid.md"),
        "---\ntitle: Broken\nstatus: typo\n---\n",
    )
    .unwrap();
    cli_fails(&checkout, &args);
    assert_eq!(fs::read(checkout.join("manifest.json")).unwrap(), encoded);
}

#[test]
fn import_resolves_the_source_from_cwd_and_changes_only_new_canonical_source() {
    let project = temp_dir("mempool-import-project");
    let checkout = temp_dir("mempool-import-checkout");
    fs::create_dir(checkout.join("writing")).unwrap();
    fs::write(checkout.join("writing/example.md"), DRAFT).unwrap();
    fs::create_dir_all(project.join("content/writing")).unwrap();
    fs::create_dir(project.join(".git")).unwrap();
    let unchanged = [
        (".git/index", "untouched"),
        ("content/manifest.json", "untouched"),
        (
            "content/writing/_index.dir.json",
            r#"{"kind":"directory","authored":{"title":"Writing"}}"#,
        ),
    ];
    for (path, body) in unchanged {
        fs::write(project.join(path), body).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_websh-cli"))
        .current_dir(&checkout)
        .arg("--root")
        .arg(&*project)
        .args(["mempool", "import", "writing/example.md"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let canonical = fs::read_to_string(project.join("content/writing/example.md")).unwrap();
    let (yaml, body) = canonical
        .strip_prefix("---\n")
        .unwrap()
        .split_once("---\n")
        .unwrap();
    let fields: Fields = serde_norway::from_str(yaml).unwrap();
    assert_eq!(fields.title.as_deref(), Some("A title: with punctuation"));
    assert_eq!(fields.date.as_deref(), Some("2026-10-05"));
    assert_eq!(fields.tags.as_ref().unwrap(), &["a, b", "rust"]);
    assert_eq!(body, "\r\n# Draft\r\nBody remains intact.\r\n");
    assert_eq!(
        fs::read_to_string(checkout.join("writing/example.md")).unwrap(),
        DRAFT
    );
    for (path, body) in unchanged {
        assert_eq!(fs::read_to_string(project.join(path)).unwrap(), body);
    }
    assert!(!project.join("content/writing/example.meta.json").exists());
}

#[test]
fn import_rejects_invalid_drafts_destinations_and_overwrites_before_writing() {
    use std::os::unix::fs::symlink;

    let project = temp_dir("mempool-import-invalid");
    fs::create_dir(project.join("content")).unwrap();
    let checkout = temp_dir("mempool-import-invalid-source");
    let source = checkout.join("draft.md");
    let args = [
        "mempool",
        "import",
        source.to_str().unwrap(),
        "--to",
        "writing/example.md",
    ];
    for body in [
        "# No frontmatter\n",
        "---\ntitle: No closing fence\n",
        "---\ntitle: Unknown metadata\nunknown: value\n---\n",
        "---\ntitle: Broken date\nmodified: 2026-02-30\n---\n",
    ] {
        fs::write(&source, body).unwrap();
        cli_fails(&project, &args);
        assert!(
            fs::read_dir(project.join("content"))
                .unwrap()
                .next()
                .is_none()
        );
    }
    fs::write(&source, DRAFT).unwrap();
    cli_fails(
        &project,
        &[
            "mempool",
            "import",
            source.to_str().unwrap(),
            "--to",
            "../escape.md",
        ],
    );
    assert!(
        fs::read_dir(project.join("content"))
            .unwrap()
            .next()
            .is_none()
    );
    cli_with_env(&project, &args, &[("PATH", "")]);
    let destination = project.join("content/writing/example.md");
    fs::write(&destination, "existing authored content").unwrap();
    cli_fails(&project, &args);
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        "existing authored content"
    );

    fs::create_dir(project.join("content/projects")).unwrap();
    fs::write(
        project.join("content/projects/example.html"),
        "<p>Existing route</p>",
    )
    .unwrap();
    cli_fails(
        &project,
        &[
            "mempool",
            "import",
            source.to_str().unwrap(),
            "--to",
            "projects/example.md",
        ],
    );
    assert!(!project.join("content/projects/example.md").exists());

    symlink(&*checkout, project.join("content/papers")).unwrap();
    cli_fails(
        &project,
        &[
            "mempool",
            "import",
            source.to_str().unwrap(),
            "--to",
            "papers/example.md",
        ],
    );
    assert!(!checkout.join("example.md").exists());
}
