//! Frontmatter parsing and auto-derivation helpers for mempool entries.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawMempoolMeta {
    pub title: Option<String>,
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
             status: draft\n\
             priority: med\n\
             modified: \"2026-04-25\"\n\
             tags: [essay, writing-process]\n\
             ---\n\
             # On writing slow\n\nbody...\n",
        );
        let meta = parse_mempool_frontmatter(&raw).expect("parses");
        assert_eq!(meta.title.as_deref(), Some("On writing slow"));
        assert_eq!(meta.status.as_deref(), Some("draft"));
        assert_eq!(meta.priority.as_deref(), Some("med"));
        assert_eq!(meta.modified.as_deref(), Some("2026-04-25"));
        assert_eq!(meta.tags, vec!["essay".to_string(), "writing-process".to_string()]);
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
}
