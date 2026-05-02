//! WASM-only IDB round-trip. Run with:
//!   wasm-pack test --chrome --headless
//!
//! Gated behind #[cfg(target_arch = "wasm32")] so it's skipped in `cargo test`.

#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

use websh::core::changes::{ChangeSet, ChangeType};
use websh::core::storage::idb::{load_draft, open_db, save_draft};
use websh::models::{NodeMetadata, VirtualPath};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn save_then_load_draft_preserves_content() {
    let db = open_db().await.expect("open db");
    let mut cs = ChangeSet::new();
    let p = VirtualPath::from_absolute("/rt.md").unwrap();
    cs.upsert(
        p.clone(),
        ChangeType::CreateFile {
            content: "roundtrip".into(),
            meta: NodeMetadata::default(),
        },
    );

    save_draft(&db, "test-mount", &cs).await.expect("save");
    let loaded = load_draft(&db, "test-mount")
        .await
        .expect("load")
        .expect("exists");

    let entry = loaded.get(&p).expect("entry present");
    match &entry.change {
        ChangeType::CreateFile { content, .. } => assert_eq!(content, "roundtrip"),
        _ => panic!("wrong variant"),
    }
}
