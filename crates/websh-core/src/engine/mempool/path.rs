//! Canonical root for externally authored mempool content.

use std::sync::LazyLock;

use crate::domain::VirtualPath;

static MEMPOOL_ROOT: LazyLock<VirtualPath> =
    LazyLock::new(|| VirtualPath::from_absolute("/mempool").expect("mempool root is absolute"));

pub fn mempool_root() -> &'static VirtualPath {
    &MEMPOOL_ROOT
}

/// First directory beneath the mount root; loose files belong to `misc`.
pub fn category_for_mempool_path(path: &VirtualPath, root: &VirtualPath) -> String {
    let relative = path
        .as_str()
        .strip_prefix(root.as_str())
        .unwrap_or(path.as_str())
        .trim_start_matches('/');
    relative
        .split_once('/')
        .map(|(category, _)| category)
        .filter(|category| !category.is_empty())
        .unwrap_or("misc")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_is_the_first_directory_beneath_the_mount() {
        for (relative, expected) in [
            ("writing/example.md", "writing"),
            ("papers/series/example.md", "papers"),
            ("loose.md", "misc"),
        ] {
            assert_eq!(
                category_for_mempool_path(&mempool_root().join(relative), mempool_root()),
                expected,
            );
        }
    }
}
