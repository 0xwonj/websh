//! Mempool data model — entries + the rendered model the component consumes.

use std::collections::BTreeMap;

use websh_core::mempool::{LEDGER_CATEGORIES, category_for_mempool_path};
use websh_core::domain::{MempoolFields, MempoolStatus, NodeMetadata, Priority, VirtualPath};
use crate::utils::format::{format_size, format_thousands_u32, iso_date_prefix};

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

/// Local mirror of `ledger_page::LedgerFilter` (private there) so the
/// mempool stays independently testable.
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

/// One mempool file projected from its manifest entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedMempoolFile {
    pub path: VirtualPath,
    pub meta: NodeMetadata,
    pub mempool: MempoolFields,
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
        mempool,
    } = file;

    let title = meta
        .title()
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_TITLE_FALLBACK)
        .to_string();
    let status = mempool.status;
    let priority = mempool.priority;
    let date = meta.date().map(str::to_string);
    let modified = date.clone().unwrap_or_else(|| "undated".to_string());
    let sort_key = date
        .as_deref()
        .and_then(|raw| iso_date_prefix(raw).map(|prefix| prefix.to_string()));
    let category = mempool
        .category
        .clone()
        .unwrap_or_else(|| category_for_mempool_path(&path, mempool_root));
    let kind = kind_for_category(&category);
    let desc = meta.description().unwrap_or("").to_string();
    let is_markdown = path.as_str().ends_with(".md");
    let gas = if is_markdown {
        meta.word_count()
            .map(|w| format!("~{} words", format_thousands_u32(w)))
            .unwrap_or_default()
    } else {
        meta.size_bytes()
            .map(|n| format_size(Some(n), false))
            .unwrap_or_default()
    };
    let tags = meta.tags_owned();

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
        tags,
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
    use websh_core::domain::{Fields, NodeKind, SCHEMA_VERSION};

    fn loaded(
        path: &str,
        date: Option<&str>,
        status: MempoolStatus,
        priority: Option<Priority>,
    ) -> LoadedMempoolFile {
        LoadedMempoolFile {
            path: VirtualPath::from_absolute(path).unwrap(),
            meta: NodeMetadata {
                schema: SCHEMA_VERSION,
                kind: NodeKind::Page,
                authored: Fields {
                    title: Some("untitled".to_string()),
                    date: date.map(str::to_string),
                    ..Fields::default()
                },
                derived: Fields::default(),
            },
            mempool: MempoolFields {
                status,
                priority,
                category: None,
            },
        }
    }

    #[test]
    fn build_model_orders_by_modified_desc() {
        let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
        let files = vec![
            loaded(
                "/mempool/writing/old.md",
                Some("2026-03-01"),
                MempoolStatus::Draft,
                None,
            ),
            loaded(
                "/mempool/writing/new.md",
                Some("2026-04-01"),
                MempoolStatus::Draft,
                None,
            ),
            loaded(
                "/mempool/writing/mid.md",
                Some("2026-03-15"),
                MempoolStatus::Review,
                Some(Priority::Med),
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
                Some("2026-04-01"),
                MempoolStatus::Draft,
                None,
            ),
            loaded(
                "/mempool/papers/b.md",
                Some("2026-04-02"),
                MempoolStatus::Draft,
                None,
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
                Some("2026-04-01"),
                MempoolStatus::Draft,
                None,
            ),
            loaded(
                "/mempool/writing/undated.md",
                None,
                MempoolStatus::Draft,
                None,
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
