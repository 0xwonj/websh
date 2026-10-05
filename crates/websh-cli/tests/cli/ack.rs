use std::fs;

use websh_core::crypto::ack::{ACK_LOCAL_SOURCE_PATH, AckArtifact};
use websh_site::ACK_ARTIFACT_PATH;

use crate::support::{cli, cli_fails, temp_dir};

#[test]
fn ack_exports_receipts_for_the_published_commitment_without_a_cache() {
    let root = temp_dir("ack");
    fs::create_dir(root.join("content")).unwrap();
    cli(&root, &["ack", "add", "coffee", "--visibility", "public"]);
    cli(
        &root,
        &["ack", "add", "익명 리뷰어", "--visibility", "private"],
    );
    assert_eq!(
        cli(&root, &["ack", "list"]),
        "public\tcoffee\nprivate\t익명 리뷰어\n"
    );

    let artifact_body = fs::read_to_string(root.join(ACK_ARTIFACT_PATH)).unwrap();
    assert!(!artifact_body.contains("익명 리뷰어"));
    serde_json::from_str::<AckArtifact>(&artifact_body)
        .unwrap()
        .validate()
        .unwrap();
    assert_eq!(
        fs::read_dir(root.join(".websh/local/crypto"))
            .unwrap()
            .count(),
        1
    );

    let receipt = root.join("proof.json");
    let file = receipt.to_str().unwrap();
    cli(&root, &["ack", "receipt", "익명 리뷰어", "--out", file]);
    let snapshot = fs::read(&receipt).unwrap();
    cli(&root, &["ack", "verify", file]);

    cli(&root, &["ack", "remove", "coffee"]);
    assert_eq!(fs::read(&receipt).unwrap(), snapshot);
    cli_fails(&root, &["ack", "verify", file]);
    cli(&root, &["ack", "receipt", "익명 리뷰어", "--out", file]);
    cli(&root, &["ack", "verify", file]);

    // A public checkout can verify a proof, but cannot replace a commitment
    // by implicitly creating an empty author source.
    fs::remove_file(root.join(ACK_LOCAL_SOURCE_PATH)).unwrap();
    cli(&root, &["ack", "verify", file]);
    let published = fs::read(root.join(ACK_ARTIFACT_PATH)).unwrap();
    cli_fails(&root, &["ack", "add", "someone", "--visibility", "public"]);
    assert_eq!(fs::read(root.join(ACK_ARTIFACT_PATH)).unwrap(), published);
}

#[test]
fn ack_rejects_invalid_updates_before_writing_and_stale_receipt_exports() {
    let root = temp_dir("ack-validation");
    fs::create_dir(root.join("content")).unwrap();
    cli_fails(&root, &["ack", "add", "   ", "--visibility", "public"]);
    assert!(!root.join(ACK_LOCAL_SOURCE_PATH).exists());
    assert!(!root.join(ACK_ARTIFACT_PATH).exists());

    cli(&root, &["ack", "add", "Alice", "--visibility", "private"]);
    let source = fs::read(root.join(ACK_LOCAL_SOURCE_PATH)).unwrap();
    let artifact = fs::read(root.join(ACK_ARTIFACT_PATH)).unwrap();
    for name in [" ", " ALICE "] {
        cli_fails(&root, &["ack", "add", name, "--visibility", "public"]);
        assert_eq!(fs::read(root.join(ACK_LOCAL_SOURCE_PATH)).unwrap(), source);
        assert_eq!(fs::read(root.join(ACK_ARTIFACT_PATH)).unwrap(), artifact);
    }

    let mut edited: serde_json::Value = serde_json::from_slice(&source).unwrap();
    edited["entries"][0]["name"] = "Bob".into();
    fs::write(
        root.join(ACK_LOCAL_SOURCE_PATH),
        serde_json::to_vec(&edited).unwrap(),
    )
    .unwrap();
    let receipt = root.join("proof.json");
    cli_fails(
        &root,
        &["ack", "receipt", "Bob", "--out", receipt.to_str().unwrap()],
    );
    assert!(!receipt.exists());
    assert_eq!(fs::read(root.join(ACK_ARTIFACT_PATH)).unwrap(), artifact);
}
