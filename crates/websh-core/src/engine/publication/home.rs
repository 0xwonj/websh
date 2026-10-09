use crate::domain::VirtualPath;
use crate::filesystem::{GlobalFs, content_href_for_path};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct RecentItem {
    pub kind: String,
    pub date: String,
    pub title: String,
    pub href: String,
    pub tag: String,
}

pub fn count_toc_entries(fs: &GlobalFs, root: &VirtualPath) -> usize {
    let Some(entries) = fs.list_dir(root) else {
        return 0;
    };

    entries
        .into_iter()
        .map(|entry| {
            if entry.name.starts_with('.') || entry.name.starts_with('_') {
                return 0;
            }
            if entry.is_dir {
                if fs
                    .node_metadata(&entry.path)
                    .is_some_and(|meta| meta.is_bundle())
                {
                    1
                } else {
                    count_toc_entries(fs, &entry.path)
                }
            } else if toc_countable_file(&entry.path) {
                1
            } else {
                0
            }
        })
        .sum()
}

fn toc_countable_file(path: &VirtualPath) -> bool {
    let Some(name) = path.file_name() else {
        return false;
    };
    if name.starts_with('_') {
        return false;
    }
    matches!(
        name.rsplit_once('.').map(|(_, ext)| ext),
        Some("md" | "html" | "pdf" | "link" | "app")
    )
}

pub fn recent_items_from_fs(fs: &GlobalFs) -> Vec<RecentItem> {
    let mut items = Vec::new();

    for root in ["papers", "projects", "writing", "talks"] {
        let path = VirtualPath::from_absolute(format!("/{root}")).expect("constant category path");
        collect_recent_items(fs, &path, &mut items);
    }

    items.sort_by(|left, right| {
        right
            .date
            .cmp(&left.date)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.href.cmp(&right.href))
    });
    items.truncate(6);
    items
}

fn collect_recent_items(fs: &GlobalFs, path: &VirtualPath, out: &mut Vec<RecentItem>) {
    let Some(entries) = fs.list_dir(path) else {
        return;
    };

    for entry in entries {
        if entry.is_dir {
            if fs
                .node_metadata(&entry.path)
                .is_some_and(|meta| meta.is_bundle())
            {
                collect_recent_bundle(fs, &entry.path, out);
            } else {
                collect_recent_items(fs, &entry.path, out);
            }
            continue;
        }

        let node_meta = fs.node_metadata(&entry.path);
        let Some(date) = non_empty_text(node_meta.and_then(|meta| meta.date()).map(str::to_string))
        else {
            continue;
        };
        let Some(kind) = category_label_for_path(entry.path.as_str()) else {
            continue;
        };
        let title = non_empty_text(node_meta.and_then(|meta| meta.title()).map(str::to_string))
            .unwrap_or(entry.title);
        let tag = node_meta
            .and_then(|meta| meta.tags())
            .and_then(first_tag)
            .unwrap_or_default();

        out.push(RecentItem {
            kind,
            date,
            title,
            href: content_href_for_path(entry.path.as_str()),
            tag,
        });
    }
}

fn collect_recent_bundle(fs: &GlobalFs, path: &VirtualPath, out: &mut Vec<RecentItem>) {
    let node_meta = fs.node_metadata(path);
    let Some(date) = non_empty_text(node_meta.and_then(|meta| meta.date()).map(str::to_string))
    else {
        return;
    };
    let Some(kind) = category_label_for_path(path.as_str()) else {
        return;
    };
    let fallback_title = path
        .file_name()
        .map(str::to_string)
        .unwrap_or_else(|| path.as_str().trim_matches('/').to_string());
    let title = non_empty_text(node_meta.and_then(|meta| meta.title()).map(str::to_string))
        .unwrap_or(fallback_title);
    let tag = node_meta
        .and_then(|meta| meta.tags())
        .and_then(first_tag)
        .unwrap_or_default();

    out.push(RecentItem {
        kind,
        date,
        title,
        href: content_href_for_path(path.as_str()),
        tag,
    });
}

fn non_empty_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn first_tag(tags: &[String]) -> Option<String> {
    tags.iter()
        .map(|tag| tag.trim().to_string())
        .find(|tag| !tag.is_empty())
}

fn category_label_for_path(path: &str) -> Option<String> {
    let folder = path.trim_start_matches('/').split('/').next()?;
    let label = match folder {
        "papers" => "paper",
        "projects" => "project",
        "talks" => "talk",
        "writing" => "writing",
        _ => return None,
    };
    Some(label.to_string())
}
