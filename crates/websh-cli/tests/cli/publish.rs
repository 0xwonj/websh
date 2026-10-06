use crate::support::{cli, cli_fails, temp_dir};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn drafts_publish_frozen_commits_and_resume_a_failed_push_without_new_commits() {
    let root = temp_dir("publish-checkout");
    let remote = temp_dir("publish-origin");
    git(&remote, &["init", "--bare", "--initial-branch=main"]);
    git(&root, &["init", "--initial-branch=main"]);
    git(&root, &["config", "user.name", "Fixture"]);
    git(&root, &["config", "user.email", "fixture@example.test"]);
    git(&root, &["config", "commit.gpgsign", "false"]);
    git(
        &root,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    fs::write(root.join("README.md"), "Draft workspace\n").unwrap();
    fs::write(root.join(".gitignore"), ".env\n.websh/local/\n").unwrap();
    git(&root, &["add", "README.md", ".gitignore"]);
    git(&root, &["commit", "-m", "Initialize content workspace"]);
    git(&root, &["push", "origin", "main"]);
    fs::write(root.join(".env"), "INVALID DEPLOYMENT ENVIRONMENT\n").unwrap();
    fs::create_dir(root.join("writing")).unwrap();
    fs::write(
        root.join("writing/draft.md"),
        "---\ntitle: Draft\n---\nFirst body\n",
    )
    .unwrap();
    let hook = remote.join("hooks/pre-receive");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let args = ["mempool", "publish", root.to_str().unwrap()];
    cli_fails(&root, &args);
    let prepared_head = git(&root, &["rev-parse", "HEAD"]);
    assert_eq!(git(&root, &["rev-list", "--count", "HEAD"]), "3");
    assert_eq!(git(&root, &["status", "--porcelain"]), "");
    let pointer: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("current.json")).unwrap()).unwrap();
    let snapshot = pointer["commit"].as_str().unwrap();
    assert_eq!(snapshot, git(&root, &["rev-parse", "HEAD^"]));
    assert_eq!(
        git(&root, &["show", &format!("{snapshot}:writing/draft.md")]),
        "---\ntitle: Draft\n---\nFirst body"
    );
    assert!(
        git(&root, &["ls-tree", "-r", "--name-only", snapshot])
            .lines()
            .all(|path| path != ".env")
    );
    fs::remove_file(&hook).unwrap();
    cli(&root, &args);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), prepared_head);
    assert_eq!(
        git(&remote, &["rev-parse", "refs/heads/main"]),
        prepared_head
    );
    cli(&root, &args);
    assert_eq!(git(&root, &["rev-parse", "HEAD"]), prepared_head);
    fs::write(
        root.join("writing/draft.md"),
        "---\ntitle: Draft\n---\nSecond body\n",
    )
    .unwrap();
    cli(&root, &args);
    assert_eq!(git(&root, &["rev-list", "--count", "HEAD"]), "5");
    assert_eq!(
        git(&root, &["show", &format!("{snapshot}:writing/draft.md")]),
        "---\ntitle: Draft\n---\nFirst body"
    );
    fs::write(root.join("private.txt"), "must not publish").unwrap();
    cli_fails(&root, &args);
    assert_eq!(git(&root, &["rev-list", "--count", "HEAD"]), "5");
    fs::remove_file(root.join("private.txt")).unwrap();
    fs::write(
        root.join("writing/draft.md"),
        "---\ntitle: Draft\n---\nThird body\n",
    )
    .unwrap();
    git(&root, &["add", "writing/draft.md"]);
    cli_fails(&root, &args);
    assert_eq!(git(&root, &["rev-list", "--count", "HEAD"]), "5");
}
