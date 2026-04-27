//! Compose / Edit modal for mempool authoring.
//!
//! Owns:
//! - `ComposeMode` enum (New | Edit) and `ComposeForm` value type
//! - Form validation
//! - `save_compose` async handler that builds a `ChangeSet` and commits to
//!   the mempool backend via `commit_backend`
//! - `ComposeModal` Leptos component surfacing the form to the user
//!
//! The modal is conditionally rendered: when `open` is `Some(_)`, the form
//! is mounted and seeded from the mode; when `None`, nothing renders.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::app::AppContext;
use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::core::changes::{ChangeSet, ChangeType};
use crate::core::runtime::commit_backend;
use crate::core::runtime::state::github_token_for_commit;
use crate::models::{FileMetadata, VirtualPath};
use crate::utils::current_timestamp;
use crate::utils::format::{format_date_iso, iso_date_prefix};

use super::loader::mempool_root;
use super::parse::parse_mempool_frontmatter;
use super::serialize::{ComposePayload, serialize_mempool_file, slug_from_title};

stylance::import_crate_style!(css, "src/components/mempool/compose.module.css");

const ALLOWED_STATUSES: &[&str] = &["draft", "review"];

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
    SlugInvalid,
    StatusUnknown,
    ModifiedNotIso,
    CategoryUnknown,
}

