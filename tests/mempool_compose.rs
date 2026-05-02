//! Integration tests for pure mempool helpers.

use websh::mempool::{
    ComposeError, ComposeForm, ComposePayload, MempoolManifestState, build_mempool_manifest_state,
    form_to_payload, parse_mempool_frontmatter, serialize_mempool_file, slug_from_title,
    validate_form,
};
use websh::models::{MempoolStatus, VirtualPath};

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
fn slug_from_title_matches_kebab_case_intuition() {
    assert_eq!(slug_from_title("On Writing Slow"), "on-writing-slow");
    assert_eq!(slug_from_title("Hello, World!"), "hello-world");
    assert_eq!(slug_from_title(""), "untitled");
}

#[test]
fn serialize_then_parse_roundtrips_required_fields() {
    let form = sample_form();
    let payload = form_to_payload(&form);
    let body = serialize_mempool_file(&payload);

    let parsed = parse_mempool_frontmatter(&body).expect("frontmatter parses");
    assert_eq!(parsed.title.as_deref(), Some("On writing slow"));
    assert_eq!(parsed.status.as_deref(), Some("draft"));
    assert_eq!(parsed.modified.as_deref(), Some("2026-04-28"));
    assert_eq!(parsed.priority.as_deref(), Some("med"));
    assert_eq!(parsed.tags, vec!["essay".to_string(), "slow".to_string()]);
}

#[test]
fn serialize_omits_optional_fields_when_unset() {
    let payload = ComposePayload {
        title: "minimal".into(),
        status: "draft".into(),
        modified: "2026-04-28".into(),
        priority: None,
        tags: vec![],
        body: "body\n".into(),
    };
    let body = serialize_mempool_file(&payload);
    assert!(!body.contains("priority"));
    assert!(!body.contains("tags"));
    assert!(body.starts_with("---\n"));
}

#[test]
fn validation_flags_known_bad_inputs() {
    let mut form = sample_form();
    form.title = "   ".into();
    form.status = "published".into();
    form.modified = "April 28".into();
    form.slug = "Bad Slug".into();
    form.category = "fiction".into();

    let errs = validate_form(&form);
    assert!(errs.contains(&ComposeError::TitleEmpty));
    assert!(errs.contains(&ComposeError::StatusUnknown));
    assert!(errs.contains(&ComposeError::ModifiedNotIso));
    assert!(errs.contains(&ComposeError::SlugInvalid));
    assert!(errs.contains(&ComposeError::CategoryUnknown));
}

#[test]
fn validation_passes_minimal_valid_form() {
    let form = sample_form();
    assert!(validate_form(&form).is_empty());
}

#[test]
fn validation_rejects_title_chars_that_break_yaml_roundtrip() {
    let mut form = sample_form();
    form.title = "He said \"hi\"".into();
    let errs = validate_form(&form);
    assert!(errs.contains(&ComposeError::TitleHasReservedChars));
}

#[test]
fn validation_rejects_priority_outside_known_set() {
    let mut form = sample_form();
    form.priority = Some("urgent".into());
    let errs = validate_form(&form);
    assert!(errs.contains(&ComposeError::PriorityUnknown));
}

#[test]
fn validation_rejects_tags_that_break_inline_list() {
    let mut form = sample_form();
    form.tags = vec!["safe".into(), "bad,comma".into()];
    let errs = validate_form(&form);
    assert!(errs.contains(&ComposeError::TagHasReservedChars));
}

#[test]
fn manifest_state_for_new_entry_populates_authored_and_derived() {
    let form = sample_form();
    let body = serialize_mempool_file(&form_to_payload(&form));
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
