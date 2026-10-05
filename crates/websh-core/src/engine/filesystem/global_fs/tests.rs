use std::collections::HashMap;

use crate::domain::{AuthoredMetadata, DerivedMetadata, EntryExtensions, NodeKind, NodeMetadata};
use crate::ports::{ScannedDirectory, ScannedFile, ScannedSubtree};

use super::*;

fn file_meta(kind: NodeKind) -> NodeMetadata {
    NodeMetadata {
        kind,
        bundle: None,
        authored: AuthoredMetadata::default(),
        derived: DerivedMetadata::default(),
    }
}

fn dir_meta(name: &str) -> NodeMetadata {
    NodeMetadata {
        kind: NodeKind::Directory,
        bundle: None,
        authored: AuthoredMetadata {
            title: if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            },
            ..AuthoredMetadata::default()
        },
        derived: DerivedMetadata::default(),
    }
}

fn snapshot(files: &[&str], directories: &[&str]) -> ScannedSubtree {
    ScannedSubtree {
        files: files
            .iter()
            .map(|path| ScannedFile {
                path: (*path).to_string(),
                meta: file_meta(NodeKind::Asset),
                extensions: EntryExtensions::default(),
            })
            .collect(),
        directories: directories
            .iter()
            .map(|path| ScannedDirectory {
                path: (*path).to_string(),
                meta: dir_meta(path.rsplit('/').next().unwrap_or(path)),
            })
            .collect(),
    }
}

#[test]
fn mounts_scanned_subtrees_under_canonical_prefixes() {
    let mut global = GlobalFs::empty();
    let site = snapshot(&["index.html", "about.md"], &["blog"]);
    let db = snapshot(&["notes/todo.md"], &["notes"]);

    global
        .mount_scanned_subtree(VirtualPath::root(), &site)
        .unwrap();
    global
        .mount_scanned_subtree(VirtualPath::from_absolute("/db").unwrap(), &db)
        .unwrap();

    assert!(
        global
            .get_entry(&VirtualPath::from_absolute("/index.html").unwrap())
            .is_some()
    );
    assert!(
        global
            .get_entry(&VirtualPath::from_absolute("/db/notes/todo.md").unwrap())
            .is_some()
    );
}

#[test]
fn mutation_rejects_file_ancestor_without_replacing_it() {
    let mut global = GlobalFs::empty();
    let parent = VirtualPath::from_absolute("/a").unwrap();
    global.upsert_file(
        parent.clone(),
        String::new(),
        file_meta(NodeKind::Asset),
        EntryExtensions::default(),
    );

    let err = global
        .try_upsert_file(
            VirtualPath::from_absolute("/a/b.md").unwrap(),
            "child".to_string(),
            file_meta(NodeKind::Asset),
            EntryExtensions::default(),
        )
        .unwrap_err();

    assert_eq!(
        err,
        FsMutationError::ParentIsFile {
            path: parent.clone()
        }
    );
    assert!(
        global
            .get_entry(&parent)
            .is_some_and(|entry| !entry.is_directory())
    );
    assert!(
        global
            .get_entry(&VirtualPath::from_absolute("/a/b.md").unwrap())
            .is_none()
    );
    assert!(
        global
            .read_inline_text(&VirtualPath::from_absolute("/a/b.md").unwrap())
            .is_none()
    );
}

#[test]
fn refuses_to_replace_existing_directory_mountpoint() {
    let mut global = GlobalFs::empty();
    global
        .mount_scanned_subtree(
            VirtualPath::from_absolute("/db").unwrap(),
            &snapshot(&["index.md"], &[]),
        )
        .unwrap();

    let err = global
        .mount_scanned_subtree(
            VirtualPath::from_absolute("/db").unwrap(),
            &snapshot(&["other.md"], &[]),
        )
        .unwrap_err();

    assert_eq!(
        err,
        MountError::MountPointOccupied {
            path: VirtualPath::from_absolute("/db").unwrap()
        }
    );
}

#[test]
fn reserved_mount_hides_parent_fallback_content() {
    let mut global = GlobalFs::empty();
    global
        .mount_scanned_subtree(
            VirtualPath::root(),
            &snapshot(&["index.md", "mempool/root-fallback.md"], &["mempool"]),
        )
        .unwrap();
    global
        .reserve_mount_point(VirtualPath::from_absolute("/mempool").unwrap())
        .unwrap();

    assert!(global.is_directory(&VirtualPath::from_absolute("/mempool").unwrap()));
    assert!(
        global
            .get_entry(&VirtualPath::from_absolute("/mempool/root-fallback.md").unwrap())
            .is_none()
    );
}

#[test]
fn replaces_reserved_mount_point_with_scanned_subtree() {
    let mut global = GlobalFs::empty();
    global
        .mount_scanned_subtree(VirtualPath::root(), &snapshot(&["index.md"], &[]))
        .unwrap();
    let db_root = VirtualPath::from_absolute("/db").unwrap();
    global.reserve_mount_point(db_root.clone()).unwrap();
    global
        .replace_scanned_subtree(db_root.clone(), &snapshot(&["notes/todo.md"], &["notes"]))
        .unwrap();

    assert!(
        global
            .get_entry(&VirtualPath::from_absolute("/db/notes/todo.md").unwrap())
            .is_some()
    );

    assert!(global.mount_points().any(|root| root == &db_root));
}

