mod files;
pub(crate) mod frontmatter;
mod media;
mod metadata;
mod snapshot;

use files::{kind_for_content_path, route_for_content_path};
pub(crate) use snapshot::ContentSnapshot;

pub(crate) const DEFAULT_CONTENT_DIR: &str = "content";

pub(crate) fn validate_public_bytes(path: &str, bytes: &[u8]) -> crate::CliResult {
    const LFS_VERSION: &[u8] = b"version https://git-lfs.github.com/spec/v1";
    if bytes
        .split(|byte| *byte == b'\n')
        .next()
        .is_some_and(|line| line.strip_suffix(b"\r").unwrap_or(line) == LFS_VERSION)
    {
        anyhow::bail!(
            "Git LFS pointer is not publishable content: {path}; restore the actual file bytes and remove its LFS tracking rule"
        );
    }
    Ok(())
}
