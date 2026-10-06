//! Pure runtime orchestration helpers shared by the web app and CLI.

use crate::domain::{
    AuthoredMetadata, DerivedMetadata, EntryExtensions, NodeKind, NodeMetadata, WalletState,
    runtime_state_root,
};
use crate::engine::filesystem::GlobalFs;

pub(crate) mod boot;
mod state;

pub use boot::{assemble_global_fs, bootstrap_global_fs, bootstrap_runtime_mount};
pub use state::RuntimeStateSnapshot;

pub fn build_runtime_overlay(
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) -> GlobalFs {
    let mut fs = GlobalFs::empty();
    let state_root = runtime_state_root().clone();

    let dir = |title: &str| NodeMetadata {
        kind: NodeKind::Directory,
        bundle: None,
        authored: AuthoredMetadata::default(),
        derived: DerivedMetadata {
            title: Some(title.to_string()),
            ..DerivedMetadata::default()
        },
    };
    let data_file = || NodeMetadata {
        kind: NodeKind::Data,
        bundle: None,
        authored: AuthoredMetadata::default(),
        derived: DerivedMetadata::default(),
    };

    fs.upsert_directory(state_root.clone(), dir("state"));
    fs.upsert_directory(state_root.join("env"), dir("env"));
    fs.upsert_directory(state_root.join("session"), dir("session"));
    fs.upsert_directory(state_root.join("wallet"), dir("wallet"));

    for (key, value) in &runtime_state.env {
        fs.upsert_file(
            state_root.join(&format!("env/{key}")),
            value.clone(),
            data_file(),
            EntryExtensions::default(),
        );
    }

    let wallet_session = if runtime_state.wallet_session {
        "1"
    } else {
        "0"
    }
    .to_string();
    fs.upsert_file(
        state_root.join("session/wallet_session"),
        wallet_session,
        data_file(),
        EntryExtensions::default(),
    );

    let wallet_json = serde_json::to_string_pretty(wallet_state).unwrap_or_default();
    fs.upsert_file(
        state_root.join("wallet/connection.json"),
        wallet_json,
        data_file(),
        EntryExtensions::default(),
    );
    fs
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::{VirtualPath, WalletState, runtime_state_root};
    use crate::filesystem::{FsView, RouteRequest, resolve_shell_route};

    #[test]
    fn runtime_view_merges_listings_without_changing_content() {
        let mut base = GlobalFs::empty();
        for path in [
            "/article.md",
            "/.websh/attestations.json",
            "/.websh/state/env/STALE",
        ] {
            base.upsert_file(
                VirtualPath::from_absolute(path).unwrap(),
                "content".to_string(),
                NodeMetadata::default(),
                EntryExtensions::default(),
            );
        }
        let runtime_state = RuntimeStateSnapshot {
            env: BTreeMap::from([("USER".to_string(), "wonj".to_string())]),
            wallet_session: true,
        };

        let overlay = build_runtime_overlay(&WalletState::Disconnected, &runtime_state);
        let system = FsView::with_runtime(&base, &overlay);

        assert!(system.exists(runtime_state_root()));
        assert!(system.exists(&VirtualPath::from_absolute("/.websh/state/env/USER").unwrap()));
        assert!(!system.exists(&runtime_state_root().join("drafts")));
        assert_eq!(
            overlay
                .read_inline_text(&runtime_state_root().join("session/wallet_session"))
                .as_deref(),
            Some("1")
        );
        let names = |path| {
            system
                .list_dir(&VirtualPath::from_absolute(path).unwrap())
                .unwrap()
                .into_iter()
                .map(|entry| entry.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names("/"), [".websh", "article.md"]);
        assert_eq!(names("/.websh"), ["state", "attestations.json"]);
        assert_eq!(names("/.websh/state/env"), ["USER"]);
        assert!(!system.exists(&runtime_state_root().join("env/STALE")));
        assert!(!base.exists(&runtime_state_root().join("env/USER")));
        assert_eq!(
            base.read_inline_text(&runtime_state_root().join("env/STALE"))
                .as_deref(),
            Some("content")
        );
        assert!(!overlay.exists(&VirtualPath::from_absolute("/article.md").unwrap()));

        let empty = GlobalFs::empty();
        let standalone = FsView::with_runtime(&empty, &overlay);
        let websh = VirtualPath::from_absolute("/.websh").unwrap();
        assert!(standalone.is_directory(&websh));
        assert_eq!(standalone.list_dir(&websh).unwrap()[0].name, "state");
        let route = resolve_shell_route(standalone, &RouteRequest::new("/websh/.websh"))
            .expect("shell navigation uses the same synthesized directories as cd");
        assert_eq!(route.node_path, websh);
        let state_meta = standalone.node_metadata(runtime_state_root()).unwrap();
        assert_eq!(state_meta.kind, NodeKind::Directory);
        assert_eq!(state_meta.authored.title, None);
        assert_eq!(state_meta.derived.title.as_deref(), Some("state"));
        let wallet_meta = standalone
            .node_metadata(&runtime_state_root().join("wallet/connection.json"))
            .unwrap();
        assert_eq!(wallet_meta.kind, NodeKind::Data);
        assert!(
            standalone
                .permissions(&websh, &WalletState::Disconnected)
                .unwrap()
                .read
        );

        let without_runtime = FsView::with_runtime(&base, &empty);
        let entries = without_runtime.list_dir(&websh).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "attestations.json");
        assert_eq!(without_runtime.child_count(&websh), Some(1));
    }
}
