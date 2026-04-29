//! Mempool data model: entries, statuses, priorities, and the rendered model
//! that the `Mempool` component consumes.

use std::collections::BTreeMap;

use crate::components::ledger_routes::LEDGER_CATEGORIES;
use crate::components::mempool::parse::{
    RawMempoolMeta, category_for_mempool_path, derive_gas, extract_first_paragraph,
    parse_mempool_status, parse_priority,
};
use crate::models::VirtualPath;
use crate::utils::format::iso_date_prefix;

const DEFAULT_TITLE_FALLBACK: &str = "untitled";

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

/// One file fetched from the mempool mount, ready to feed `build_mempool_model`.
#[derive(Clone, Debug)]
pub struct LoadedMempoolFile {
    pub path: VirtualPath,
    pub meta: RawMempoolMeta,
    pub body: String,
    pub byte_len: usize,
    pub is_markdown: bool,
}

pub fn build_mempool_model(
    mempool_root: &VirtualPath,
    files: Vec<LoadedMempoolFile>,
    filter: &LedgerFilterShape,
) -> MempoolModel {
    let mut all = files
        .into_iter()
        .filter_map(|file| build_entry(mempool_root, file))
        .collect::<Vec<_>>();

    let mut counts = BTreeMap::new();
    for entry in &all {
        *counts.entry(entry.category.clone()).or_default() += 1;
    }
    let total_count = all.len();

    sort_entries(&mut all);

    let entries = all
        .iter()
        .filter(|entry| filter.includes(entry))
        .cloned()
        .collect::<Vec<_>>();

    MempoolModel {
        filter: filter.clone(),
        entries,
        total_count,
        counts,
    }
}

fn build_entry(mempool_root: &VirtualPath, file: LoadedMempoolFile) -> Option<MempoolEntry> {
    let LoadedMempoolFile {
        path,
        meta,
        body,
        byte_len,
        is_markdown,
    } = file;

    let title = meta
        .title
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_TITLE_FALLBACK.to_string());
    let status = meta
        .status
        .as_deref()
        .and_then(parse_mempool_status)
        .unwrap_or(MempoolStatus::Draft);
    let priority = meta.priority.as_deref().and_then(parse_priority);
    let modified = meta
        .modified
        .clone()
        .unwrap_or_else(|| "undated".to_string());
    let sort_key = meta
        .modified
        .as_deref()
        .and_then(|raw| iso_date_prefix(raw).map(|prefix| prefix.to_string()));
    let category = category_for_mempool_path(&path, mempool_root);
    let kind = kind_for_category(&category);
    let desc = extract_first_paragraph(&body);
    let gas = derive_gas(&body, byte_len, is_markdown);

    Some(MempoolEntry {
        path,
        title,
        desc,
        status,
        priority,
        kind,
        category,
        modified,
        sort_key,
        gas,
        tags: meta.tags,
    })
}

fn kind_for_category(category: &str) -> String {
    match category {
        "writing" => "writing",
        "projects" => "project",
        "papers" => "paper",
        "talks" => "talk",
        _ => "note",
    }
    .to_string()
}

fn sort_entries(entries: &mut [MempoolEntry]) {
    entries.sort_by(|left, right| match (&left.sort_key, &right.sort_key) {
        (Some(left_key), Some(right_key)) => right_key
            .cmp(left_key)
            .then_with(|| left.path.as_str().cmp(right.path.as_str())),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.path.as_str().cmp(right.path.as_str()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::mempool::parse::*;

    fn meta(status: &str, modified: &str, priority: Option<&str>) -> RawMempoolMeta {
        RawMempoolMeta {
            title: Some("untitled".to_string()),
            category: None,
            status: Some(status.to_string()),
            priority: priority.map(str::to_string),
            modified: Some(modified.to_string()),
            tags: vec![],
        }
    }

    fn loaded(
        path: &str,
        meta: RawMempoolMeta,
        body: &str,
        byte_len: usize,
        is_markdown: bool,
    ) -> LoadedMempoolFile {
        LoadedMempoolFile {
            path: VirtualPath::from_absolute(path).unwrap(),
            meta,
            body: body.to_string(),
            byte_len,
            is_markdown,
        }
    }

    #[test]
    fn build_model_orders_by_modified_desc() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded(
                "/mempool/writing/old.md",
                meta("draft", "2026-03-01", None),
                "old",
                3,
                true,
            ),
            loaded(
                "/mempool/writing/new.md",
                meta("draft", "2026-04-01", None),
                "new",
                3,
                true,
            ),
            loaded(
                "/mempool/writing/mid.md",
                meta("review", "2026-03-15", Some("med")),
                "mid",
                3,
                true,
            ),
        ];
        let model = build_mempool_model(&mempool_root, files, &LedgerFilterShape::All);
        assert_eq!(model.entries.len(), 3);
        assert_eq!(model.entries[0].path.as_str(), "/mempool/writing/new.md");
        assert_eq!(model.entries[1].path.as_str(), "/mempool/writing/mid.md");
        assert_eq!(model.entries[2].path.as_str(), "/mempool/writing/old.md");
        assert_eq!(model.total_count, 3);
        assert_eq!(model.counts.get("writing").copied(), Some(3));
    }

    #[test]
    fn build_model_filters_by_category() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded(
                "/mempool/writing/a.md",
                meta("draft", "2026-04-01", None),
                "a",
                1,
                true,
            ),
            loaded(
                "/mempool/papers/b.md",
                meta("draft", "2026-04-02", None),
                "b",
                1,
                true,
            ),
        ];
        let model = build_mempool_model(
            &mempool_root,
            files,
            &LedgerFilterShape::Category("writing".to_string()),
        );
        assert_eq!(model.entries.len(), 1);
        assert_eq!(model.entries[0].category, "writing");
        assert_eq!(model.total_count, 2);
        assert_eq!(model.counts.get("writing").copied(), Some(1));
        assert_eq!(model.counts.get("papers").copied(), Some(1));
    }

    #[test]
    fn build_model_treats_undated_as_lowest_priority_sort() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded(
                "/mempool/writing/dated.md",
                meta("draft", "2026-04-01", None),
                "x",
                1,
                true,
            ),
            loaded(
                "/mempool/writing/undated.md",
                RawMempoolMeta {
                    title: Some("u".into()),
                    status: Some("draft".into()),
                    modified: None,
                    ..Default::default()
                },
                "y",
                1,
                true,
            ),
        ];
        let model = build_mempool_model(&mempool_root, files, &LedgerFilterShape::All);
        assert_eq!(model.entries.len(), 2);
        assert_eq!(model.entries[0].path.as_str(), "/mempool/writing/dated.md");
        assert_eq!(
            model.entries[1].path.as_str(),
            "/mempool/writing/undated.md"
        );
    }
}
