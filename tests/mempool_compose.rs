//! Integration tests for the mempool compose / edit flow. Exercises the
//! pure pieces — serialize roundtrip, target paths, validation, and
//! `ChangeSet` shape — without spinning up a backend or async runtime.

use websh::components::mempool::{
    ComposeError, ComposeForm, ComposeMode, ComposePayload, build_change_set, commit_message,
    derive_form_from_mode, form_to_payload, parse_mempool_frontmatter, save_path_for,
    serialize_mempool_file, slug_from_title, target_path, validate_form,
};
use websh::core::changes::ChangeType;
use websh::models::VirtualPath;

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
fn target_path_uses_mempool_namespace() {
    let form = sample_form();
    assert_eq!(
        target_path(&form).as_str(),
        "/mempool/writing/on-writing-slow.md"
    );
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
fn change_set_for_new_mode_emits_create_file_at_target_path() {
    let form = sample_form();
    let mode = ComposeMode::New {
        default_category: None,
    };
    let changes = build_change_set(&mode, &form);
    let entries: Vec<_> = changes.iter_all().collect();
    assert_eq!(entries.len(), 1);
    let (path, entry) = entries[0];
    assert_eq!(path.as_str(), "/mempool/writing/on-writing-slow.md");
    match &entry.change {
        ChangeType::CreateFile { content, .. } => {
            assert!(content.starts_with("---\n"));
            assert!(content.contains("title: \"On writing slow\""));
        }
        other => panic!("expected CreateFile, got {other:?}"),
    }
}

#[test]
fn change_set_for_edit_mode_emits_update_file_at_existing_path() {
    let form = sample_form();
    let existing = VirtualPath::from_absolute("/mempool/writing/already-here.md").unwrap();
    let mode = ComposeMode::Edit {
        path: existing.clone(),
        body: String::new(),
    };
    let changes = build_change_set(&mode, &form);
    let entries: Vec<_> = changes.iter_all().collect();
    assert_eq!(entries.len(), 1);
    let (path, entry) = entries[0];
    assert_eq!(path, &existing);
    assert!(matches!(&entry.change, ChangeType::UpdateFile { .. }));
}

#[test]
fn save_path_for_edit_keeps_existing_path_even_if_form_drifts() {
    let mut form = sample_form();
    form.category = "papers".into();
    form.slug = "renamed".into();
    let existing = VirtualPath::from_absolute("/mempool/writing/foo.md").unwrap();
    let mode = ComposeMode::Edit {
        path: existing.clone(),
        body: String::new(),
    };
    assert_eq!(save_path_for(&mode, &form), existing);
}

#[test]
fn commit_message_uses_relative_path_without_extension() {
    let form = sample_form();
    let new_mode = ComposeMode::New {
        default_category: None,
    };
    let edit_mode = ComposeMode::Edit {
        path: VirtualPath::from_absolute("/mempool/writing/on-writing-slow.md").unwrap(),
        body: String::new(),
    };
    assert_eq!(
        commit_message(&new_mode, &form),
        "mempool: add writing/on-writing-slow"
    );
    assert_eq!(
        commit_message(&edit_mode, &form),
        "mempool: edit writing/on-writing-slow"
    );
}

#[test]
fn derive_edit_form_recovers_authoring_inputs_from_serialized_body() {
    let original = sample_form();
    let body = serialize_mempool_file(&form_to_payload(&original));
    let path = target_path(&original);
    let mode = ComposeMode::Edit {
        path: path.clone(),
        body,
    };
    let derived = derive_form_from_mode(&mode, "2026-04-28");
    assert_eq!(derived.title, original.title);
    assert_eq!(derived.status, original.status);
    assert_eq!(derived.modified, original.modified);
    assert_eq!(derived.priority, original.priority);
    assert_eq!(derived.tags, original.tags);
    assert_eq!(derived.category, original.category);
    assert_eq!(derived.slug, original.slug);
    assert_eq!(derived.body.trim(), original.body.trim());
}
