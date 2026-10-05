use anyhow::{Context, bail};
use serde::Deserialize;
use websh_core::domain::{AuthoredMetadata, MempoolStatus, Priority};
use websh_core::mempool::LEDGER_CATEGORIES;
use websh_core::support::format::iso_date_prefix;

use crate::CliResult;

/// Authored YAML is parsed once for both the external manifest and canonical import.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Metadata {
    title: String,
    pub(super) category: Option<String>,
    #[serde(default = "draft_status")]
    pub(super) status: MempoolStatus,
    pub(super) priority: Option<Priority>,
    modified: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

pub(super) struct Draft<'a> {
    pub(super) metadata: Metadata,
    pub(super) body: &'a str,
}

impl<'a> Draft<'a> {
    pub(super) fn parse(source: &'a str) -> CliResult<Self> {
        let (yaml, body) = crate::workflows::content::frontmatter::split_yaml_frontmatter(source)
            .context("draft requires complete YAML frontmatter")?;
        let metadata: Metadata = serde_norway::from_str(yaml).context("parse draft frontmatter")?;
        metadata.validate()?;
        Ok(Self { metadata, body })
    }

    pub(super) fn canonical(&self) -> CliResult<String> {
        let yaml = serde_norway::to_string(&self.metadata.fields())
            .context("serialize canonical frontmatter")?;
        Ok(format!("---\n{yaml}---\n{}", self.body))
    }
}

impl Metadata {
    fn validate(&self) -> CliResult {
        if self.title.trim().is_empty() {
            bail!("draft title must not be empty");
        }
        if let Some(category) = &self.category
            && !LEDGER_CATEGORIES.contains(&category.as_str())
        {
            bail!("unknown draft category `{category}`");
        }
        if let Some(date) = &self.modified
            && !valid_date(date)
        {
            bail!("draft modified must be a valid YYYY-MM-DD date");
        }
        if self.tags.iter().any(|tag| tag.trim().is_empty()) {
            bail!("draft tags must not be empty");
        }
        Ok(())
    }

    pub(super) fn fields(&self) -> AuthoredMetadata {
        AuthoredMetadata {
            title: Some(self.title.clone()),
            date: self.modified.clone(),
            tags: (!self.tags.is_empty()).then(|| self.tags.clone()),
            ..AuthoredMetadata::default()
        }
    }
}

fn draft_status() -> MempoolStatus {
    MempoolStatus::Draft
}

fn valid_date(date: &str) -> bool {
    if date.len() != 10 || iso_date_prefix(date).is_none() {
        return false;
    }
    let year: u32 = date[..4].parse().unwrap_or_default();
    let month: u32 = date[5..7].parse().unwrap_or_default();
    let day: u32 = date[8..].parse().unwrap_or_default();
    let days = match month {
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    (1..=days).contains(&day)
}
