use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::config::{USER_VAR_PREFIX, WALLET_SESSION_KEY};
use crate::core::error::EnvironmentError;
use crate::utils::dom;

const GITHUB_TOKEN_KEY: &str = "websh.gh_token";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeStateSnapshot {
    pub env: BTreeMap<String, String>,
    pub github_token: Option<String>,
    pub wallet_session: bool,
}

thread_local! {
    static RUNTIME_STATE: RefCell<Option<RuntimeStateSnapshot>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut RuntimeStateSnapshot) -> R) -> R {
    RUNTIME_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.get_or_insert_with(load_from_browser_storage);
        f(state)
    })
}

fn load_from_browser_storage() -> RuntimeStateSnapshot {
    let mut snapshot = RuntimeStateSnapshot::default();

    if let Some(storage) = dom::local_storage() {
        let len = storage.length().unwrap_or(0);
        for idx in 0..len {
            if let Ok(Some(key)) = storage.key(idx) {
                if let Some(env_key) = key.strip_prefix(USER_VAR_PREFIX) {
                    if let Ok(Some(value)) = storage.get_item(&key) {
                        snapshot.env.insert(env_key.to_string(), value);
                    }
                    continue;
                }

                if key == WALLET_SESSION_KEY {
                    snapshot.wallet_session = storage
                        .get_item(WALLET_SESSION_KEY)
                        .ok()
                        .flatten()
                        .is_some();
                }
            }
        }
    }

    snapshot.github_token = dom::session_storage()
        .and_then(|storage| storage.get_item(GITHUB_TOKEN_KEY).ok().flatten());

    snapshot
}

pub fn snapshot() -> RuntimeStateSnapshot {
    with_state(|state| state.clone())
}

pub fn get_env_var(key: &str) -> Option<String> {
    with_state(|state| state.env.get(key).cloned())
}

pub fn set_env_var(key: &str, value: &str) -> Result<(), EnvironmentError> {
    with_state(|state| {
        state.env.insert(key.to_string(), value.to_string());
    });

    if let Some(storage) = dom::local_storage() {
        storage
            .set_item(&format!("{USER_VAR_PREFIX}{key}"), value)
            .map_err(|_| EnvironmentError::SaveFailed)?;
    }

    Ok(())
}

pub fn unset_env_var(key: &str) -> Result<(), EnvironmentError> {
    with_state(|state| {
        state.env.remove(key);
    });

    if let Some(storage) = dom::local_storage() {
        storage
            .remove_item(&format!("{USER_VAR_PREFIX}{key}"))
            .map_err(|_| EnvironmentError::RemoveFailed)?;
    }

    Ok(())
}

pub fn all_env_vars() -> Vec<(String, String)> {
    with_state(|state| {
        state
            .env
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    })
}

pub fn get_github_token() -> Option<String> {
    with_state(|state| state.github_token.clone())
}

pub fn set_github_token(token: &str) {
    with_state(|state| {
        state.github_token = Some(token.to_string());
    });

    if let Some(storage) = dom::session_storage() {
        let _ = storage.set_item(GITHUB_TOKEN_KEY, token);
    }
}

pub fn clear_github_token() {
    with_state(|state| {
        state.github_token = None;
    });

    if let Some(storage) = dom::session_storage() {
        let _ = storage.remove_item(GITHUB_TOKEN_KEY);
    }
}

pub fn has_wallet_session() -> bool {
    with_state(|state| state.wallet_session)
}

pub fn set_wallet_session(active: bool) {
    with_state(|state| {
        state.wallet_session = active;
    });

    if let Some(storage) = dom::local_storage() {
        if active {
            let _ = storage.set_item(WALLET_SESSION_KEY, "1");
        } else {
            let _ = storage.remove_item(WALLET_SESSION_KEY);
        }
    }
}
