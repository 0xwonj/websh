//! Canonical category set for `/mempool/<category>/<slug>.md`.
//!
//! The same set drives `/ledger` filter routes; routing-specific helpers
//! live in `components::ledger_routes` and import this constant.

pub const LEDGER_CATEGORIES: &[&str] = &["writing", "projects", "papers", "talks", "misc"];
