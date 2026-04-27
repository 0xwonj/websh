//! Mempool data model: entries, statuses, priorities, and the rendered model
//! that the `Mempool` component consumes.

use std::collections::BTreeMap;

use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::models::VirtualPath;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MempoolModel {
    pub filter: LedgerFilterShape,
    pub entries: Vec<MempoolEntry>,
    pub total_count: usize,
    pub counts: BTreeMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MempoolEntry {
    pub path: VirtualPath,
    pub title: String,
    pub desc: String,
    pub status: MempoolStatus,
    pub priority: Option<Priority>,
    pub kind: String,
    pub category: String,
    pub modified: String,
    pub sort_key: Option<String>,
    pub gas: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MempoolStatus {
    Draft,
    Review,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Med,
    High,
}

/// Mirror of `LedgerFilter` used by the chain page, scoped to mempool needs.
/// We do not import `LedgerFilter` directly because it is a private item of
/// `ledger_page.rs`; copying the shape here keeps the mempool independently
/// testable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerFilterShape {
    All,
    Category(String),
}

impl LedgerFilterShape {
    pub fn includes(&self, entry: &MempoolEntry) -> bool {
        match self {
            Self::All => true,
            Self::Category(category) if LEDGER_CATEGORIES.contains(&category.as_str()) => {
                entry.category == *category
            }
            Self::Category(category) => entry.path.as_str().contains(&format!("/{category}/")),
        }
    }
}
