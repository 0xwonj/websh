use std::fmt;

use websh_core::mempool::LEDGER_CATEGORIES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MempoolEntryPath(String);

impl MempoolEntryPath {
    pub(crate) fn parse(raw: &str) -> Result<Self, MempoolEntryPathError> {
        if raw.is_empty() {
            return Err(MempoolEntryPathError::Empty);
        }
        if raw.starts_with('/') {
            return Err(MempoolEntryPathError::Absolute);
        }
        if raw == "manifest.json" || raw.ends_with("/manifest.json") {
            return Err(MempoolEntryPathError::Reserved);
        }

        let parts: Vec<&str> = raw.split('/').collect();
        if parts.len() != 2 {
            return Err(MempoolEntryPathError::Shape);
        }
        if parts.iter().any(|part| part.is_empty()) {
            return Err(MempoolEntryPathError::EmptySegment);
        }
        if parts.iter().any(|part| matches!(*part, "." | "..")) {
            return Err(MempoolEntryPathError::Traversal);
        }
        if !LEDGER_CATEGORIES.contains(&parts[0]) {
            return Err(MempoolEntryPathError::UnknownCategory(parts[0].to_string()));
        }
        let Some(slug) = parts[1].strip_suffix(".md") else {
            return Err(MempoolEntryPathError::Extension);
        };
        if !slug_is_valid(slug) {
            return Err(MempoolEntryPathError::Slug);
        }

        Ok(Self(raw.to_string()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MempoolEntryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MempoolEntryPathError {
    Empty,
    Absolute,
    Reserved,
    Shape,
    EmptySegment,
    Traversal,
    UnknownCategory(String),
    Extension,
    Slug,
}

impl fmt::Display for MempoolEntryPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "mempool entry path is empty"),
            Self::Absolute => write!(f, "mempool entry path must be repo-relative"),
            Self::Reserved => write!(f, "mempool entry path targets a reserved file"),
            Self::Shape => write!(f, "mempool entry path must be <category>/<slug>.md"),
            Self::EmptySegment => write!(f, "mempool entry path contains an empty segment"),
            Self::Traversal => write!(f, "mempool entry path cannot contain . or .."),
            Self::UnknownCategory(category) => {
                write!(f, "unknown mempool category `{category}`")
            }
            Self::Extension => write!(f, "mempool entry path must end in .md"),
            Self::Slug => write!(
                f,
                "mempool entry slug must be lowercase ASCII letters, digits, and hyphens"
            ),
        }
    }
}

impl std::error::Error for MempoolEntryPathError {}

fn slug_is_valid(slug: &str) -> bool {
    if slug.is_empty() {
        return false;
    }
    let bytes = slug.as_bytes();
    if !bytes[0].is_ascii_alphanumeric() {
        return false;
    }
    bytes
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_entry_path() {
        assert_eq!(
            MempoolEntryPath::parse("writing/hello-world.md")
                .unwrap()
                .as_str(),
            "writing/hello-world.md"
        );
    }

    #[test]
    fn rejects_reserved_or_escaping_paths() {
        for raw in [
            "",
            "/writing/a.md",
            "manifest.json",
            "writing/../manifest.json",
            "writing//a.md",
            "writing/a.txt",
            "unknown/a.md",
            "writing/-bad.md",
        ] {
            assert!(MempoolEntryPath::parse(raw).is_err(), "{raw} should fail");
        }
    }
}