pub fn validate_form(form: &ComposeForm) -> Vec<ComposeError> {
    let mut errors = Vec::new();
    if form.title.trim().is_empty() {
        errors.push(ComposeError::TitleEmpty);
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
    bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'-')
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

/// Today as `YYYY-MM-DD` from the wall clock. Wraps `current_timestamp` so
/// `derive_form_from_mode` can stay deterministic in tests.
fn iso_today() -> String {
    format_date_iso(current_timestamp() / 1000)
}

/// Strip a leading `---\n...---\n` frontmatter block from a markdown file
/// and return the remaining body. Mirrors `parse::strip_frontmatter` but is
/// kept local so the compose module is self-contained for serialization
/// round-trips. Leading newlines after the closing fence are trimmed.
fn body_after_frontmatter(body: &str) -> String {
    let mut iter = body.splitn(3, "---\n");
    match (iter.next(), iter.next(), iter.next()) {
        (Some(empty), Some(_meta), Some(rest)) if empty.is_empty() => {
            rest.trim_start_matches('\n').to_string()
        }
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
            let category = if segments.clone().next().is_some()
                && LEDGER_CATEGORIES.contains(&first)
            {
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

    let root = mempool_root();
    let backend = ctx
        .backend_for_path(&root)
        .ok_or_else(|| "mempool mount is not configured".to_string())?;
    let token = github_token_for_commit()
        .ok_or_else(|| "missing GitHub token for mempool commit".to_string())?;
    let expected_head = ctx.remote_head_for_path(&root);

    let message = commit_message(&mode, &form);
    let changes = build_change_set(&mode, &form);

    commit_backend(backend, root, changes, message, expected_head, Some(token))
        .await
        .map(|_outcome| ())
        .map_err(|err| err.to_string())
}

#[component]
pub fn ComposeModal(
    open: ReadSignal<Option<ComposeMode>>,
    set_open: WriteSignal<Option<ComposeMode>>,
    #[prop(into)] on_saved: Callback<()>,
) -> impl IntoView {
    let ctx = use_context::<AppContext>().expect("AppContext must be provided");

    let form = RwSignal::new(ComposeForm::default());
    let errors = RwSignal::new(Vec::<ComposeError>::new());
    let save_error = RwSignal::new(None::<String>);
    let saving = RwSignal::new(false);

    // Seed the form whenever `open` transitions to a new mode. We compare the
    // mode by value: re-running the seed on the same open mode would clobber
    // the user's edits, so the effect tracks the last-seen mode.
    let last_mode = StoredValue::new(None::<ComposeMode>);
    Effect::new(move |_| {
        let next = open.get();
        let last = last_mode.get_value();
        if next == last {
            return;
        }
        last_mode.set_value(next.clone());
        if let Some(mode) = next {
            let seeded = derive_form_from_mode(&mode, &iso_today());
            errors.set(validate_form(&seeded));
            form.set(seeded);
            save_error.set(None);
            saving.set(false);
        }
    });

    let close = move || {
        set_open.set(None);
    };

    let on_field_change = move || {
        errors.set(validate_form(&form.get_untracked()));
    };

    let title_input = move |ev| {
        form.update(|f| {
            f.title = event_target_value(&ev);
            if f.slug.is_empty() {
                f.slug = slug_from_title(&f.title);
            }
        });
        on_field_change();
    };

    let slug_input = move |ev| {
        form.update(|f| f.slug = event_target_value(&ev));
        on_field_change();
    };

    let category_input = move |ev| {
        form.update(|f| f.category = event_target_value(&ev));
        on_field_change();
    };

    let status_input = move |ev| {
        form.update(|f| f.status = event_target_value(&ev));
        on_field_change();
    };

    let priority_input = move |ev| {
        let value = event_target_value(&ev);
        form.update(|f| {
            f.priority = if value.is_empty() { None } else { Some(value) };
        });
    };

    let modified_input = move |ev| {
        form.update(|f| f.modified = event_target_value(&ev));
        on_field_change();
    };

    let tags_input = move |ev| {
        let value = event_target_value(&ev);
        form.update(|f| {
            f.tags = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        });
    };

    let body_input = move |ev| {
        form.update(|f| f.body = event_target_value(&ev));
    };

    let on_save = move |_| {
        let Some(mode) = open.get_untracked() else {
            return;
        };
        let snapshot = form.get_untracked();
        let errs = validate_form(&snapshot);
        if !errs.is_empty() {
            errors.set(errs);
            save_error.set(Some("fix the highlighted fields before saving".into()));
            return;
        }
        save_error.set(None);
        saving.set(true);
        let ctx_clone = ctx.clone();
        spawn_local(async move {
            let result = save_compose(ctx_clone, mode, snapshot).await;
            saving.set(false);
            match result {
                Ok(()) => {
                    on_saved.run(());
                    set_open.set(None);
                }
                Err(message) => save_error.set(Some(message)),
            }
        });
    };

    let on_cancel = move |_| close();

    let mode_label = move || match open.get() {
        Some(ComposeMode::New { .. }) => "compose",
        Some(ComposeMode::Edit { .. }) => "edit",
        None => "",
    };

    let save_disabled = move || saving.get() || !errors.with(|e| e.is_empty());

    let priority_value = move || form.with(|f| f.priority.clone().unwrap_or_default());
    let tags_value = move || form.with(|f| f.tags.join(", "));

    let has_error = move |kind: ComposeError| errors.with(|e| e.contains(&kind));

    view! {
        <Show when=move || open.with(|o| o.is_some())>
            <div
                class=css::backdrop
                on:click=move |_| close()
            >
                <div
                    class=css::panel
                    role="dialog"
                    aria-label="Mempool compose"
                    on:click=|ev: leptos::ev::MouseEvent| ev.stop_propagation()
                >
                    <header class=css::header>
                        <span class=css::modeTag>{mode_label}</span>
                        <button
                            class=css::close
                            type="button"
                            aria-label="Close"
                            on:click=move |_| close()
                        >
                            "\u{00d7}"
                        </button>
                    </header>
                    <div class=css::body>
                        {move || save_error.get().map(|message| view! {
                            <div class=css::errorBanner role="alert">{message}</div>
                        })}
                        <div class=css::row>
                            <label class=css::field>
                                <span class=css::label>"title"</span>
                                <input
                                    class=css::input
                                    r#type="text"
                                    prop:value=move || form.with(|f| f.title.clone())
                                    on:input=title_input
                                />
                                {move || has_error(ComposeError::TitleEmpty).then(|| view! {
                                    <span class=css::fieldError>"title is required"</span>
                                })}
                            </label>
                        </div>
                        <div class=css::row>
                            <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                                <span class=css::label>"category"</span>
                                <select
                                    class=css::select
                                    prop:value=move || form.with(|f| f.category.clone())
                                    on:change=category_input
                                >
                                    {LEDGER_CATEGORIES.iter().map(|cat| view! {
                                        <option value=*cat>{*cat}</option>
                                    }).collect_view()}
                                </select>
                                {move || has_error(ComposeError::CategoryUnknown).then(|| view! {
                                    <span class=css::fieldError>"unknown category"</span>
                                })}
                            </label>
                            <label class=css::field>
                                <span class=css::label>"slug"</span>
                                <input
                                    class=css::input
                                    r#type="text"
                                    prop:value=move || form.with(|f| f.slug.clone())
                                    on:input=slug_input
                                />
                                {move || has_error(ComposeError::SlugInvalid).then(|| view! {
                                    <span class=css::fieldError>"slug must be kebab-case ASCII"</span>
                                })}
                            </label>
                        </div>
                        <div class=css::row>
                            <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                                <span class=css::label>"status"</span>
                                <select
                                    class=css::select
                                    prop:value=move || form.with(|f| f.status.clone())
                                    on:change=status_input
                                >
                                    <option value="draft">"draft"</option>
                                    <option value="review">"review"</option>
                                </select>
                                {move || has_error(ComposeError::StatusUnknown).then(|| view! {
                                    <span class=css::fieldError>"status must be draft or review"</span>
                                })}
                            </label>
                            <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                                <span class=css::label>"priority"</span>
                                <select
                                    class=css::select
                                    prop:value=priority_value
                                    on:change=priority_input
                                >
                                    <option value="">"—"</option>
                                    <option value="low">"low"</option>
                                    <option value="med">"med"</option>
                                    <option value="high">"high"</option>
                                </select>
                            </label>
                            <label class={format!("{} {}", css::field, css::fieldNarrow)}>
                                <span class=css::label>"modified"</span>
                                <input
                                    class=css::input
                                    r#type="text"
                                    placeholder="YYYY-MM-DD"
                                    prop:value=move || form.with(|f| f.modified.clone())
                                    on:input=modified_input
                                />
                                {move || has_error(ComposeError::ModifiedNotIso).then(|| view! {
                                    <span class=css::fieldError>"date must be YYYY-MM-DD"</span>
                                })}
                            </label>
                            <label class=css::field>
                                <span class=css::label>"tags"</span>
                                <input
                                    class=css::input
                                    r#type="text"
                                    placeholder="comma, separated"
                                    prop:value=tags_value
                                    on:input=tags_input
                                />
                            </label>
                        </div>
                        <textarea
                            class=css::bodyArea
                            placeholder="Markdown body…"
                            prop:value=move || form.with(|f| f.body.clone())
                            on:input=body_input
                        />
                    </div>
                    <footer class=css::footer>
                        <button
                            class=css::cancel
                            type="button"
                            on:click=on_cancel
                            prop:disabled=move || saving.get()
                        >
                            "Cancel"
                        </button>
                        <button
                            class=css::save
                            type="button"
                            on:click=on_save
                            prop:disabled=save_disabled
                        >
                            {move || if saving.get() { "Saving…" } else { "Save" }}
                        </button>
                    </footer>
                </div>
            </div>
        </Show>
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
        assert_eq!(save_path_for(&mode, &form).as_str(), "/mempool/papers/alpha.md");
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
