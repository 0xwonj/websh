//! Absolute Unix-style path newtype used as the key in `ChangeSet` and storage layers.
//!
//! Constructed via `from_absolute` which enforces:
//! - non-empty
//! - begins with `/`
//!
//! No normalization (`.` / `..` / duplicate slashes) — the caller must pass a canonical path.
//! Legacy `VirtualFs::get_entry(&str)` is intentionally left alone; this type is for new code.

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
}
