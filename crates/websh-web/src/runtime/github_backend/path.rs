//! Repository-relative URL path validation and encoding.

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RepoPathError {
    #[error("path must not be empty")]
    Empty,
    #[error("path must be repo-relative: {path}")]
    Absolute { path: String },
    #[error("path must use forward slashes only: {path}")]
    Backslash { path: String },
    #[error("path contains an empty segment: {path}")]
    EmptySegment { path: String },
    #[error("path contains traversal segment: {path}")]
    Traversal { path: String },
    #[error("path contains a control character: {path}")]
    ControlCharacter { path: String },
}

pub fn normalize_repo_prefix(prefix: &str) -> Result<String, RepoPathError> {
    let normalized = prefix.trim_matches('/');
    validate_repo_relative_path(normalized, true)?;
    Ok(normalized.to_string())
}

pub fn validate_repo_relative_path(path: &str, allow_empty: bool) -> Result<(), RepoPathError> {
    if path.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err(RepoPathError::Empty)
        };
    }
    if path.starts_with('/') {
        return Err(RepoPathError::Absolute {
            path: path.to_string(),
        });
    }
    if path.contains('\\') {
        return Err(RepoPathError::Backslash {
            path: path.to_string(),
        });
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(RepoPathError::EmptySegment {
                path: path.to_string(),
            });
        }
        if segment == "." || segment == ".." {
            return Err(RepoPathError::Traversal {
                path: path.to_string(),
            });
        }
        if segment.chars().any(char::is_control) {
            return Err(RepoPathError::ControlCharacter {
                path: path.to_string(),
            });
        }
    }
    Ok(())
}

pub fn encoded_repo_relative_path(path: &str, allow_empty: bool) -> Result<String, RepoPathError> {
    validate_repo_relative_path(path, allow_empty)?;
    Ok(path
        .split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/"))
}

fn percent_encode_segment(segment: &str) -> String {
    let mut out = String::new();
    for byte in segment.as_bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(*byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn accepts_empty_prefix_and_tilde_prefix() {
        assert_eq!(normalize_repo_prefix("").unwrap(), "");
        assert_eq!(normalize_repo_prefix("/~/").unwrap(), "~");
    }

    #[wasm_bindgen_test]
    fn rejects_ambiguous_repo_paths() {
        for path in ["/abs", "a//b", "a/./b", "a/../b", r"a\b", "a/\n/b"] {
            assert!(
                validate_repo_relative_path(path, false).is_err(),
                "{path:?} should reject"
            );
        }
    }

    #[wasm_bindgen_test]
    fn encodes_url_segments_without_encoding_slashes() {
        assert_eq!(
            encoded_repo_relative_path("dir/file #1.md", false).unwrap(),
            "dir/file%20%231.md"
        );
    }
}