#[test]
fn remounting_root_replaces_mount_registry() {
    let mut global = GlobalFs::empty();
    global
        .mount_subtree(
            VirtualPath::root(),
            FsEntry::Directory {
                children: HashMap::new(),
                meta: dir_meta(""),
            },
        )
        .unwrap();

    let points: Vec<_> = global
        .mount_points()
        .map(|p| p.as_str().to_string())
        .collect();
    assert_eq!(points, vec!["/"]);
}

#[test]
fn list_dir_uses_global_absolute_paths() {
    let mut global = GlobalFs::empty();
    global
        .mount_scanned_subtree(
            VirtualPath::root(),
            &snapshot(&["blog/hello.md"], &["blog"]),
        )
        .unwrap();

    let entries = global
        .list_dir(&VirtualPath::from_absolute("/blog").unwrap())
        .unwrap();

    assert_eq!(entries[0].path.as_str(), "/blog/hello.md");
}

#[test]
fn child_summary_avoids_full_dir_entry_materialization() {
    let mut global = GlobalFs::empty();
    global
        .mount_scanned_subtree(
            VirtualPath::root(),
            &snapshot(&["blog/hello.md", "blog/zeta.md"], &["blog", "blog/assets"]),
        )
        .unwrap();
    let blog = VirtualPath::from_absolute("/blog").unwrap();

    assert_eq!(global.child_count(&blog), Some(3));
    assert_eq!(
        global.child_names(&blog),
        Some(vec![
            "assets".to_string(),
            "hello.md".to_string(),
            "zeta.md".to_string()
        ])
    );
}

#[test]
fn inline_text_tracks_upserts() {
    let mut global = GlobalFs::empty();
    let path = VirtualPath::from_absolute("/new.md").unwrap();
    global.upsert_file(
        path.clone(),
        "hello".to_string(),
        file_meta(NodeKind::Page),
        EntryExtensions::default(),
    );

    assert_eq!(global.read_inline_text(&path).as_deref(), Some("hello"));
}

#[test]
fn scanned_subtree_roundtrip_is_byte_stable() {
    let golden = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/manifest_golden.json"
    ));
    let snapshot = crate::ports::parse_manifest_snapshot(golden).expect("golden parses");

    let out = crate::ports::serialize_manifest_snapshot(&snapshot).expect("serialize");

    assert_eq!(out.trim_end(), golden.trim_end());
}

#[test]
fn scanned_empty_directory_retains_metadata() {
    let mut global = GlobalFs::empty();
    let root = VirtualPath::from_absolute("/external").unwrap();
    global
        .mount_scanned_subtree(root.clone(), &snapshot(&[], &["empty"]))
        .unwrap();
    let path = root.join("empty");
    assert!(global.is_directory(&path));
    assert_eq!(global.child_count(&path), Some(0));
    assert_eq!(
        global.node_metadata(&path).unwrap().kind,
        NodeKind::Directory
    );
}

#[test]
fn recipient_read_markers_follow_current_wallet_without_write_capability() {
    use crate::domain::{AccessFilter, Recipient, WalletState};
    let mut fs = GlobalFs::empty();
    let mut meta = file_meta(NodeKind::Page);
    meta.authored.access = Some(AccessFilter {
        recipients: vec![Recipient {
            address: "0xAbC".into(),
        }],
    });
    let path = VirtualPath::from_absolute("/advisory.md").unwrap();
    fs.upsert_file(
        path.clone(),
        "public bytes".into(),
        meta,
        EntryExtensions::default(),
    );
    let entry = fs.get_entry(&path).unwrap();
    let wallet = |address: &str| WalletState::Connected {
        address: address.into(),
        ens_name: None,
        chain_id: Some(1),
    };
    assert_eq!(
        entry.permissions(&WalletState::Disconnected).to_string(),
        "----"
    );
    assert_eq!(entry.permissions(&wallet("0xabc")).to_string(), "-r--");
    assert_eq!(entry.permissions(&wallet("0xdef")).to_string(), "----");
    assert_eq!(fs.read_inline_text(&path).unwrap(), "public bytes");
}

#[test]
fn has_children_distinguishes_populated_directories() {
    let mut fs = GlobalFs::empty();
    fs.upsert_directory(VirtualPath::root().join("empty"), dir_meta("empty"));
    fs.upsert_directory(VirtualPath::root().join("docs"), dir_meta("docs"));
    fs.upsert_file(
        VirtualPath::root().join("docs/readme.md"),
        String::new(),
        file_meta(NodeKind::Page),
        EntryExtensions::default(),
    );
    for (path, expected) in [
        ("empty", false),
        ("docs", true),
        ("missing", false),
        ("docs/readme.md", false),
    ] {
        assert_eq!(
            fs.has_children(&VirtualPath::root().join(path)),
            expected,
            "{path}"
        );
    }
}
