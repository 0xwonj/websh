//! Pure helpers for mempool compose authoring.
//!
//! Owns:
//! - `ComposeMode` enum (New | Edit) and `ComposeForm` value type
//! - Form validation (`validate_form`, `ComposeError`)
//! - Mode → form derivation (`derive_form_from_mode`)
//! - Save path / commit message helpers
//! - `save_compose` async handler that builds a `ChangeSet` and commits to
//!   the mempool backend via `commit_backend`
//!
//! Phase 6 deleted the `ComposeModal` Leptos component that lived here. The
//! un-modal'd `MempoolEditor` (in `editor.rs`) and the CLI `mempool add`
//! subcommand now share these helpers.

use leptos::prelude::*;

use crate::app::AppContext;
use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::runtime::commit_backend;
use crate::core::runtime::state::github_token_for_commit;
use crate::core::storage::CommitOutcome;
use crate::models::{FileMetadata, RuntimeMount, VirtualPath};
use crate::utils::format::iso_date_prefix;

use super::loader::mempool_root;
use super::parse::parse_mempool_frontmatter;
use super::serialize::{ComposePayload, serialize_mempool_file};

const ALLOWED_STATUSES: &[&str] = &["draft", "review"];
const ALLOWED_PRIORITIES: &[&str] = &["low", "med", "high"];

/// Characters in a `title` that the simple quoted-string YAML serializer
/// cannot round-trip through `parse_mempool_frontmatter`. Validation
/// rejects them outright rather than risk silent corruption on save.
const TITLE_RESERVED: &[char] = &['"', '\\', '\n', '\r', ':'];

