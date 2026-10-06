use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::process::Command;

use crate::support::{cli, temp_dir, write_site_fixture};

#[test]
fn deploy_rejects_unbuilt_and_fixture_bundles_before_loading_credentials_or_running_tools() {
    let root = temp_dir("deploy-unsigned");
    write_site_fixture(&root);
    cli(&root, &["sync"]);
    fs::create_dir(root.join("dist")).unwrap();
    fs::write(root.join("dist/index.html"), "prebuilt").unwrap();
    fs::create_dir_all(root.join(".websh/local/deploy")).unwrap();
    fs::write(
        root.join(".websh/local/deploy/release.json"),
        "previous deployment\n",
    )
    .unwrap();
    let bin = root.join("bin");
    fs::create_dir(&bin).unwrap();
    let tool = bin.join("pinata");
    fs::write(
        &tool,
        "#!/bin/sh\nprintf 'unexpected call' > calls\nexit 99\n",
    )
    .unwrap();
    fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_websh-cli"))
        .arg("--root")
        .arg(&*root)
        .arg("deploy")
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("dist has no built application entry point"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("calls").exists());
    assert_eq!(
        fs::read_to_string(root.join("dist/index.html")).unwrap(),
        "prebuilt"
    );
    assert_eq!(
        fs::read_to_string(root.join(".websh/local/deploy/release.json")).unwrap(),
        "previous deployment\n"
    );

    fs::create_dir_all(root.join("dist/assets/crypto")).unwrap();
    fs::write(
        root.join("dist/index.html"),
        "<html><script src='app.js'></script></html>",
    )
    .unwrap();
    fs::write(root.join("dist/app.js"), "void 0;").unwrap();
    fs::write(root.join("dist/app.wasm"), b"\0asm\x01\0\0\0\0").unwrap();
    fs::write(
        root.join("dist/assets/crypto/site.asc"),
        include_bytes!("../../../../tests/fixtures/pgp/public.asc"),
    )
    .unwrap();
    fs::write(root.join(".env"), "INVALID DEPLOYMENT ENVIRONMENT").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_websh-cli"))
        .arg("--root")
        .arg(&*root)
        .arg("deploy")
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("pinned production identity"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!root.join("calls").exists());
}

#[test]
fn deploy_rejects_missing_and_linked_output_before_running_tools() {
    let root = temp_dir("deploy-output-boundary");
    write_site_fixture(&root);
    fs::write(root.join("content/source.md"), "preserved").unwrap();
    for linked in [false, true] {
        if linked {
            symlink(root.join("content"), root.join("dist")).unwrap();
        }
        let output = Command::new(env!("CARGO_BIN_EXE_websh-cli"))
            .arg("--root")
            .arg(&*root)
            .arg("deploy")
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("dist must be"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read_to_string(root.join("content/source.md")).unwrap(),
        "preserved"
    );
}
