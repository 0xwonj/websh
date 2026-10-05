use anyhow::Context;
use websh_core::domain::Fields;

use crate::CliResult;

/// Split a markdown body into `(yaml_str, body_after_fence)` if it opens
/// with a YAML frontmatter block. Recognizes both LF and CRLF line
/// endings, and anchors the closing `---` fence to the start of a line
/// so an inline `---` in the body content can't false-close the block.
fn split_yaml_frontmatter(body: &str) -> Option<(&str, &str)> {
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

pub(crate) fn parse_yaml_frontmatter(body: &str) -> CliResult<Option<Fields>> {
    let Some((yaml, _)) = split_yaml_frontmatter(body) else {
        if body.starts_with("---\n") || body.starts_with("---\r\n") {
            anyhow::bail!("frontmatter is missing its closing fence");
        }
        return Ok(None);
    };
    parse_frontmatter_fields(yaml).map(Some)
}

fn parse_frontmatter_fields(yaml: &str) -> CliResult<Fields> {
    if yaml.trim().is_empty() {
        return Ok(Fields::default());
    }
    let fields = serde_norway::from_str(yaml).context("frontmatter YAML parse")?;
    super::metadata::validate_authored(&fields)?;
    Ok(fields)
}

pub(crate) fn strip_yaml_frontmatter(body: &str) -> &str {
    split_yaml_frontmatter(body)
        .map(|(_, rest)| rest)
        .unwrap_or(body)
}

#[cfg(test)]
mod tests {
    use websh_core::domain::{NodeKind, RendererKind, TrustLevel};

    use super::*;

    #[test]
    fn parse_yaml_frontmatter_deserializes_supported_metadata() {
        let body = r#"---
title: A note
kind: page
renderer: markdown_page
description: |
  First line
  Second line
date: 2026-05-03
tags:
  - rust
  - yaml
links:
  - label: Paper
    url: https://eprint.iacr.org/2026/001
    kind: paper
trust: trusted
access:
  recipients:
    - address: "0xabc"
---
# Body
"#;

        let fields = parse_yaml_frontmatter(body)
            .expect("frontmatter parses")
            .expect("frontmatter exists");

        assert_eq!(fields.title.as_deref(), Some("A note"));
        assert_eq!(fields.kind, Some(NodeKind::Page));
        assert_eq!(fields.renderer, Some(RendererKind::MarkdownPage));
        assert_eq!(
            fields.description.as_deref(),
            Some("First line\nSecond line\n")
        );
        assert_eq!(fields.date.as_deref(), Some("2026-05-03"));
        assert_eq!(
            fields.tags.as_deref(),
            Some(["rust".to_string(), "yaml".to_string()].as_slice())
        );
        let links = fields.links.as_deref().expect("links parsed");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].label, "Paper");
        assert_eq!(links[0].url, "https://eprint.iacr.org/2026/001");
        assert_eq!(links[0].kind.as_deref(), Some("paper"));
        assert_eq!(fields.trust, Some(TrustLevel::Trusted));
        assert_eq!(
            fields
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
    }
}
