//! Shared category and path conventions for externally authored drafts.

mod categories;
mod path;

pub use categories::LEDGER_CATEGORIES;
pub use path::{category_for_mempool_path, mempool_root};
