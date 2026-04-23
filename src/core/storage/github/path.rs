pub fn normalize_repo_prefix(prefix: &str) -> Result<String, String> {
    let normalized = prefix.trim_matches('/');
    validate_repo_relative_path(normalized, true)?;
    Ok(normalized.to_string())
}

pub fn validate_repo_relative_path(path: &str, allow_empty: bool) -> Result<(), String> {
    if path.is_empty() {
        return if allow_empty {
            Ok(())
        } else {
            Err("path must not be empty".to_string())
        };
    }
    if path.starts_with('/') {
        return Err(format!("path must be repo-relative: {path}"));
    }
    if path.contains('\\') {
        return Err(format!("path must use forward slashes only: {path}"));
    }
    for segment in path.split('/') {
        if segment.is_empty() {
            return Err(format!("path contains an empty segment: {path}"));
        }
        if segment == "." || segment == ".." {
            return Err(format!("path contains traversal segment: {path}"));
        }
        if segment.chars().any(char::is_control) {
            return Err(format!("path contains a control character: {path}"));
        }
    }
    Ok(())
}

pub fn prefixed_repo_path(prefix: &str, path: &str) -> Result<String, String> {
    let prefix = normalize_repo_prefix(prefix)?;
    let path = path.trim_start_matches('/');
    validate_repo_relative_path(path, false)?;
    if prefix.is_empty() {
        Ok(path.to_string())
    } else {
        Ok(format!("{prefix}/{path}"))
    }
}

pub fn encoded_repo_relative_path(path: &str, allow_empty: bool) -> Result<String, String> {
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

    #[test]
    fn accepts_empty_prefix_and_tilde_prefix() {
        assert_eq!(normalize_repo_prefix("").unwrap(), "");
        assert_eq!(normalize_repo_prefix("/~/").unwrap(), "~");
        assert_eq!(
            prefixed_repo_path("~", "manifest.json").unwrap(),
            "~/manifest.json"
        );
    }

    #[test]
    fn rejects_ambiguous_repo_paths() {
        for path in ["/abs", "a//b", "a/./b", "a/../b", r"a\b", "a/\n/b"] {
            assert!(
                validate_repo_relative_path(path, false).is_err(),
                "{path:?} should reject"
            );
        }
    }

    #[test]
    fn encodes_url_segments_without_encoding_slashes() {
        assert_eq!(
            encoded_repo_relative_path("dir/file #1.md", false).unwrap(),
            "dir/file%20%231.md"
        );
    }
}
