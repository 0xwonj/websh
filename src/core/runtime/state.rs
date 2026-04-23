use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::config::{USER_VAR_PREFIX, WALLET_SESSION_KEY};
use crate::core::error::EnvironmentError;
use crate::utils::dom;

const GITHUB_TOKEN_KEY: &str = "websh.gh_token";

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RuntimeStateSnapshot {
    pub env: BTreeMap<String, String>,
    pub github_token_present: bool,
    pub wallet_session: bool,
}

#[derive(Clone, Default)]
struct RuntimeState {
    env: BTreeMap<String, String>,
    github_token: Option<String>,
    wallet_session: bool,
}

impl RuntimeState {
    fn snapshot(&self) -> RuntimeStateSnapshot {
        RuntimeStateSnapshot {
            env: self.env.clone(),
            github_token_present: self.github_token.is_some(),
            wallet_session: self.wallet_session,
        }
    }
}

thread_local! {
    static RUNTIME_STATE: RefCell<Option<RuntimeState>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut RuntimeState) -> R) -> R {
    RUNTIME_STATE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let state = slot.get_or_insert_with(load_from_browser_storage);
        f(state)
    })
}

fn load_from_browser_storage() -> RuntimeState {
    let mut state = RuntimeState::default();

    if let Some(storage) = dom::local_storage() {
        let len = storage.length().unwrap_or(0);
        for idx in 0..len {
            if let Ok(Some(key)) = storage.key(idx) {
                if let Some(env_key) = key.strip_prefix(USER_VAR_PREFIX) {
                    if let Ok(Some(value)) = storage.get_item(&key) {
                        state.env.insert(env_key.to_string(), value);
                    }
                    continue;
                }

                if key == WALLET_SESSION_KEY {
                    state.wallet_session = storage
                        .get_item(WALLET_SESSION_KEY)
                        .ok()
                        .flatten()
                        .is_some();
                }
            }
        }
    }

    state.github_token = dom::session_storage()
        .and_then(|storage| storage.get_item(GITHUB_TOKEN_KEY).ok().flatten());

    state
}

pub fn snapshot() -> RuntimeStateSnapshot {
    with_state(|state| state.snapshot())
}

pub fn get_env_var(key: &str) -> Option<String> {
    with_state(|state| state.env.get(key).cloned())
}

pub fn set_env_var(key: &str, value: &str) -> Result<RuntimeStateSnapshot, EnvironmentError> {
    with_state(|state| {
        state.env.insert(key.to_string(), value.to_string());
    });

    if let Some(storage) = dom::local_storage() {
        storage
            .set_item(&format!("{USER_VAR_PREFIX}{key}"), value)
            .map_err(|_| EnvironmentError::SaveFailed)?;
    }

    Ok(snapshot())
}

pub fn unset_env_var(key: &str) -> Result<RuntimeStateSnapshot, EnvironmentError> {
    with_state(|state| {
        state.env.remove(key);
    });

    if let Some(storage) = dom::local_storage() {
        storage
            .remove_item(&format!("{USER_VAR_PREFIX}{key}"))
            .map_err(|_| EnvironmentError::RemoveFailed)?;
    }

    Ok(snapshot())
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

pub fn github_token_for_commit() -> Option<String> {
    with_state(|state| state.github_token.clone())
}

pub fn set_github_token(token: &str) -> RuntimeStateSnapshot {
    with_state(|state| {
        state.github_token = Some(token.to_string());
    });

    if let Some(storage) = dom::session_storage() {
        let _ = storage.set_item(GITHUB_TOKEN_KEY, token);
    }

    snapshot()
}

pub fn clear_github_token() -> RuntimeStateSnapshot {
    with_state(|state| {
        state.github_token = None;
    });

    if let Some(storage) = dom::session_storage() {
        let _ = storage.remove_item(GITHUB_TOKEN_KEY);
    }

    snapshot()
}

pub fn has_wallet_session() -> bool {
    with_state(|state| state.wallet_session)
}

pub fn set_wallet_session(active: bool) -> RuntimeStateSnapshot {
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

    snapshot()
}