/// Characters in a single `tag` that break the inline-list shape
/// `tags: [a, b, c]`. Same validation rationale as titles.
const TAG_RESERVED: &[char] = &['[', ']', ',', '"', '\n', '\r'];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeMode {
    New { default_category: Option<String> },
    Edit { path: VirtualPath, body: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ComposeForm {
    pub title: String,
    pub category: String,
    pub slug: String,
    pub status: String,
    pub modified: String,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposeError {
    TitleEmpty,
    TitleHasReservedChars,
    SlugInvalid,
    StatusUnknown,
    ModifiedNotIso,
    CategoryUnknown,
    PriorityUnknown,
    TagHasReservedChars,
}

pub fn validate_form(form: &ComposeForm) -> Vec<ComposeError> {
    let mut errors = Vec::new();
    if form.title.trim().is_empty() {
        errors.push(ComposeError::TitleEmpty);
    } else if form.title.chars().any(|c| TITLE_RESERVED.contains(&c)) {
        errors.push(ComposeError::TitleHasReservedChars);
    }
    if !slug_is_valid(&form.slug) {
        errors.push(ComposeError::SlugInvalid);
    }
    if !ALLOWED_STATUSES.contains(&form.status.as_str()) {
        errors.push(ComposeError::StatusUnknown);
    }
    if iso_date_prefix(&form.modified).is_none() {
        errors.push(ComposeError::ModifiedNotIso);
    }
    if !LEDGER_CATEGORIES.contains(&form.category.as_str()) {
        errors.push(ComposeError::CategoryUnknown);
    }
    if let Some(priority) = &form.priority
        && !ALLOWED_PRIORITIES.contains(&priority.as_str())
    {
        errors.push(ComposeError::PriorityUnknown);
    }
    if form
        .tags
        .iter()
        .any(|tag| tag.chars().any(|c| TAG_RESERVED.contains(&c)))
    {
        errors.push(ComposeError::TagHasReservedChars);
    }
    errors
}

fn slug_is_valid(slug: &str) -> bool {
    if slug.is_empty() {
        return false;
    }
    let bytes = slug.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'-')
}

pub fn form_to_payload(form: &ComposeForm) -> ComposePayload {
    ComposePayload {
        title: form.title.trim().to_string(),
        status: form.status.clone(),
        modified: form.modified.clone(),
        priority: form.priority.clone(),
        tags: form.tags.clone(),
        body: form.body.clone(),
    }
}

pub fn target_path(form: &ComposeForm) -> VirtualPath {
    VirtualPath::from_absolute(format!("/mempool/{}/{}.md", form.category, form.slug))
        .expect("compose target path is absolute")
}

/// Strip a leading `---\n...---\n` frontmatter block from a markdown file
/// and return the remaining body. Mirrors `parse::strip_frontmatter` but is
/// kept local so the compose module is self-contained for serialization
/// round-trips. Leading newlines after the closing fence are trimmed.
fn body_after_frontmatter(body: &str) -> String {
    let mut iter = body.splitn(3, "---\n");
    match (iter.next(), iter.next(), iter.next()) {
        (Some(""), Some(_meta), Some(rest)) => rest.trim_start_matches('\n').to_string(),
        _ => body.to_string(),
    }
}

/// Derive a `ComposeForm` from a `ComposeMode`. `today` is injected so the
/// caller can provide the current date deterministically (production passes
/// `iso_today()`; tests pass a fixed string).
pub fn derive_form_from_mode(mode: &ComposeMode, today: &str) -> ComposeForm {
    match mode {
        ComposeMode::New { default_category } => {
            let category = default_category
                .clone()
                .filter(|c| LEDGER_CATEGORIES.contains(&c.as_str()))
                .unwrap_or_else(|| LEDGER_CATEGORIES[0].to_string());
            ComposeForm {
                title: String::new(),
                category,
                slug: String::new(),
                status: "draft".to_string(),
                modified: today.to_string(),
                priority: None,
                tags: Vec::new(),
                body: String::new(),
            }
        }
        ComposeMode::Edit { path, body } => {
            let meta = parse_mempool_frontmatter(body).unwrap_or_default();
            let mempool = mempool_root();
            let rel = path
                .as_str()
                .strip_prefix(mempool.as_str())
                .unwrap_or_else(|| path.as_str())
                .trim_start_matches('/');
            let mut segments = rel.split('/');
            let first = segments.next().unwrap_or("");
            let category =
                if segments.clone().next().is_some() && LEDGER_CATEGORIES.contains(&first) {
                    first.to_string()
                } else {
                    LEDGER_CATEGORIES[0].to_string()
                };
            let slug = path
                .as_str()
                .rsplit('/')
                .next()
                .unwrap_or("")
                .strip_suffix(".md")
                .unwrap_or("")
                .to_string();
            ComposeForm {
                title: meta.title.unwrap_or_default(),
                category,
                slug,
                status: meta.status.unwrap_or_else(|| "draft".to_string()),
                modified: meta.modified.unwrap_or_else(|| today.to_string()),
                priority: meta.priority,
                tags: meta.tags,
                body: body_after_frontmatter(body),
            }
        }
    }
}

/// Determine the canonical save path for a form. New mode derives the path
/// from `category`/`slug`; Edit mode preserves the existing path so renames
/// stay out of scope for Phase 2.
pub fn save_path_for(mode: &ComposeMode, form: &ComposeForm) -> VirtualPath {
    match mode {
        ComposeMode::New { .. } => target_path(form),
        ComposeMode::Edit { path, .. } => path.clone(),
    }
}

/// Build the commit message for a save. New = add, Edit = edit.
pub fn commit_message(mode: &ComposeMode, form: &ComposeForm) -> String {
    let path = save_path_for(mode, form);
    let rel = path
        .as_str()
        .strip_prefix(mempool_root().as_str())
        .unwrap_or(path.as_str())
        .trim_start_matches('/')
        .trim_end_matches(".md");
    match mode {
        ComposeMode::New { .. } => format!("mempool: add {rel}"),
        ComposeMode::Edit { .. } => format!("mempool: edit {rel}"),
    }
}

/// Build the staged `ChangeSet` for the form save. Returned independently so
/// it is unit-testable without a backend or async runtime.
pub fn build_change_set(mode: &ComposeMode, form: &ComposeForm) -> ChangeSet {
    let payload = form_to_payload(form);
    let body = serialize_mempool_file(&payload);
    let path = save_path_for(mode, form);
    let mut changes = ChangeSet::new();
    let change = match mode {
        ComposeMode::New { .. } => ChangeType::CreateFile {
            content: body,
            meta: FileMetadata::default(),
        },
        ComposeMode::Edit { .. } => ChangeType::UpdateFile {
            content: body,
            description: None,
        },
    };
    changes.upsert(path, change);
    changes
}

/// Validate the form, build a ChangeSet, resolve auth + backend, and commit.
/// Returns `Ok(())` on success and a human-readable message on failure.
pub async fn save_compose(
    ctx: AppContext,
    mode: ComposeMode,
    form: ComposeForm,
) -> Result<(), String> {
    let errs = validate_form(&form);
    if !errs.is_empty() {
        return Err(format!("invalid form ({} field error(s))", errs.len()));
    }

    if matches!(mode, ComposeMode::New { .. }) {
        let target = target_path(&form);
        let collides = ctx.view_global_fs.with_untracked(|fs| fs.exists(&target));
        if collides {
            return Err(format!(
                "draft already exists at {} — pick a different slug",
                target.as_str()
            ));
        }
    }

    let root = mempool_root();
    let backend = ctx.backend_for_mount_root(&root).ok_or_else(|| {
        "mempool mount is not registered — check that \
             content/.websh/mounts/mempool.mount.json exists and \
             content/manifest.json is up to date"
            .to_string()
    })?;
    let token = github_token_for_commit()
        .ok_or_else(|| "missing GitHub token for mempool commit".to_string())?;
    let expected_head = ctx.remote_head_for_path(&root);

    let message = commit_message(&mode, &form);
    let changes = build_change_set(&mode, &form);

    let outcome = commit_backend(
        backend,
        root.clone(),
        changes,
        message,
        expected_head,
        Some(token),
    )
    .await
    .map_err(|err| err.to_string())?;
    apply_commit_outcome(&ctx, &root, &outcome).await;

    // Refresh the runtime so view_global_fs reflects the new GitHub state
    // — without this, the LocalResource refetch sees the same in-memory
    // tree it had before the commit and the new entry never appears.
    // Best-effort: a reload failure logs but does not poison the
    // already-successful commit.
    match crate::core::runtime::reload_runtime().await {
        Ok(load) => ctx.apply_runtime_load(load),
        Err(error) => {
            leptos::logging::warn!("compose: runtime reload after commit failed: {error}")
        }
    }
    Ok(())
}

/// Save raw markdown bytes (frontmatter included) to the mempool repo.
///
/// Used by the Phase 7 single-component reader's Edit/View toggle. Unlike
/// `save_compose`, this helper does not parse or serialize a structured
/// `ComposeForm` — the user is editing raw text in a textarea, so the
/// bytes pass through unchanged. Validation and frontmatter parsing is
/// the caller's responsibility (the page calls `derive_new_path` for new
/// drafts; existing edits skip validation since the runtime parser is
/// forgiving).
///
/// Mirrors `save_compose`'s backend resolution, commit, post-commit
/// bookkeeping (`apply_commit_outcome`), and runtime reload pattern.
/// Reload errors are logged via `leptos::logging::warn!` but do not
/// poison a successful commit.
pub async fn save_raw(
    ctx: AppContext,
    path: VirtualPath,
    body: String,
    message: String,
    is_new: bool,
) -> Result<(), String> {
    if is_new {
        let collides = ctx.view_global_fs.with_untracked(|fs| fs.exists(&path));
        if collides {
            return Err(format!(
                "draft already exists at {} — pick a different slug",
                path.as_str()
            ));
        }
    }

    let root = mempool_root();
    let backend = ctx.backend_for_mount_root(&root).ok_or_else(|| {
        "mempool mount is not registered — check that \
         content/.websh/mounts/mempool.mount.json exists and \
         content/manifest.json is up to date"
            .to_string()
    })?;
    let token = github_token_for_commit()
        .ok_or_else(|| "missing GitHub token for mempool commit".to_string())?;
    let expected_head = ctx.remote_head_for_path(&root);

    let mut changes = ChangeSet::new();
    let change = if is_new {
        ChangeType::CreateFile {
            content: body,
            meta: FileMetadata::default(),
        }
    } else {
        ChangeType::UpdateFile {
            content: body,
            description: None,
        }
    };
    changes.upsert(path, change);

    let outcome = commit_backend(
        backend,
        root.clone(),
        changes,
        message,
        expected_head,
        Some(token),
    )
    .await
    .map_err(|err| err.to_string())?;
    apply_commit_outcome(&ctx, &root, &outcome).await;

    match crate::core::runtime::reload_runtime().await {
        Ok(load) => ctx.apply_runtime_load(load),
        Err(error) => {
            leptos::logging::warn!("compose: runtime reload after raw save failed: {error}")
        }
    }
    Ok(())
}

/// Apply the post-commit bookkeeping after a successful UI-driven commit:
/// update `ctx.remote_heads` so subsequent `expected_head` lookups reflect
/// the just-committed OID, and persist the new HEAD to IDB so the next
/// session boots with it. Best-effort — an IDB write failure is logged but
/// does not poison the in-memory signal.
///
/// Originally lived in `mempool::promote` for Phase 3's two-commit
/// orchestration; Phase 5 moves promote out of the wasm runtime, leaving
/// compose as the sole consumer.
pub(super) async fn apply_commit_outcome(
    ctx: &AppContext,
    mount_root: &VirtualPath,
    outcome: &CommitOutcome,
) {
    ctx.remote_heads.update(|map| {
        map.insert(mount_root.clone(), outcome.new_head.clone());
    });

    let storage_id = ctx
        .runtime_mounts
        .with_untracked(|mounts| {
            mounts
                .iter()
                .find(|m| &m.root == mount_root)
                .map(RuntimeMount::storage_id)
        })
        .unwrap_or_else(|| mount_id_fallback(mount_root));

    if let Ok(db) = crate::core::storage::idb::open_db().await
        && let Err(error) = crate::core::storage::idb::save_metadata(
            &db,
            &format!("remote_head.{storage_id}"),
            &outcome.new_head,
        )
        .await
    {
        leptos::logging::warn!(
            "compose: persist remote_head for {} failed: {error}",
            mount_root.as_str()
        );
    }
}

fn mount_id_fallback(root: &VirtualPath) -> String {
    if root.is_root() {
        "~".to_string()
    } else {
        root.as_str().trim_start_matches('/').replace('/', ":")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_form_rejects_empty_title() {
        let payload = sample(|p| p.title.clear());
        let errors = validate_form(&payload);
        assert!(errors.iter().any(|e| matches!(e, ComposeError::TitleEmpty)));
    }

    #[test]
    fn validate_form_rejects_unknown_status() {
        let payload = sample(|p| p.status = "published".into());
        let errors = validate_form(&payload);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ComposeError::StatusUnknown))
        );
    }

    #[test]
    fn validate_form_rejects_invalid_modified_date() {
        let payload = sample(|p| p.modified = "April 28".into());
        let errors = validate_form(&payload);
        assert!(
            errors
                .iter()
                .any(|e| matches!(e, ComposeError::ModifiedNotIso))
        );
    }

    #[test]
    fn validate_form_accepts_minimal_valid() {
        let payload = sample(|_| {});
        assert!(validate_form(&payload).is_empty());
    }

    #[test]
    fn validate_form_rejects_title_with_reserved_chars() {
        for bad in ['"', '\\', '\n', '\r', ':'] {
            let payload = sample(|p| p.title = format!("hello {bad} world"));
            let errs = validate_form(&payload);
            assert!(
                errs.contains(&ComposeError::TitleHasReservedChars),
                "expected TitleHasReservedChars for char {:?}; got {:?}",
                bad,
                errs
            );
        }
    }

    #[test]
    fn validate_form_rejects_unknown_priority() {
        let payload = sample(|p| p.priority = Some("urgent".into()));
        let errs = validate_form(&payload);
        assert!(errs.contains(&ComposeError::PriorityUnknown));
    }

    #[test]
    fn validate_form_accepts_known_priority_or_none() {
        for value in [
            None,
            Some("low".into()),
            Some("med".into()),
            Some("high".into()),
        ] {
            let payload = sample(|p| p.priority = value.clone());
            assert!(
                validate_form(&payload).is_empty(),
                "expected no errors for priority {:?}",
                value
            );
        }
    }

    #[test]
    fn validate_form_rejects_tags_with_reserved_chars() {
        let payload = sample(|p| p.tags = vec!["good".into(), "bad[tag]".into()]);
        let errs = validate_form(&payload);
        assert!(errs.contains(&ComposeError::TagHasReservedChars));
    }

    #[test]
    fn target_path_includes_category_and_slug() {
        let form = sample(|p| {
            p.category = "writing".into();
            p.slug = "foo".into();
        });
        assert_eq!(target_path(&form).as_str(), "/mempool/writing/foo.md");
    }

    #[test]
    fn derive_form_for_new_uses_default_category_when_known() {
        let mode = ComposeMode::New {
            default_category: Some("papers".into()),
        };
        let form = derive_form_from_mode(&mode, "2026-04-28");
        assert_eq!(form.category, "papers");
        assert_eq!(form.status, "draft");
        assert_eq!(form.modified, "2026-04-28");
        assert!(form.title.is_empty());
        assert!(form.slug.is_empty());
        assert!(form.body.is_empty());
    }

    #[test]
    fn derive_form_for_new_falls_back_to_first_category() {
        let mode = ComposeMode::New {
            default_category: Some("not-a-category".into()),
        };
        let form = derive_form_from_mode(&mode, "2026-04-28");
        assert_eq!(form.category, LEDGER_CATEGORIES[0]);
    }

    #[test]
    fn derive_form_for_edit_parses_path_and_frontmatter() {
        let path = VirtualPath::from_absolute("/mempool/writing/on-slow.md").unwrap();
        let body = "---\ntitle: \"On slow\"\nstatus: review\nmodified: \"2026-04-20\"\npriority: med\ntags: [essay, slow]\n---\n\nfirst para\n".to_string();
        let mode = ComposeMode::Edit { path, body };
        let form = derive_form_from_mode(&mode, "2026-04-28");
        assert_eq!(form.category, "writing");
        assert_eq!(form.slug, "on-slow");
        assert_eq!(form.title, "On slow");
        assert_eq!(form.status, "review");
        assert_eq!(form.modified, "2026-04-20");
        assert_eq!(form.priority.as_deref(), Some("med"));
        assert_eq!(form.tags, vec!["essay".to_string(), "slow".to_string()]);
        assert_eq!(form.body.trim(), "first para");
    }

    #[test]
    fn save_path_for_new_uses_form_category_slug() {
        let mode = ComposeMode::New {
            default_category: None,
        };
        let form = sample(|p| {
            p.category = "papers".into();
            p.slug = "alpha".into();
        });
        assert_eq!(
            save_path_for(&mode, &form).as_str(),
            "/mempool/papers/alpha.md"
        );
    }

    #[test]
    fn save_path_for_edit_preserves_existing_path() {
        let path = VirtualPath::from_absolute("/mempool/writing/old.md").unwrap();
        let mode = ComposeMode::Edit {
            path: path.clone(),
            body: String::new(),
        };
        let form = sample(|p| {
            p.category = "papers".into();
            p.slug = "renamed".into();
        });
        assert_eq!(save_path_for(&mode, &form), path);
    }

    #[test]
    fn commit_message_distinguishes_new_and_edit() {
        let form = sample(|p| {
            p.category = "writing".into();
            p.slug = "foo".into();
        });
        let new_mode = ComposeMode::New {
            default_category: None,
        };
        let edit_mode = ComposeMode::Edit {
            path: VirtualPath::from_absolute("/mempool/writing/foo.md").unwrap(),
            body: String::new(),
        };
        assert_eq!(commit_message(&new_mode, &form), "mempool: add writing/foo");
        assert_eq!(
            commit_message(&edit_mode, &form),
            "mempool: edit writing/foo"
        );
    }

    #[test]
    fn build_change_set_new_emits_create_file() {
        let form = sample(|_| {});
        let mode = ComposeMode::New {
            default_category: None,
        };
        let changes = build_change_set(&mode, &form);
        let entries: Vec<_> = changes.iter_all().collect();
        assert_eq!(entries.len(), 1);
        let (path, entry) = entries[0];
        assert_eq!(path.as_str(), "/mempool/writing/foo.md");
        assert!(matches!(&entry.change, ChangeType::CreateFile { .. }));
    }

    #[test]
    fn build_change_set_edit_emits_update_file() {
        let form = sample(|_| {});
        let mode = ComposeMode::Edit {
            path: VirtualPath::from_absolute("/mempool/writing/foo.md").unwrap(),
            body: String::new(),
        };
        let changes = build_change_set(&mode, &form);
        let entries: Vec<_> = changes.iter_all().collect();
        assert_eq!(entries.len(), 1);
        let (_, entry) = entries[0];
        assert!(matches!(&entry.change, ChangeType::UpdateFile { .. }));
    }

    #[test]
    fn body_after_frontmatter_strips_fence_block() {
        let raw = "---\ntitle: x\n---\n\nbody line\n";
        assert_eq!(body_after_frontmatter(raw), "body line\n");
    }

    #[test]
    fn body_after_frontmatter_returns_input_when_no_fence() {
        let raw = "no fence here\nstill body\n";
        assert_eq!(body_after_frontmatter(raw), raw);
    }

    fn sample(mutate: impl FnOnce(&mut ComposeForm)) -> ComposeForm {
        let mut form = ComposeForm {
            title: "foo".into(),
            category: "writing".into(),
            slug: "foo".into(),
            status: "draft".into(),
            modified: "2026-04-28".into(),
            priority: None,
            tags: vec![],
            body: "body".into(),
        };
        mutate(&mut form);
        form
    }
}
