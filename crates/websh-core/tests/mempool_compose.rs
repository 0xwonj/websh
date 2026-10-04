//! Integration tests for pure mempool helpers.

use websh_core::domain::{MempoolStatus, VirtualPath};
use websh_core::mempool::{
    ComposeForm, MempoolManifestState, build_mempool_manifest_state, form_to_payload,
    parse_mempool_frontmatter, serialize_mempool_file, validate_form,
};

fn sample_form() -> ComposeForm {
    ComposeForm {
        title: "On writing slow".into(),
        category: "writing".into(),
        slug: "on-writing-slow".into(),
        status: "draft".into(),
        modified: "2026-04-28".into(),
        priority: Some("med".into()),
        tags: vec!["essay".into(), "slow".into()],
        body: "# heading\n\nFirst paragraph.\n".into(),
    }
}

#[test]
fn valid_form_round_trips_into_a_manifest_entry() {
    let form = sample_form();
    assert!(validate_form(&form).is_empty());
    let payload = form_to_payload(&form);
    let body = serialize_mempool_file(&payload);

    let parsed = parse_mempool_frontmatter(&body).expect("frontmatter parses");
    assert_eq!(parsed.title.as_deref(), Some("On writing slow"));
    assert_eq!(parsed.status.as_deref(), Some("draft"));
    assert_eq!(parsed.modified.as_deref(), Some("2026-04-28"));
    assert_eq!(parsed.priority.as_deref(), Some("med"));
    assert_eq!(parsed.tags, vec!["essay".to_string(), "slow".to_string()]);

    let path = VirtualPath::from_absolute("/mempool/writing/on-writing-slow.md").unwrap();
    let MempoolManifestState { meta, extensions } = build_mempool_manifest_state(&body, &path);

    assert_eq!(meta.authored.title.as_deref(), Some("On writing slow"));
    assert_eq!(meta.authored.date.as_deref(), Some("2026-04-28"));
    assert_eq!(
        meta.authored.tags.as_deref(),
        Some(&["essay".to_string(), "slow".to_string()][..])
    );
    assert_eq!(meta.derived.size_bytes, Some(body.len() as u64));
    assert!(meta.derived.word_count.is_some());
    assert!(
        meta.derived
            .content_sha256
            .as_deref()
            .is_some_and(|s| s.starts_with("0x"))
    );

    let mp = extensions.mempool.expect("mempool block populated");
    assert_eq!(mp.status, MempoolStatus::Draft);
    assert_eq!(mp.category.as_deref(), Some("writing"));
}
