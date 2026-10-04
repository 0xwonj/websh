use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::support::{cli_with_env, temp_dir};

#[test]
fn deploy_builds_and_uploads_the_selected_bundle_with_explicit_environment() {
    let root = temp_dir("deploy");
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    for (name, script) in [
        (
            "trunk",
            r#"#!/bin/sh
set -eu
printf 'trunk %s\n' "$*" >> calls
test "$WEBSH_NO_SIGN" = 1
case "$1" in
  clean) test "$2" = --dist; test "$3" = dist-preview ;;
  build) test "$2" = --release; test "$3" = --locked; test "$4" = --dist; test "$5" = dist-preview
    /bin/mkdir -p "$5"; printf 'built' > "$5/index.html" ;;
  *) exit 1 ;;
esac
"#,
        ),
        (
            "pinata",
            r#"#!/bin/sh
set -eu
printf 'pinata %s\n' "$*" >> calls
test "$PINATA_JWT" = test-only
test "$1" = upload; test "$2" = dist-preview; test "$3" = --name; test "$4" = fixture
test -f dist-preview/index.html
printf '{"cid":"bafyfixture"}\n'
"#,
        ),
    ] {
        let path = bin.join(name);
        fs::write(&path, script).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::write(root.join(".env"), "PINATA_JWT=test-only\n").unwrap();
    let path = bin.to_string_lossy().into_owned();
    let args = [
        "deploy",
        "pinata",
        "--dist-dir",
        "dist-preview",
        "--name",
        "fixture",
        "--no-sign",
    ];
    cli_with_env(&root, &args, &[("PATH", &path)]);
    assert_eq!(
        fs::read_to_string(root.join("calls")).unwrap(),
        "trunk clean --dist dist-preview\ntrunk build --release --locked --dist dist-preview\npinata upload dist-preview --name fixture\n"
    );
    assert_eq!(
        fs::read_to_string(root.join(".last-cid")).unwrap(),
        "bafyfixture\n"
    );

    fs::write(root.join("calls"), "").unwrap();
    let mut args = args.to_vec();
    args.push("--no-build");
    cli_with_env(&root, &args, &[("PATH", &path)]);
    assert_eq!(
        fs::read_to_string(root.join("calls")).unwrap(),
        "pinata upload dist-preview --name fixture\n"
    );
}

#[test]
fn deploy_rejects_source_paths_and_symlinks_before_running_tools() {
    use std::os::unix::fs::symlink;
    use std::process::Command;

    let root = temp_dir("deploy-output-boundary");
    fs::create_dir(root.join("content")).unwrap();
    fs::write(root.join("content/source.md"), "preserved").unwrap();
    symlink(root.join("content"), root.join("dist-linked")).unwrap();
    fs::write(root.join("dist-file"), "preserved").unwrap();
    let absolute = root.join("dist-preview");
    for directory in [
        ".",
        "..",
        "content",
        "dist/..",
        "dist/nested",
        "dist-linked",
        "dist-file",
        "dist-",
        "dist-bad.name",
        absolute.to_str().unwrap(),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_websh-cli"))
            .arg("--root")
            .arg(&*root)
            .args(["deploy", "pinata", "--dist-dir", directory])
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!output.status.success(), "accepted {directory}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(
            error.contains("--dist-dir must be") || error.contains("deployment output must be"),
            "{error}"
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("content/source.md")).unwrap(),
        "preserved"
    );
    assert_eq!(
        fs::read_to_string(root.join("dist-file")).unwrap(),
        "preserved"
    );
}
