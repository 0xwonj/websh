//! Integration tests for the mempool model builder. Exercises the same code
//! paths the runtime would, with synthesized inputs.

use websh::components::mempool::{LedgerFilterShape, LoadedMempoolFile, build_mempool_model};
use websh::models::{
    Fields, MempoolFields, MempoolStatus, NodeKind, NodeMetadata, Priority, SCHEMA_VERSION,
    VirtualPath,
};

fn loaded(
    path: &str,
    title: &str,
    date: &str,
    status: MempoolStatus,
    priority: Option<Priority>,
) -> LoadedMempoolFile {
    LoadedMempoolFile {
        path: VirtualPath::from_absolute(path).unwrap(),
        meta: NodeMetadata {
            schema: SCHEMA_VERSION,
            kind: NodeKind::Page,
            authored: Fields {
                title: Some(title.to_string()),
                date: Some(date.to_string()),
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
fn end_to_end_build_renders_mixed_categories() {
    let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
    let files = vec![
        loaded(
            "/mempool/writing/foo.md",
            "foo",
            "2026-04-01",
            MempoolStatus::Draft,
            None,
        ),
        loaded(
            "/mempool/papers/bar.md",
            "bar",
            "2026-04-02",
            MempoolStatus::Review,
            Some(Priority::High),
        ),
        loaded(
            "/mempool/talks/baz.md",
            "baz",
            "2026-03-10",
            MempoolStatus::Draft,
            None,
        ),
    ];

    let model = build_mempool_model(&mempool_root, files, &LedgerFilterShape::All);
    assert_eq!(model.total_count, 3);
    assert_eq!(model.entries.len(), 3);
    assert_eq!(model.entries[0].path.as_str(), "/mempool/papers/bar.md");
    assert_eq!(format!("{:?}", model.entries[0].priority), "Some(High)");
    assert_eq!(model.entries[1].path.as_str(), "/mempool/writing/foo.md");
    assert_eq!(model.entries[2].path.as_str(), "/mempool/talks/baz.md");

    let writing_only = build_mempool_model(
        &mempool_root,
        vec![
            loaded(
                "/mempool/writing/foo.md",
                "foo",
                "2026-04-01",
                MempoolStatus::Draft,
                None,
            ),
            loaded(
                "/mempool/papers/bar.md",
                "bar",
                "2026-04-02",
                MempoolStatus::Review,
                None,
            ),
        ],
        &LedgerFilterShape::Category("writing".to_string()),
    );
    assert_eq!(writing_only.entries.len(), 1);
    assert_eq!(writing_only.total_count, 2);
}
