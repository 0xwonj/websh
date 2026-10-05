use super::metadata::SourceMetadata;
use anyhow::Context;

use crate::CliResult;

/// Split a markdown body into `(yaml_str, body_after_fence)` if it opens
/// with a YAML frontmatter block. Recognizes both LF and CRLF line
/// endings, and anchors the closing `---` fence to the start of a line
/// so an inline `---` in the body content can't false-close the block.
pub(crate) fn split_yaml_frontmatter(body: &str) -> Option<(&str, &str)> {
    let after_open = body
        .strip_prefix("---\n")
        .or_else(|| body.strip_prefix("---\r\n"))?;
    let mut offset = 0;
    for line in after_open.split_inclusive('\n') {
        if line.trim_end_matches(['\r', '\n', ' ', '\t']) == "---" {
            return Some((&after_open[..offset], &after_open[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

pub(crate) fn parse_yaml_frontmatter(body: &str) -> CliResult<Option<SourceMetadata>> {
    let Some((yaml, _)) = split_yaml_frontmatter(body) else {
        if body.starts_with("---\n") || body.starts_with("---\r\n") {
            anyhow::bail!("frontmatter is missing its closing fence");
        }
        return Ok(None);
    };
    parse_frontmatter_fields(yaml).map(Some)
}

fn parse_frontmatter_fields(yaml: &str) -> CliResult<SourceMetadata> {
    if yaml.trim().is_empty() {
        return Ok(SourceMetadata::default());
    }
    serde_norway::from_str(yaml).context("frontmatter YAML parse")
}

pub(crate) fn strip_yaml_frontmatter(body: &str) -> &str {
    split_yaml_frontmatter(body)
        .map(|(_, rest)| rest)
        .unwrap_or(body)
}

#[cfg(test)]
mod tests {
    use websh_core::domain::NodeKind;

    use super::*;

    #[test]
    fn parse_yaml_frontmatter_deserializes_supported_metadata() {
        let body = r#"---
title: "1984"
kind: page
description: |
  First line
  Second line
date: 2026-05-03
tags:
  - rust
  - "2024"
links:
  - label: Paper
    url: https://eprint.iacr.org/2026/001
    kind: paper
access:
  recipients:
    - address: "0xabc"
---
# Body
"#;

        let fields = parse_yaml_frontmatter(body)
            .expect("frontmatter parses")
            .expect("frontmatter exists");

        assert_eq!(fields.authored.title.as_deref(), Some("1984"));
        assert_eq!(fields.kind, Some(NodeKind::Page));
        assert_eq!(
            fields.authored.description.as_deref(),
            Some("First line\nSecond line\n")
        );
        assert_eq!(fields.authored.date.as_deref(), Some("2026-05-03"));
        assert_eq!(
            fields.authored.tags.as_deref(),
            Some(["rust".to_string(), "2024".to_string()].as_slice())
        );
        let links = fields.authored.links.as_deref().expect("links parsed");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].label, "Paper");
        assert_eq!(links[0].url, "https://eprint.iacr.org/2026/001");
        assert_eq!(links[0].kind.as_deref(), Some("paper"));
        assert_eq!(
            fields
                .authored
                .access
                .as_ref()
                .and_then(|access| access.recipients.first())
                .map(|recipient| recipient.address.as_str()),
            Some("0xabc")
        );
    }

    #[test]
    fn parse_yaml_frontmatter_ignores_bodies_without_frontmatter() {
        assert!(
            parse_yaml_frontmatter("# Body\n")
                .expect("body without frontmatter is valid")
                .is_none()
        );
    }

    #[test]
    fn parse_yaml_frontmatter_reports_yaml_errors_with_context() {
        let err = parse_yaml_frontmatter("---\nunknown: value\n---\n")
            .expect_err("unknown fields are rejected");

        assert_eq!(err.to_string(), "frontmatter YAML parse");
        assert!(format!("{err:#}").contains("unknown field"));
        for field in ["title: 1984", "tags: [zk, 2024]"] {
            let err = parse_yaml_frontmatter(&format!("---\n{field}\n---\n"))
                .expect_err("text fields require YAML strings");
            assert!(format!("{err:#}").contains("invalid type"));
        }
    }
}
