use websh_core::domain::{VirtualPath, VirtualPathParseError};

/// Encode a canonical repository-relative path without changing segment boundaries.
pub fn encoded_repo_relative_path(path: &str) -> Result<String, VirtualPathParseError> {
    VirtualPath::from_absolute(format!("/{path}"))?;
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
