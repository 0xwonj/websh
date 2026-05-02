//! Slug derivation and frontmatter serialization for mempool authoring.
//!
//! Pairs with `parse::parse_mempool_frontmatter` — the writer emits only
//! the keys the parser knows how to read. Strings are quoted with `"..."`
//! and `\` / `"` are backslash-escaped; tags are emitted inline.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposePayload {
    pub title: String,
    pub status: String,
    pub modified: String,
    pub priority: Option<String>,
    pub tags: Vec<String>,
    pub body: String,
}

pub fn slug_from_title(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut prev_dash = false;
    for ch in title.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            slug.push(lower);
            prev_dash = false;
        } else if !slug.is_empty() && !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

pub fn serialize_mempool_file(payload: &ComposePayload) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("title: \"{}\"\n", escape_yaml(&payload.title)));
    out.push_str(&format!("status: {}\n", payload.status));
    out.push_str(&format!("modified: \"{}\"\n", payload.modified));
    if let Some(p) = &payload.priority {
        out.push_str(&format!("priority: {p}\n"));
    }
    if !payload.tags.is_empty() {
        out.push_str(&format!("tags: [{}]\n", payload.tags.join(", ")));
    }
    out.push_str("---\n\n");
    out.push_str(&payload.body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn escape_yaml(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_from_title_kebab_cases_basic() {
        assert_eq!(slug_from_title("On Writing Slow"), "on-writing-slow");
    }

    #[test]
    fn slug_from_title_strips_punctuation() {
        assert_eq!(slug_from_title("Hello, World!"), "hello-world");
    }

    #[test]
    fn slug_from_title_collapses_double_dashes() {
        assert_eq!(slug_from_title("foo  --  bar"), "foo-bar");
    }

    #[test]
    fn slug_from_title_falls_back_for_empty() {
        assert_eq!(slug_from_title(""), "untitled");
        assert_eq!(slug_from_title("!!!"), "untitled");
    }

    #[test]
    fn serialize_emits_required_fields_only() {
        let body = serialize_mempool_file(&ComposePayload {
            title: "foo".into(),
            status: "draft".into(),
            modified: "2026-04-28".into(),
            priority: None,
            tags: vec![],
            body: "Hello.".into(),
        });
        assert!(body.starts_with("---\n"));
        assert!(body.contains("title: \"foo\"\n"));
        assert!(body.contains("status: draft\n"));
        assert!(body.contains("modified: \"2026-04-28\"\n"));
        assert!(!body.contains("priority"));
        assert!(!body.contains("tags"));
        assert!(body.ends_with("Hello.\n"));
    }

    #[test]
    fn serialize_includes_optional_fields_when_set() {
        let body = serialize_mempool_file(&ComposePayload {
            title: "foo".into(),
            status: "review".into(),
            modified: "2026-04-28".into(),
            priority: Some("high".into()),
            tags: vec!["zk".into(), "essay".into()],
            body: "Body.".into(),
        });
        assert!(body.contains("priority: high\n"));
        assert!(body.contains("tags: [zk, essay]\n"));
    }
}
