use std::fs;

use websh_core::crypto::ack::{ACK_RECEIPTS_DIR, AckArtifact, slugify_name};
use websh_site::ACK_ARTIFACT_PATH;

use crate::support::{cli, cli_fails, temp_dir};

#[test]
fn builds_ack_artifact_and_private_receipt() {
    let root = temp_dir("ack");
    cli(&root, &["crypto", "ack", "init"]);
    cli(&root, &["crypto", "ack", "add", "--public", "coffee"]);
    cli(&root, &["crypto", "ack", "verify", "--name", "coffee"]);

    cli(
        &root,
        &["crypto", "ack", "add", "--private", "anonymous reviewer"],
    );

    let artifact_body = fs::read_to_string(root.join(ACK_ARTIFACT_PATH)).unwrap();
    assert!(!artifact_body.contains("anonymous reviewer"));
    let artifact: AckArtifact = serde_json::from_str(&artifact_body).unwrap();
    artifact.validate().unwrap();

    let receipt = root
        .join(ACK_RECEIPTS_DIR)
        .join(format!("{}.json", slugify_name("anonymous reviewer")));
    assert!(receipt.exists());
    cli(
        &root,
        &[
            "crypto",
            "ack",
            "verify",
            "--receipt",
            receipt.to_str().unwrap(),
        ],
    );

    cli(&root, &["crypto", "ack", "build"]);
    cli(
        &root,
        &["crypto", "ack", "receipt", "--name", "anonymous reviewer"],
    );

    cli(&root, &["crypto", "ack", "remove", "anonymous reviewer"]);
    assert!(!receipt.exists());

    cli(&root, &["crypto", "ack", "remove", "coffee"]);
    cli_fails(&root, &["crypto", "ack", "verify", "--name", "coffee"]);

    let artifact_body = fs::read_to_string(root.join(ACK_ARTIFACT_PATH)).unwrap();
    let artifact: AckArtifact = serde_json::from_str(&artifact_body).unwrap();
    artifact.validate().unwrap();
}

#[test]
fn ack_handles_unicode_private_receipt_names() {
    let root = temp_dir("ack-unicode");
    cli(&root, &["crypto", "ack", "init"]);
    cli(&root, &["crypto", "ack", "add", "--private", "익명 리뷰어"]);

    let receipt = root
        .join(ACK_RECEIPTS_DIR)
        .join(format!("{}.json", slugify_name("익명 리뷰어")));
    assert!(receipt.exists());
    assert!(receipt.file_name().unwrap().to_string_lossy().is_ascii());

    cli(
        &root,
        &[
            "crypto",
            "ack",
            "verify",
            "--receipt",
            receipt.to_str().unwrap(),
        ],
    );
    cli(&root, &["crypto", "ack", "remove", "익명 리뷰어"]);
    assert!(!receipt.exists());
}
