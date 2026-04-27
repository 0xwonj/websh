//! Integration tests for the mempool promote flow. Pure-helper coverage:
//! exercise path mapping, commit messages, change-set shapes, and the
//! preflight error matrix without standing up a backend or Leptos runtime.
//! The async transaction is exercised manually when the live mempool repo
//! is provisioned (deferred per master §10).

use websh::components::mempool::{
    PromoteError, build_bundle_add_change_set, build_mempool_drop_change_set,
    preflight_promote_paths, promote_commit_messages, promote_target_path,
};
use websh::core::changes::ChangeType;
use websh::models::VirtualPath;

fn p(s: &str) -> VirtualPath {
    VirtualPath::from_absolute(s).unwrap()
}

#[test]
fn target_path_strips_mempool_prefix_and_preserves_category() {
    assert_eq!(
        promote_target_path(&p("/mempool/writing/foo.md")).unwrap(),
        p("/writing/foo.md"),
    );
    assert_eq!(
        promote_target_path(&p("/mempool/papers/series/foo.md")).unwrap(),
        p("/papers/series/foo.md"),
    );
}

#[test]
fn target_path_rejects_paths_outside_mempool() {
    assert!(matches!(
        promote_target_path(&p("/writing/foo.md")),
        Err(PromoteError::SourceNotInMempool(_))
    ));
}

#[test]
fn target_path_rejects_root_mempool_path() {
    assert!(matches!(
        promote_target_path(&p("/mempool")),
        Err(PromoteError::SourceNotInMempool(_))
    ));
}

#[test]
fn commit_messages_format_relative_path_without_extension() {
    let msgs = promote_commit_messages(&p("/mempool/writing/on-slow.md")).unwrap();
    assert_eq!(msgs.bundle_add, "promote: add writing/on-slow");
    assert_eq!(msgs.mempool_drop, "mempool: drop writing/on-slow (promoted)");
}

#[test]
fn bundle_add_change_set_creates_one_file_with_body_verbatim() {
    let target = p("/writing/foo.md");
    let body = "---\ntitle: foo\n---\n\nbody\n";
    let cs = build_bundle_add_change_set(&target, body);
    let entries: Vec<_> = cs.iter_all().collect();
    assert_eq!(entries.len(), 1);
    let (path, entry) = entries[0];
    assert_eq!(path, &target);
    match &entry.change {
        ChangeType::CreateFile { content, .. } => assert_eq!(content, body),
        other => panic!("expected CreateFile, got {other:?}"),
    }
}

#[test]
fn mempool_drop_change_set_deletes_one_file() {
    let source = p("/mempool/writing/foo.md");
    let cs = build_mempool_drop_change_set(&source);
    let entries: Vec<_> = cs.iter_all().collect();
    assert_eq!(entries.len(), 1);
    let (path, entry) = entries[0];
    assert_eq!(path, &source);
    assert!(matches!(entry.change, ChangeType::DeleteFile));
}

#[test]
fn preflight_returns_target_when_all_inputs_valid() {
    let target = preflight_promote_paths(
        &p("/mempool/writing/foo.md"),
        true,
        false,
        true,
        true,
        true,
    )
    .unwrap();
    assert_eq!(target, p("/writing/foo.md"));
}

#[test]
fn preflight_flags_missing_source() {
    assert!(matches!(
        preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            false,
            false,
            true,
            true,
            true
        ),
        Err(PromoteError::MempoolEntryMissing(_))
    ));
}

#[test]
fn preflight_flags_target_collision_carrying_target_path() {
    match preflight_promote_paths(&p("/mempool/writing/foo.md"), true, true, true, true, true) {
        Err(PromoteError::BundleTargetCollision(path)) => {
            assert!(path.as_str().ends_with("writing/foo.md"));
        }
        other => panic!("expected BundleTargetCollision, got {other:?}"),
    }
}

#[test]
fn preflight_flags_missing_bundle_backend() {
    assert!(matches!(
        preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            true,
            false,
            false,
            true,
            true
        ),
        Err(PromoteError::BackendMissingFor(_))
    ));
}

#[test]
fn preflight_flags_missing_mempool_backend() {
    assert!(matches!(
        preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            true,
            false,
            true,
            false,
            true
        ),
        Err(PromoteError::BackendMissingFor(_))
    ));
}

#[test]
fn preflight_flags_missing_token() {
    assert!(matches!(
        preflight_promote_paths(
            &p("/mempool/writing/foo.md"),
            true,
            false,
            true,
            true,
            false
        ),
        Err(PromoteError::TokenMissing)
    ));
}
