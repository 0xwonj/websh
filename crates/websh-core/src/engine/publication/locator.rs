use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::{GitHubMount, VirtualPath};

use super::ReleaseError;

/// A complete GitHub commit identifier, never a mutable ref or URL fragment.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GitCommit(String);

impl GitCommit {
    pub fn parse(value: impl Into<String>) -> Result<Self, ReleaseError> {
        let value = value.into();
        if value.len() != 40
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ReleaseError::Invalid(
                "expected a full lowercase Git commit".into(),
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GitCommit {
    type Error = ReleaseError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<GitCommit> for String {
    fn from(value: GitCommit) -> Self {
        value.0
    }
}

impl fmt::Display for GitCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Advisory discovery only. This does not grant publisher authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePointer {
    pub commit: GitCommit,
}

/// Persistable transport evidence; every restore must authenticate/validate again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSnapshot {
    pub commit: GitCommit,
    pub manifest: String,
    pub signature: Option<String>,
}

impl GitHubMount {
    /// Repository-root discovery does not use the content prefix.
    pub fn pointer_url(&self) -> String {
        format!(
            "https://raw.githubusercontent.com/{}/{}/current.json",
            self.repo(),
            encode_path(self.branch())
        )
    }

    /// Fixed-commit reads cannot select another repository or delivery origin.
    pub fn snapshot_url(&self, commit: &GitCommit, path: &str) -> Result<String, ReleaseError> {
        if path.is_empty() || path.starts_with('/') {
            return Err(ReleaseError::Invalid(
                "content URL needs a relative file path".into(),
            ));
        }
        VirtualPath::from_absolute(format!("/{path}"))
            .map_err(|error| ReleaseError::Invalid(error.to_string()))?;
        let path = if self.root().is_empty() {
            path.to_owned()
        } else {
            format!("{}/{path}", self.root())
        };
        Ok(format!(
            "https://raw.githubusercontent.com/{}/{}/{}",
            self.repo(),
            commit,
            encode_path(&path)
        ))
    }
}

fn encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{byte:02X}").expect("write to string");
        }
    }
    encoded
}
