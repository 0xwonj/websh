use super::hash::{hash_len_prefixed, normalize_ack_name};

pub fn slugify_name(name: &str) -> String {
    let mut out = String::new();
    let normalized = normalize_ack_name(name);
    for ch in normalized.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let slug = out.trim_matches('-');
    let digest = receipt_filename_digest(&normalized);
    let suffix = &digest[..16];
    if slug.is_empty() {
        format!("ack-{suffix}")
    } else {
        format!("{slug}-{suffix}")
    }
}

fn receipt_filename_digest(normalized: &str) -> String {
    hash_len_prefixed(b"websh.ack.receipt.filename.v1", normalized.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
