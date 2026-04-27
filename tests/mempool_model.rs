//! Integration tests for the mempool model builder. Exercises the same code
//! paths the runtime would, with synthesized inputs.

use websh::components::mempool::{
    LedgerFilterShape, LoadedMempoolFile, build_mempool_model, parse_mempool_frontmatter,
};
use websh::models::VirtualPath;

fn loaded(path: &str, body: &str) -> LoadedMempoolFile {
    let meta = parse_mempool_frontmatter(body).unwrap_or_default();
    LoadedMempoolFile {
        path: VirtualPath::from_absolute(path).unwrap(),
        meta,
        body: body.to_string(),
        byte_len: body.len(),
        is_markdown: true,
    }
}

#[test]
fn end_to_end_build_renders_mixed_categories() {
    let mempool_root = VirtualPath::from_absolute("/mempool").unwrap();
    let files = vec![
        loaded(
            "/mempool/writing/foo.md",
            "---\ntitle: foo\nstatus: draft\nmodified: 2026-04-01\n---\n# foo\n\nfoo body.\n",
        ),
        loaded(
            "/mempool/papers/bar.md",
            "---\ntitle: bar\nstatus: review\npriority: high\nmodified: 2026-04-02\n---\n# bar\n\nbar body.\n",
        ),
        loaded(
            "/mempool/talks/baz.md",
            "---\ntitle: baz\nstatus: draft\nmodified: 2026-03-10\n---\n# baz\n\nbaz body.\n",
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
                "---\ntitle: foo\nstatus: draft\nmodified: 2026-04-01\n---\n# foo\n",
            ),
            loaded(
                "/mempool/papers/bar.md",
                "---\ntitle: bar\nstatus: review\nmodified: 2026-04-02\n---\n# bar\n",
            ),
        ],
        &LedgerFilterShape::Category("writing".to_string()),
    );
    assert_eq!(writing_only.entries.len(), 1);
    assert_eq!(writing_only.total_count, 2);
}
