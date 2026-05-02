//! Browser/runtime adapters that are not part of the pure filesystem engine.

use crate::domain::changes::ChangeSet;
use crate::domain::{
    EntryExtensions, Fields, NodeKind, NodeMetadata, RuntimeMount, SCHEMA_VERSION, VirtualPath,
    WalletState,
};
use crate::filesystem::GlobalFs;

pub(crate) mod boot;
mod commit;
pub mod env;
#[cfg(target_arch = "wasm32")]
mod loader;
pub mod state;
#[cfg(target_arch = "wasm32")]
pub mod wallet;

pub use commit::commit_backend;
#[cfg(target_arch = "wasm32")]
pub use loader::{MountFailure, RuntimeLoad, bootstrap_runtime_load, load_runtime, reload_runtime};
pub use state::RuntimeStateSnapshot;

pub fn build_view_global_fs(
    base: &GlobalFs,
    changes: &ChangeSet,
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) -> GlobalFs {
    let mut merged = base.clone();
    crate::filesystem::merge::apply_all_changes_to_global(&mut merged, changes);
    populate_runtime_state(&mut merged, changes, wallet_state, runtime_state);
    merged
}

fn populate_runtime_state(
    fs: &mut GlobalFs,
    changes: &ChangeSet,
    wallet_state: &WalletState,
    runtime_state: &RuntimeStateSnapshot,
) {
    let state_root = VirtualPath::from_absolute("/.websh/state").expect("constant path");
    fs.remove_subtree(&state_root);

    let dir = |title: &str| NodeMetadata {
        schema: SCHEMA_VERSION,
        kind: NodeKind::Directory,
        authored: Fields {
            title: Some(title.to_string()),
            ..Fields::default()
        },
        derived: Fields::default(),
    };
    let data_file = || NodeMetadata {
        schema: SCHEMA_VERSION,
        kind: NodeKind::Data,
        authored: Fields::default(),
        derived: Fields::default(),
    };

    fs.upsert_directory(state_root.clone(), dir("state"));
    fs.upsert_directory(
        VirtualPath::from_absolute("/.websh/state/env").expect("constant path"),
        dir("env"),
    );
    fs.upsert_directory(
        VirtualPath::from_absolute("/.websh/state/session").expect("constant path"),
        dir("session"),
    );
    fs.upsert_directory(
        VirtualPath::from_absolute("/.websh/state/wallet").expect("constant path"),
        dir("wallet"),
    );
    fs.upsert_directory(
        VirtualPath::from_absolute("/.websh/state/drafts").expect("constant path"),
        dir("drafts"),
    );

    for (key, value) in &runtime_state.env {
        fs.upsert_file(
            VirtualPath::from_absolute(format!("/.websh/state/env/{key}")).expect("constant path"),
            value.clone(),
            data_file(),
            EntryExtensions::default(),
        );
    }

    if runtime_state.github_token_present {
        fs.upsert_file(
            VirtualPath::from_absolute("/.websh/state/session/github_token_present")
                .expect("constant path"),
            "1".to_string(),
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
        VirtualPath::from_absolute("/.websh/state/session/wallet_session").expect("constant path"),
        wallet_session,
        data_file(),
        EntryExtensions::default(),
    );

    let wallet_json = serde_json::to_string_pretty(wallet_state).unwrap_or_default();
    fs.upsert_file(
        VirtualPath::from_absolute("/.websh/state/wallet/connection.json").expect("constant path"),
        wallet_json,
        data_file(),
        EntryExtensions::default(),
    );

    let draft_json = serde_json::to_string_pretty(&changes.summary()).unwrap_or_default();
    fs.upsert_file(
        VirtualPath::from_absolute("/.websh/state/drafts/summary.json").expect("constant path"),
        draft_json,
        data_file(),
        EntryExtensions::default(),
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
