//! Canonical root for externally authored mempool content.

use std::sync::LazyLock;

use crate::domain::VirtualPath;

static MEMPOOL_ROOT: LazyLock<VirtualPath> =
    LazyLock::new(|| VirtualPath::from_absolute("/mempool").expect("mempool root is absolute"));

pub fn mempool_root() -> &'static VirtualPath {
    &MEMPOOL_ROOT
}
