use anyhow::{Context, ensure};
use websh_core::mempool::LEDGER_CATEGORIES;

use crate::CliResult;

pub(super) struct EntryPath(String);

impl EntryPath {
    pub(super) fn parse(raw: &str) -> CliResult<Self> {
        let (category, filename) = raw
            .split_once('/')
            .context("entry path must be <category>/<slug>.md")?;
        ensure!(
            LEDGER_CATEGORIES.contains(&category),
            "unknown mempool category `{category}`"
        );
        let slug = filename
            .strip_suffix(".md")
            .context("entry must end in .md")?;
        ensure!(
            slug.as_bytes()
                .first()
                .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && slug
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
            "entry slug must start with a lowercase ASCII letter or digit and contain only those characters and hyphens"
        );
        Ok(Self(raw.to_string()))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EntryPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
