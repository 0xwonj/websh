//! Pure runtime orchestration helpers shared by the web app and CLI.

use crate::domain::{
    EntryExtensions, Fields, NodeKind, NodeMetadata, WalletState, runtime_state_root,
};
use crate::engine::filesystem::GlobalFs;

pub(crate) mod boot;
mod state;

pub use boot::{assemble_global_fs, bootstrap_global_fs, bootstrap_runtime_mount};
pub use state::RuntimeStateSnapshot;

pub fn build_view_global_fs(
    base: &GlobalFs,
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) -> GlobalFs {
    let mut merged = base.clone();
    populate_runtime_state(&mut merged, wallet_state, runtime_state);
    merged
}

fn populate_runtime_state(
    fs: &mut GlobalFs,
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) {
    let state_root = runtime_state_root().clone();
    fs.remove_subtree(&state_root);

    let dir = |title: &str| NodeMetadata {
        kind: NodeKind::Directory,
        bundle: None,
        authored: Fields {
            title: Some(title.to_string()),
            ..Fields::default()
        },
        derived: Fields::default(),
    };
    let data_file = || NodeMetadata {
        kind: NodeKind::Data,
        bundle: None,
        authored: Fields::default(),
        derived: Fields::default(),
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
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::{VirtualPath, WalletState, runtime_state_root};

    #[test]
    fn system_view_materializes_runtime_state_overlay() {
        let base = GlobalFs::empty();
        let runtime_state = RuntimeStateSnapshot {
            env: BTreeMap::from([("USER".to_string(), "wonj".to_string())]),
            wallet_session: true,
        };

        let system = build_view_global_fs(&base, &WalletState::Disconnected, &runtime_state);

        assert!(system.exists(runtime_state_root()));
        assert!(system.exists(&VirtualPath::from_absolute("/.websh/state/env/USER").unwrap()));
        assert!(!system.exists(&runtime_state_root().join("drafts")));
        assert_eq!(
            system
                .read_inline_text(&runtime_state_root().join("session/wallet_session"))
                .as_deref(),
            Some("1")
        );
    }
}
