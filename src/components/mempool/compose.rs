//! Compose / Edit modal types and validation for mempool authoring.

use crate::models::VirtualPath;
use crate::utils::format::iso_date_prefix;

use super::serialize::ComposePayload;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComposeMode {
    New { default_category: Option<String> },
    Edit { path: VirtualPath, body: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

const ALLOWED_STATUSES: &[&str] = &["draft", "review"];
const ALLOWED_CATEGORIES: &[&str] = &["writing", "projects", "papers", "talks"];

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
    if !ALLOWED_CATEGORIES.contains(&form.category.as_str()) {
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
        assert!(errors.iter().any(|e| matches!(e, ComposeError::StatusUnknown)));
    }

    #[test]
    fn validate_form_rejects_invalid_modified_date() {
        let payload = sample(|p| p.modified = "April 28".into());
        let errors = validate_form(&payload);
        assert!(errors.iter().any(|e| matches!(e, ComposeError::ModifiedNotIso)));
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
