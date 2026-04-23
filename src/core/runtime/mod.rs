//! Browser/runtime adapters that are not part of the pure filesystem engine.

use crate::core::changes::ChangeSet;
use crate::core::engine::GlobalFs;
use crate::models::RuntimeMount;
use crate::models::{DirectoryMetadata, FileMetadata, VirtualPath, WalletState};

#[path = "../env.rs"]
pub mod env;

mod commit;
mod loader;
pub(crate) mod state;
#[path = "../wallet.rs"]
pub mod wallet;

pub use commit::commit_backend;
pub use loader::{RuntimeLoad, bootstrap_runtime_load, load_runtime, reload_runtime};
pub use state::RuntimeStateSnapshot;

pub fn build_view_global_fs(
    base: &GlobalFs,
    changes: &ChangeSet,
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) -> GlobalFs {
    let mut merged = base.clone();
    populate_runtime_state(&mut merged, changes, wallet_state, runtime_state);
    crate::core::merge::apply_all_changes_to_global(&mut merged, changes);
    merged
}

fn populate_runtime_state(
    fs: &mut GlobalFs,
    changes: &ChangeSet,
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) {
    let state_root = VirtualPath::from_absolute("/state").expect("constant path");
    fs.remove_subtree(&state_root);

    let dir = |title: &str| DirectoryMetadata {
        title: title.to_string(),
        ..Default::default()
    };

    fs.upsert_directory(state_root.clone(), dir("state"));
    fs.upsert_directory(
        VirtualPath::from_absolute("/state/env").expect("constant path"),
        dir("env"),
    );
    fs.upsert_directory(
        VirtualPath::from_absolute("/state/session").expect("constant path"),
        dir("session"),
    );
    fs.upsert_directory(
        VirtualPath::from_absolute("/state/wallet").expect("constant path"),
        dir("wallet"),
    );
    fs.upsert_directory(
        VirtualPath::from_absolute("/state/drafts").expect("constant path"),
        dir("drafts"),
    );

    for (key, value) in &runtime_state.env {
        fs.upsert_file(
            VirtualPath::from_absolute(format!("/state/env/{key}")).expect("constant path"),
            value.clone(),
            FileMetadata::default(),
        );
    }

    if runtime_state.github_token_present {
        fs.upsert_file(
            VirtualPath::from_absolute("/state/session/github_token_present")
                .expect("constant path"),
            "1".to_string(),
            FileMetadata::default(),
        );
    }

    let wallet_session = if runtime_state.wallet_session {
        "1"
    } else {
        "0"
    }
    .to_string();
    fs.upsert_file(
        VirtualPath::from_absolute("/state/session/wallet_session").expect("constant path"),
        wallet_session,
        FileMetadata::default(),
    );

    let wallet_json = serde_json::to_string_pretty(wallet_state).unwrap_or_default();
    fs.upsert_file(
        VirtualPath::from_absolute("/state/wallet/connection.json").expect("constant path"),
        wallet_json,
        FileMetadata::default(),
    );

    let draft_json = serde_json::to_string_pretty(&changes.summary()).unwrap_or_default();
    fs.upsert_file(
        VirtualPath::from_absolute("/state/drafts/summary.json").expect("constant path"),
        draft_json,
        FileMetadata::default(),
    );
}

pub fn writable_mount_for_path(
    mounts: &[RuntimeMount],
    path: &VirtualPath,
) -> Option<RuntimeMount> {
    mounts
        .iter()
        .filter(|mount| mount.contains(path))
        .max_by_key(|mount| mount.root.as_str().len())
        .cloned()
}
