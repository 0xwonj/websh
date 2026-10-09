use crate::shared::components::size_summary_parts;
pub(super) use websh_core::publication::{
    LedgerEntry, LedgerFilter, LedgerModel, build_ledger_model, ledger_filter_for_route,
};
use websh_core::support::format::format_size;

pub(super) fn entry_meta_line(entry: &LedgerEntry) -> Vec<String> {
    let meta = &entry.metrics;
    meta_line_for_entry(
        size_summary_parts(
            meta.kind,
            meta.word_count(),
            meta.page_count(),
            meta.image_dimensions(),
        ),
        entry.size,
        &entry.tags,
    )
}

fn meta_line_for_entry(
    summary_parts: Vec<String>,
    size: Option<u64>,
    tags: &[String],
) -> Vec<String> {
    let mut out = summary_parts;
    if out.is_empty()
        && let Some(bytes) = size
    {
        out.push(format_size(Some(bytes), false));
    }
    out.extend(tags.iter().take(3).cloned());
    if out.is_empty() {
        out.push("content".to_string());
    }
    out
}
