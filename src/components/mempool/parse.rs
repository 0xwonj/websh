//! Frontmatter parsing and auto-derivation helpers for mempool entries.

use crate::models::VirtualPath;
use crate::utils::format::format_size;

use super::model::{MempoolStatus, Priority};

const DESC_MAX_CHARS: usize = 140;

pub fn parse_mempool_status(value: &str) -> Option<MempoolStatus> {
    match value {
        "draft" => Some(MempoolStatus::Draft),
        "review" => Some(MempoolStatus::Review),
        _ => None,
    }
}

pub fn parse_priority(value: &str) -> Option<Priority> {
    match value {
        "low" => Some(Priority::Low),
        "med" => Some(Priority::Med),
        "high" => Some(Priority::High),
        _ => None,
    }
}

/// First non-heading paragraph from a markdown body. Skips `# ...` headings
/// at the top, joins continuation lines with a single space, truncates at
/// `DESC_MAX_CHARS` with an ellipsis.
pub fn extract_first_paragraph(body: &str) -> String {
    let body = strip_frontmatter(body);
    let mut lines = body
        .lines()
        .skip_while(|line| line.is_empty() || line.starts_with('#'));

    let mut paragraph = String::new();
    for line in lines.by_ref() {
        if line.trim().is_empty() {
            if !paragraph.is_empty() {
                break;
            }
            continue;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(line.trim());
    }

    if paragraph.chars().count() > DESC_MAX_CHARS {
        let truncated: String = paragraph.chars().take(DESC_MAX_CHARS).collect();
        format!("{}…", truncated.trim_end())
    } else {
        paragraph
    }
}

/// First path segment beneath `mempool_root`. Returns `"misc"` for files that
/// live directly under `mempool_root` (no category folder).
pub fn category_for_mempool_path(path: &VirtualPath, mempool_root: &VirtualPath) -> String {
    let path_str = path.as_str();
    let prefix = mempool_root.as_str();
    let rel = path_str
        .strip_prefix(prefix)
        .unwrap_or(path_str)
        .trim_start_matches('/');
    let mut segments = rel.split('/');
    let first = segments.next().unwrap_or("");
    if segments.next().is_none() {
        return "misc".to_string();
    }
    if first.is_empty() {
        "misc".to_string()
    } else {
        first.to_string()
    }
}

/// "Gas" — a vibe metric of entry size. Markdown gets a rounded word count;
/// binaries get `format_size`.
pub fn derive_gas(body: &str, byte_len: usize, is_markdown: bool) -> String {
    if !is_markdown {
        return format_size(Some(byte_len as u64), false);
    }
    let body_after_frontmatter = strip_frontmatter(body);
    let word_count = body_after_frontmatter.split_whitespace().count();
    let rounded = match word_count {
        0..=99 => word_count - (word_count % 10),
        100..=999 => word_count - (word_count % 50),
        _ => word_count - (word_count % 100),
    };
    format_with_thousands(rounded)
}

fn strip_frontmatter(body: &str) -> &str {
    let mut iter = body.splitn(3, "---\n");
    match (iter.next(), iter.next(), iter.next()) {
        (Some(empty), Some(_meta), Some(rest)) if empty.is_empty() => rest,
        _ => body,
    }
}

fn format_with_thousands(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, byte) in bytes.iter().enumerate() {
        let from_end = bytes.len() - i;
        if i > 0 && from_end % 3 == 0 {
            out.push(',');
        }
        out.push(*byte as char);
    }
    format!("~{out} words")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawMempoolMeta {
    pub title: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub modified: Option<String>,
    pub tags: Vec<String>,
}

/// Parse mempool-file frontmatter. Returns `None` when the input does not
/// open with a `---` fence (i.e., the file has no frontmatter and we should
/// skip it). Unknown keys are ignored; values are read as raw strings.
pub fn parse_mempool_frontmatter(body: &str) -> Option<RawMempoolMeta> {
    let mut lines = body.lines();
    if lines.next() != Some("---") {
        return None;
    }

    let mut meta = RawMempoolMeta::default();
    for line in lines {
        if line == "---" {
            return Some(meta);
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_matches('"').trim_matches('\'');
        match key {
            "title" => meta.title = Some(value.to_string()),
            "category" => meta.category = Some(value.to_string()),
            "status" => meta.status = Some(value.to_string()),
            "priority" => meta.priority = Some(value.to_string()),
            "modified" => meta.modified = Some(value.to_string()),
            "tags" => meta.tags = parse_inline_tags(value),
            _ => {}
        }
    }
    Some(meta)
}

fn parse_inline_tags(value: &str) -> Vec<String> {
    let trimmed = value.trim();
    if let Some(inner) = trimmed
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    {
        return inner
            .split(',')
            .map(|tag| tag.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|tag| !tag.is_empty())
            .collect();
    }
    if trimmed.is_empty() {
        Vec::new()
    } else {
        vec![trimmed.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn parses_full_frontmatter() {
        let raw = body(
            "---\n\
             title: \"On writing slow\"\n\
             category: writing\n\
             status: draft\n\
             priority: med\n\
             modified: \"2026-04-25\"\n\
             tags: [essay, writing-process]\n\
             ---\n\
             # On writing slow\n\nbody...\n",
        );
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("On writing slow"));
        assert_eq!(meta.category.as_deref(), Some("writing"));
        assert_eq!(meta.status.as_deref(), Some("draft"));
        assert_eq!(meta.priority.as_deref(), Some("med"));
        assert_eq!(meta.modified.as_deref(), Some("2026-04-25"));
        assert_eq!(meta.tags, vec!["essay".to_string(), "writing-process".to_string()]);
    }

    #[test]
    fn parses_category_when_present() {
        let raw = body("---\ntitle: t\ncategory: papers\n---\n");
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.category.as_deref(), Some("papers"));
    }

    #[test]
    fn category_absent_returns_none() {
        let raw = body("---\ntitle: t\nstatus: draft\n---\n");
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert!(meta.category.is_none());
    }

    #[test]
    fn parses_minimal_frontmatter() {
        let raw = body("---\ntitle: foo\nstatus: draft\nmodified: 2026-04-22\n---\nbody\n");
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("foo"));
        assert_eq!(meta.status.as_deref(), Some("draft"));
        assert!(meta.priority.is_none());
        assert_eq!(meta.modified.as_deref(), Some("2026-04-22"));
        assert!(meta.tags.is_empty());
    }

    #[test]
    fn returns_none_when_no_frontmatter_fence() {
        assert!(parse_mempool_frontmatter("# title\nbody\n").is_none());
    }

    #[test]
    fn returns_none_for_empty_input() {
        assert!(parse_mempool_frontmatter("").is_none());
    }

    #[test]
    fn ignores_unknown_keys() {
        let raw = body("---\ntitle: foo\nstatus: draft\nmodified: 2026-04-22\nfuture: ignore\n---\n");
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("foo"));
    }

    #[test]
    fn parses_status_canonical_values() {
        assert_eq!(parse_mempool_status("draft"), Some(MempoolStatus::Draft));
        assert_eq!(parse_mempool_status("review"), Some(MempoolStatus::Review));
        assert!(parse_mempool_status("published").is_none());
        assert!(parse_mempool_status("DRAFT").is_none());
        assert!(parse_mempool_status("").is_none());
    }

    #[test]
    fn parses_priority_canonical_values() {
        assert_eq!(parse_priority("low"), Some(Priority::Low));
        assert_eq!(parse_priority("med"), Some(Priority::Med));
        assert_eq!(parse_priority("high"), Some(Priority::High));
        assert!(parse_priority("medium").is_none());
        assert!(parse_priority("").is_none());
    }

    #[test]
    fn extracts_first_paragraph_skipping_heading() {
        let body = "# Title\n\nFirst paragraph here.\nStill same paragraph.\n\nSecond para.\n";
        assert_eq!(
            extract_first_paragraph(body),
            "First paragraph here. Still same paragraph."
        );
    }

    #[test]
    fn extracts_first_paragraph_with_no_heading() {
        let body = "Standalone para.\n\nAnother.\n";
        assert_eq!(extract_first_paragraph(body), "Standalone para.");
    }

    #[test]
    fn extracts_first_paragraph_skips_frontmatter() {
        let body = "---\n\
                    title: foo\n\
                    status: draft\n\
                    modified: 2026-04-01\n\
                    ---\n\
                    # Heading\n\
                    \n\
                    Real body paragraph.\n";
        assert_eq!(extract_first_paragraph(body), "Real body paragraph.");
    }

    #[test]
    fn extracts_first_paragraph_truncates_long_text() {
        let long = "x".repeat(200);
        let body = format!("{long}\n");
        let out = extract_first_paragraph(&body);
        assert!(out.len() <= 143, "got len={} body={}", out.len(), out);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn category_for_path_uses_first_segment_under_mempool() {
        use crate::models::VirtualPath;
        let path = VirtualPath::from_absolute("/mempool/writing/foo.md").unwrap();
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        assert_eq!(category_for_mempool_path(&path, &mempool_root), "writing");
    }

    #[test]
    fn category_for_path_handles_root_level_files() {
        use crate::models::VirtualPath;
        let path = VirtualPath::from_absolute("/mempool/loose.md").unwrap();
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        assert_eq!(category_for_mempool_path(&path, &mempool_root), "misc");
    }

    #[test]
    fn category_for_path_handles_nested_paths() {
        use crate::models::VirtualPath;
        let path = VirtualPath::from_absolute("/mempool/papers/series/foo.md").unwrap();
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        assert_eq!(category_for_mempool_path(&path, &mempool_root), "papers");
    }

    #[test]
    fn derives_gas_for_markdown_word_count() {
        let body = "---\nfront: matter\n---\n# Heading\n\n".to_string()
            + &"word ".repeat(420);
        let gas = derive_gas(&body, body.len(), true);
        // 420 words → "~400 words" (rounded down to nearest 50 in 100..=999 bucket)
        assert_eq!(gas, "~400 words");
    }

    #[test]
    fn derives_gas_for_binary_uses_size() {
        let gas = derive_gas("", 12_400, false);
        assert!(gas.contains("12") || gas.contains("kB") || gas.contains("KB"));
    }

    #[test]
    fn derives_gas_for_empty_markdown() {
        let gas = derive_gas("---\n---\n", 8, true);
        assert_eq!(gas, "~0 words");
    }
}
