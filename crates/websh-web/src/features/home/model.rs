use websh_core::domain::VirtualPath;
use websh_core::filesystem::GlobalFs;
use websh_core::publication::NowItem;

pub(super) const TOC_ITEMS: &[TocItem] = &[
    TocItem {
        num: "1",
        name: "about",
        href: "#/about",
        meta: "bio · cv",
        count_root: None,
    },
    TocItem {
        num: "2",
        name: "writing",
        href: "#/writing",
        meta: "",
        count_root: Some("/writing"),
    },
    TocItem {
        num: "3",
        name: "projects",
        href: "#/projects",
        meta: "",
        count_root: Some("/projects"),
    },
    TocItem {
        num: "4",
        name: "papers",
        href: "#/papers",
        meta: "",
        count_root: Some("/papers"),
    },
    TocItem {
        num: "5",
        name: "talks",
        href: "#/talks",
        meta: "",
        count_root: Some("/talks"),
    },
    TocItem {
        num: "6",
        name: "misc",
        href: "#/misc",
        meta: "",
        count_root: Some("/misc"),
    },
];

#[derive(Clone, Copy)]
pub(super) struct TocItem {
    pub(super) num: &'static str,
    pub(super) name: &'static str,
    pub(super) href: &'static str,
    pub(super) meta: &'static str,
    count_root: Option<&'static str>,
}

impl TocItem {
    pub(super) fn is_count_backed(&self) -> bool {
        self.count_root.is_some()
    }
}

use websh_core::publication::count_toc_entries;
pub(super) use websh_core::publication::recent_items_from_fs;

pub(super) fn current_homepage_date() -> String {
    let date = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date()
    )
}

pub(super) fn compact_homepage_date(date: &str) -> String {
    let mut parts = date.split('-');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(year), Some(month), Some(day), None)
            if year.len() == 4
                && month.len() == 2
                && day.len() == 2
                && year.chars().all(|ch| ch.is_ascii_digit())
                && month.chars().all(|ch| ch.is_ascii_digit())
                && day.chars().all(|ch| ch.is_ascii_digit()) =>
        {
            format!("{year}/{month}{day}")
        }
        _ => date.to_string(),
    }
}

pub(super) fn latest_now_date(items: &[NowItem]) -> Option<String> {
    items
        .iter()
        .map(|item| item.date.as_str())
        .max()
        .map(str::to_string)
}

pub(super) fn toc_item_meta(fs: &GlobalFs, item: &TocItem) -> String {
    let Some(root) = item.count_root else {
        return item.meta.to_string();
    };
    let Ok(path) = VirtualPath::from_absolute(root) else {
        return "0".to_string();
    };
    count_toc_entries(fs, &path).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn compact_homepage_date_formats_iso_date() {
        assert_eq!(compact_homepage_date("2026-04-26"), "2026/0426");
        assert_eq!(compact_homepage_date("not-a-date"), "not-a-date");
    }

    #[wasm_bindgen_test]
    fn recent_items_use_folder_category_metadata_and_content_route() {
        use websh_core::domain::{
            AuthoredMetadata, DerivedMetadata, EntryExtensions, NodeKind, NodeMetadata,
        };
        use websh_core::ports::{ScannedFile, ScannedSubtree};

        let make_meta = |date: &str, tags: &[&str]| NodeMetadata {
            kind: NodeKind::Page,
            bundle: None,
            authored: AuthoredMetadata {
                date: Some(date.to_string()),
                tags: Some(tags.iter().map(|t| t.to_string()).collect()),
                ..AuthoredMetadata::default()
            },
            derived: DerivedMetadata::default(),
        };

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "projects/websh.md".to_string(),
                    meta: make_meta("2026-04-22", &["local app", "rust"]),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "papers/tabula.md".to_string(),
                    meta: make_meta("2026-04-26", &["EuroSys 2027", "systems"]),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: Vec::new(),
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        let items = recent_items_from_fs(&fs);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "paper");
        assert_eq!(items[0].href, "#/papers/tabula");
        assert_eq!(items[0].tag, "EuroSys 2027");
        assert_eq!(items[1].kind, "project");
    }

    #[wasm_bindgen_test]
    fn toc_counts_visible_content_files_under_each_directory() {
        use websh_core::domain::{
            AuthoredMetadata, DerivedMetadata, EntryExtensions, NodeKind, NodeMetadata,
        };
        use websh_core::ports::{ScannedFile, ScannedSubtree};

        let blank = || NodeMetadata {
            kind: NodeKind::Page,
            bundle: None,
            authored: AuthoredMetadata::default(),
            derived: DerivedMetadata::default(),
        };

        let snapshot = ScannedSubtree {
            files: vec![
                ScannedFile {
                    path: "writing/hello.md".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/deep/post.html".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "papers/tabula.pdf.meta.json".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "writing/notes.toml".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "projects/websh.md".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
                ScannedFile {
                    path: "papers/tabula.pdf".to_string(),
                    meta: blank(),
                    extensions: EntryExtensions::default(),
                },
            ],
            directories: Vec::new(),
        };
        let mut fs = GlobalFs::empty();
        fs.mount_scanned_subtree(VirtualPath::root(), &snapshot)
            .expect("mount snapshot");

        assert_eq!(
            count_toc_entries(&fs, &VirtualPath::from_absolute("/writing").unwrap()),
            2
        );
        assert_eq!(
            toc_item_meta(
                &fs,
                TOC_ITEMS
                    .iter()
                    .find(|item| item.name == "projects")
                    .unwrap()
            ),
            "1"
        );
        assert_eq!(
            toc_item_meta(
                &fs,
                TOC_ITEMS.iter().find(|item| item.name == "about").unwrap()
            ),
            "bio · cv"
        );
    }
}
