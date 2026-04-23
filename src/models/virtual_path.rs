//! Absolute Unix-style path newtype used as the key in `ChangeSet` and storage layers.
//!
//! Constructed via `from_absolute` which enforces:
//! - non-empty
//! - begins with `/`
//!
//! No normalization (`.` / `..` / duplicate slashes) — the caller must pass a canonical path.
//! String-based subtree lookups remain available internally where relative paths are required.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct VirtualPath(String);

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    NotAbsolute(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "path is empty"),
            ParseError::NotAbsolute(s) => write!(f, "path is not absolute: {}", s),
        }
    }
}

impl std::error::Error for ParseError {}

impl VirtualPath {
    /// Return the canonical filesystem root path.
    pub fn root() -> Self {
        Self("/".to_string())
    }

    pub fn from_absolute(s: impl Into<String>) -> Result<Self, ParseError> {
        let s = s.into();
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        if !s.starts_with('/') {
            return Err(ParseError::NotAbsolute(s));
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

    /// Join a canonical relative suffix onto this absolute path.
    ///
    /// This is a structural join helper only; it does not normalize `.`, `..`,
    /// or duplicate separators.
    pub fn join(&self, suffix: &str) -> Self {
        let suffix = suffix.trim_matches('/');
        if suffix.is_empty() {
            return self.clone();
        }
        if self.is_root() {
            return Self(format!("/{}", suffix));
        }
        Self(format!("{}/{}", self.0, suffix))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_absolute_path() {
        let p = VirtualPath::from_absolute("/home/wonjae/a.md").unwrap();
        assert_eq!(p.as_str(), "/home/wonjae/a.md");
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(VirtualPath::from_absolute(""), Err(ParseError::Empty));
    }

    #[test]
    fn rejects_relative() {
        match VirtualPath::from_absolute("foo/bar") {
            Err(ParseError::NotAbsolute(s)) => assert_eq!(s, "foo/bar"),
            other => panic!("expected NotAbsolute, got {:?}", other),
        }
    }

    #[test]
    fn display_round_trips() {
        let p = VirtualPath::from_absolute("/x").unwrap();
        assert_eq!(format!("{}", p), "/x");
    }

    #[test]
    fn btreemap_orders_lexicographically() {
        use std::collections::BTreeMap;
        let mut m: BTreeMap<VirtualPath, u32> = BTreeMap::new();
        m.insert(VirtualPath::from_absolute("/b").unwrap(), 2);
        m.insert(VirtualPath::from_absolute("/a").unwrap(), 1);
        let keys: Vec<_> = m.keys().map(|k| k.as_str().to_string()).collect();
        assert_eq!(keys, vec!["/a".to_string(), "/b".to_string()]);
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
        let path = VirtualPath::root().join("site/blog");
        assert_eq!(path.as_str(), "/site/blog");
        assert_eq!(path.file_name(), Some("blog"));
        assert_eq!(path.parent().unwrap().as_str(), "/site");
    }

    #[test]
    fn starts_with_respects_segment_boundaries() {
        let prefix = VirtualPath::from_absolute("/site").unwrap();
        let child = VirtualPath::from_absolute("/site/blog/post.md").unwrap();
        let other = VirtualPath::from_absolute("/site-map").unwrap();

        assert!(child.starts_with(&prefix));
        assert!(!other.starts_with(&prefix));
        assert_eq!(child.strip_prefix(&prefix), Some("blog/post.md"));
        assert_eq!(other.strip_prefix(&prefix), None);
    }
}
