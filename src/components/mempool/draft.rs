//! Pure helpers for the `/new` raw-markdown compose flow.
//!
//! The Phase 7 reader edits raw markdown — frontmatter is part of the
//! textarea text, not a structured form. For a new draft, we still need
//! to compute the destination path (`/mempool/<category>/<slug>.md`) at
//! save time, which means parsing the frontmatter the user just typed.
//! These helpers do exactly that, plus produce the placeholder text the
//! page seeds the textarea with on `/new`.

use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::models::VirtualPath;

use super::parse::parse_mempool_frontmatter;
use super::serialize::slug_from_title;

/// Characters in a `title` that the naive `parse_mempool_frontmatter`
/// (split on first `:`, strip outer quotes) cannot round-trip safely.
/// `derive_new_path` rejects them so the user gets a clear error rather
/// than a silently-wrong slug.
const TITLE_RESERVED: &[char] = &['"', '\\', '\n', '\r', ':'];

/// YAML frontmatter placeholder for the `/new` compose flow. The `today`
/// argument is injected so unit tests are deterministic; the page passes
/// `format_date_iso(current_timestamp() / 1000)`.
///
/// The placeholder's `category` is `LEDGER_CATEGORIES[0]` so it stays in
/// sync with `derive_form_from_mode`'s default and the test invariant.
pub fn placeholder_frontmatter(today: &str) -> String {
    let category = LEDGER_CATEGORIES[0];
    format!(
        "---\n\
         title: \"\"\n\
         category: {category}\n\
         status: draft\n\
         modified: {today}\n\
         ---\n\n"
    )
}

/// Parse `raw_body`'s frontmatter and derive the canonical save path for
/// a new mempool draft. Returns the human-readable error string the page
/// surfaces in `save_error`.
///
/// Contract:
/// - title required; trimmed; no [`TITLE_RESERVED`] chars.
/// - category required; ∈ [`LEDGER_CATEGORIES`].
/// - explicit `slug:` is ignored — slug is derived from title via
///   [`slug_from_title`].
pub fn derive_new_path(raw_body: &str) -> Result<VirtualPath, String> {
    let meta = parse_mempool_frontmatter(raw_body)
        .ok_or_else(|| "frontmatter is missing the leading `---` fence".to_string())?;
    let title = meta
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "title is required".to_string())?;
    if title.chars().any(|c| TITLE_RESERVED.contains(&c)) {
        return Err("title cannot contain \" \\ : or newlines".to_string());
    }
    let category = meta
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "category is required".to_string())?;
    if !LEDGER_CATEGORIES.contains(&category) {
        return Err(format!(
            "category must be one of: {}",
            LEDGER_CATEGORIES.join(", ")
        ));
    }
    let slug = slug_from_title(title);
    if slug.is_empty() {
        return Err("title must produce a non-empty slug".to_string());
    }
    VirtualPath::from_absolute(format!("/mempool/{category}/{slug}.md"))
        .map_err(|error| format!("cannot build path: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(title: &str, category: &str) -> String {
        format!("---\ntitle: \"{title}\"\ncategory: {category}\n---\nbody\n")
    }

    #[test]
    fn happy_path_writes_expected_path() {
        let raw = body("On writing slow", "writing");
        let path = derive_new_path(&raw).expect("ok");
        assert_eq!(path.as_str(), "/mempool/writing/on-writing-slow.md");
    }

    #[test]
    fn rejects_missing_frontmatter_fence() {
        let raw = "no frontmatter here\n";
        assert!(derive_new_path(raw).unwrap_err().contains("`---`"));
    }

    #[test]
    fn rejects_empty_title() {
        let raw = body("", "writing");
        assert_eq!(derive_new_path(&raw).unwrap_err(), "title is required");
    }

    #[test]
    fn rejects_title_with_dangerous_chars_in_value() {
        // Characters that survive into the title's parsed value (`"`, `\`, `:`)
        // must be rejected so the slug isn't silently corrupted. Newlines
        // (`\n`, `\r`) can't reach the title value at all — the line-based
        // frontmatter parser eats them — so they don't need a rejection path.
        for bad in ['"', '\\', ':'] {
            let raw = format!(
                "---\ntitle: hello{bad}world\ncategory: writing\n---\n"
            );
            let err = derive_new_path(&raw).unwrap_err();
            assert!(
                err.contains("cannot contain"),
                "char {bad:?}: got {err}"
            );
        }
    }

    #[test]
    fn rejects_missing_category() {
        let raw = "---\ntitle: x\n---\n";
        assert_eq!(derive_new_path(raw).unwrap_err(), "category is required");
    }

    #[test]
    fn rejects_unknown_category() {
        let raw = body("hello", "blog");
        let err = derive_new_path(&raw).unwrap_err();
        assert!(err.contains("category must be one of"), "got {err}");
    }

    #[test]
    fn ignores_explicit_slug_key() {
        // The frontmatter parser doesn't know about a `slug:` key, so it
        // ignores it. derive_new_path's slug comes from title only.
        let raw = "---\ntitle: \"Hello World\"\ncategory: writing\nslug: ignored\n---\n";
        let path = derive_new_path(raw).expect("ok");
        assert_eq!(path.as_str(), "/mempool/writing/hello-world.md");
    }

    #[test]
    fn placeholder_round_trips_through_parser() {
        let placeholder = placeholder_frontmatter("2026-04-29");
        let meta = parse_mempool_frontmatter(&placeholder)
            .expect("placeholder must parse cleanly");
        assert_eq!(meta.category.as_deref(), Some(LEDGER_CATEGORIES[0]));
        assert_eq!(meta.modified.as_deref(), Some("2026-04-29"));
        assert_eq!(meta.status.as_deref(), Some("draft"));
    }

    #[test]
    fn placeholder_category_matches_default() {
        let placeholder = placeholder_frontmatter("2026-01-01");
        // The default category should equal LEDGER_CATEGORIES[0] (currently "writing").
        assert!(placeholder.contains(&format!("category: {}", LEDGER_CATEGORIES[0])));
    }

    #[test]
    fn placeholder_can_be_completed_into_a_valid_save() {
        // User fills in title; category stays as the default. derive_new_path succeeds.
        let placeholder = placeholder_frontmatter("2026-04-29");
        let filled = placeholder.replace("title: \"\"", "title: \"My First Draft\"");
        let path = derive_new_path(&filled).expect("ok");
        assert_eq!(
            path.as_str(),
            "/mempool/writing/my-first-draft.md"
        );
    }
}
