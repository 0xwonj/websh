//! Absolute Unix-style path newtype used by filesystem and storage layers.
//!
//! Constructed via `from_absolute` which enforces canonical absolute paths.
//! String-based subtree lookups remain available internally where relative paths are required.

use std::fmt;

use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize)]
pub struct VirtualPath(String);

#[derive(Debug, PartialEq, Eq, Error)]
pub enum VirtualPathParseError {
    #[error("path is empty")]
    Empty,
    #[error("path is not absolute: {0}")]
    NotAbsolute(String),
    #[error("path contains an empty segment: {0}")]
    EmptySegment(String),
    #[error("path contains a dot segment: {0}")]
    DotSegment(String),
    #[error("path contains a parent segment: {0}")]
    ParentSegment(String),
    #[error("path contains a backslash: {0}")]
    Backslash(String),
    #[error("path contains a control character: {0}")]
    ControlCharacter(String),
}

impl VirtualPath {
    /// Return the canonical filesystem root path.
    pub fn root() -> Self {
        Self("/".to_string())
    }

    pub fn from_absolute(s: impl Into<String>) -> Result<Self, VirtualPathParseError> {
        let s = s.into();
        if s.is_empty() {
            return Err(VirtualPathParseError::Empty);
        }
        if !s.starts_with('/') {
            return Err(VirtualPathParseError::NotAbsolute(s));
        }
        if s == "/" {
            return Ok(Self(s));
        }
        if s.len() > 1 && s.ends_with('/') {
            return Err(VirtualPathParseError::EmptySegment(s));
        }
        if s.contains('\\') {
            return Err(VirtualPathParseError::Backslash(s));
        }
        if s.chars().any(char::is_control) {
            return Err(VirtualPathParseError::ControlCharacter(s));
        }
        for segment in s.split('/').skip(1) {
            if segment.is_empty() {
                return Err(VirtualPathParseError::EmptySegment(s));
            }
            match segment {
                "." => return Err(VirtualPathParseError::DotSegment(s)),
                ".." => return Err(VirtualPathParseError::ParentSegment(s)),
                _ => {}
            }
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this path is the filesystem root.
    pub fn is_root(&self) -> bool {
        self.0 == "/"
    }

    /// Iterate canonical path segments without leading empty components.
    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &str> + '_ {
        self.0.split('/').filter(|s| !s.is_empty())
    }

    /// Return the final path segment, if any.
    pub fn file_name(&self) -> Option<&str> {
        self.segments().next_back()
    }

    /// Return the parent path. `/` has no parent.
    pub fn parent(&self) -> Option<Self> {
        if self.is_root() {
            return None;
        }

        let mut parts: Vec<&str> = self.segments().collect();
        parts.pop();

        if parts.is_empty() {
            Some(Self::root())
        } else {
            Some(Self(format!("/{}", parts.join("/"))))
        }
    }

    /// Join a relative suffix onto this absolute path and keep the result canonical.
    pub fn join(&self, suffix: &str) -> Self {
        let mut parts: Vec<&str> = self.segments().collect();
        for segment in suffix.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                _ => parts.push(segment),
            }
        }
        let joined = if parts.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", parts.join("/"))
        };
        Self::from_absolute(joined).expect("join must produce a canonical absolute path")
    }

    /// Whether `self` is equal to or nested underneath `prefix`, respecting
    /// path-segment boundaries.
    pub fn starts_with(&self, prefix: &Self) -> bool {
        if prefix.is_root() {
            return true;
        }
        self.0 == prefix.0
            || self
                .0
                .strip_prefix(prefix.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
    }

    /// Strip `prefix` from `self`, returning the remaining canonical relative
    /// suffix (without a leading slash). Returns `None` when `prefix` is not
    /// an ancestor path boundary of `self`.
    pub fn strip_prefix<'a>(&'a self, prefix: &Self) -> Option<&'a str> {
        if prefix.is_root() {
            return Some(self.0.trim_start_matches('/'));
        }
        if self.0 == prefix.0 {
            return Some("");
        }
        let rest = self.0.strip_prefix(prefix.as_str())?;
        rest.strip_prefix('/')
    }
}

impl fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for VirtualPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VirtualPathVisitor;

        impl<'de> Visitor<'de> for VirtualPathVisitor {
            type Value = VirtualPath;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a canonical absolute virtual path")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                VirtualPath::from_absolute(value.to_string()).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: DeError,
            {
                VirtualPath::from_absolute(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(VirtualPathVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_and_deserialization_enforce_canonical_paths() {
        for (raw, error) in [
            ("", VirtualPathParseError::Empty),
            (
                "foo/bar",
                VirtualPathParseError::NotAbsolute("foo/bar".into()),
            ),
            ("/a//b", VirtualPathParseError::EmptySegment("/a//b".into())),
            ("/a/.", VirtualPathParseError::DotSegment("/a/.".into())),
            (
                "/a/../b",
                VirtualPathParseError::ParentSegment("/a/../b".into()),
            ),
            ("/a\\b", VirtualPathParseError::Backslash("/a\\b".into())),
            (
                "/a/\u{7}",
                VirtualPathParseError::ControlCharacter("/a/\u{7}".into()),
            ),
        ] {
            assert_eq!(VirtualPath::from_absolute(raw), Err(error), "{raw:?}");
            let json = serde_json::to_string(raw).unwrap();
            assert!(
                serde_json::from_str::<VirtualPath>(&json).is_err(),
                "{raw:?}"
            );
        }
        for raw in ["/", "/home/reader/a.md"] {
            let path = VirtualPath::from_absolute(raw).unwrap();
            let json = serde_json::to_string(&path).unwrap();
            assert_eq!(serde_json::from_str::<VirtualPath>(&json).unwrap(), path);
        }
    }

    #[test]
    fn root_helpers_work() {
        let root = VirtualPath::root();
        assert!(root.is_root());
        assert_eq!(root.file_name(), None);
        assert_eq!(root.parent(), None);
        assert_eq!(root.strip_prefix(&VirtualPath::root()), Some(""));
    }

    #[test]
    fn join_and_parent_work() {
        let path = VirtualPath::root().join("blog");
        assert_eq!(path.as_str(), "/blog");
        assert_eq!(path.file_name(), Some("blog"));
        assert_eq!(path.parent().unwrap().as_str(), "/");
    }

    #[test]
    fn join_normalizes_relative_suffix() {
        let path = VirtualPath::from_absolute("/blog")
            .unwrap()
            .join("./posts//../index.md");
        assert_eq!(path.as_str(), "/blog/index.md");
    }

    #[test]
    fn starts_with_respects_segment_boundaries() {
        let prefix = VirtualPath::from_absolute("/blog").unwrap();
        let child = VirtualPath::from_absolute("/blog/post.md").unwrap();
        let other = VirtualPath::from_absolute("/blog-map").unwrap();

        assert!(child.starts_with(&prefix));
        assert!(!other.starts_with(&prefix));
        assert_eq!(child.strip_prefix(&prefix), Some("post.md"));
        assert_eq!(other.strip_prefix(&prefix), None);
    }
}
