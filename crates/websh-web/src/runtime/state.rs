//! Browser preferences and the environment exposed by the terminal.

use std::collections::BTreeMap;

use leptos::prelude::*;
use thiserror::Error;
use wasm_bindgen::JsValue;
use websh_core::runtime::RuntimeStateSnapshot;
use websh_core::support::normalize_locale_tag;

use crate::config::{DEFAULT_LANG, LANG_ENV_KEY, USER_VAR_PREFIX, WALLET_SESSION_KEY};

pub const THEME_KEY: &str = "user.THEME";
pub const READER_SCALE_KEY: &str = "websh.reader.scale";
pub const DINO_SCORE_KEY: &str = "websh.dino.score";

#[derive(Debug, Clone, Error)]
pub enum EnvironmentError {
    #[error("localStorage not available")]
    StorageUnavailable,
    #[error("invalid variable name (use letters, numbers, underscores)")]
    InvalidVariableName,
    #[error("failed to save to localStorage")]
    SaveFailed,
    #[error("failed to remove from localStorage")]
    RemoveFailed,
}

#[derive(Clone, Copy)]
pub struct Preferences {
    pub snapshot: ReadSignal<RuntimeStateSnapshot>,
    state: RwSignal<RuntimeStateSnapshot>,
}

impl Preferences {
    pub fn new() -> Self {
        let state = RwSignal::new(load());
        Self {
            snapshot: state.read_only(),
            state,
        }
    }

    pub fn init_language(self) {
        if !self
            .state
            .with_untracked(|state| state.env.contains_key(LANG_ENV_KEY))
        {
            let value = browser_language_candidates()
                .into_iter()
                .find_map(|value| normalize_locale_tag(&value))
                .unwrap_or_else(|| DEFAULT_LANG.to_string());
            // A usable session preference does not depend on persistent storage.
            let _ = write(&format!("{USER_VAR_PREFIX}{LANG_ENV_KEY}"), &value);
            self.state.update(|state| {
                state.env.insert(LANG_ENV_KEY.into(), value);
            });
        }
    }

    pub fn set_env(self, key: &str, value: &str) -> Result<(), EnvironmentError> {
        validate_name(key)?;
        write(&format!("{USER_VAR_PREFIX}{key}"), value)?;
        self.state.update(|state| {
            state.env.insert(key.into(), value.into());
        });
        Ok(())
    }

    pub fn unset_env(self, key: &str) -> Result<(), EnvironmentError> {
        validate_name(key)?;
        remove(&format!("{USER_VAR_PREFIX}{key}"))?;
        self.state.update(|state| {
            state.env.remove(key);
        });
        Ok(())
    }

    pub fn wallet_session(self) -> bool {
        self.state.with_untracked(|state| state.wallet_session)
    }

    pub fn set_wallet_session(self, active: bool) -> Result<(), EnvironmentError> {
        // Live connection state must change even when persistence is unavailable.
        self.state.update(|state| state.wallet_session = active);
        if active {
            write(WALLET_SESSION_KEY, "1")
        } else {
            remove(WALLET_SESSION_KEY)
        }
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self::new()
    }
}

pub fn read(key: &str) -> Option<String> {
    local_storage()?.get_item(key).ok().flatten()
}

pub fn write(key: &str, value: &str) -> Result<(), EnvironmentError> {
    local_storage()
        .ok_or(EnvironmentError::StorageUnavailable)?
        .set_item(key, value)
        .map_err(|_| EnvironmentError::SaveFailed)
}

fn remove(key: &str) -> Result<(), EnvironmentError> {
    local_storage()
        .ok_or(EnvironmentError::StorageUnavailable)?
        .remove_item(key)
        .map_err(|_| EnvironmentError::RemoveFailed)
}

fn validate_name(name: &str) -> Result<(), EnvironmentError> {
    let mut chars = name.chars();
    if chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        Ok(())
    } else {
        Err(EnvironmentError::InvalidVariableName)
    }
}

fn load() -> RuntimeStateSnapshot {
    let mut env = BTreeMap::new();
    if let Some(storage) = local_storage() {
        for idx in 0..storage.length().unwrap_or(0) {
            if let Ok(Some(key)) = storage.key(idx)
                && let Some(name) = key.strip_prefix(USER_VAR_PREFIX)
                && validate_name(name).is_ok()
                && let Ok(Some(value)) = storage.get_item(&key)
            {
                env.insert(name.into(), value);
            }
        }
    }
    RuntimeStateSnapshot {
        env,
        wallet_session: read(WALLET_SESSION_KEY).as_deref() == Some("1"),
    }
}

fn browser_language_candidates() -> Vec<String> {
    let Some(navigator) = web_sys::window().map(|window| window.navigator()) else {
        return Vec::new();
    };
    let navigator = JsValue::from(navigator);
    let mut values = Vec::new();
    if let Ok(languages) = js_sys::Reflect::get(&navigator, &"languages".into())
        && js_sys::Array::is_array(&languages)
    {
        values.extend(
            js_sys::Array::from(&languages)
                .iter()
                .filter_map(|value| value.as_string()),
        );
    }
    if let Ok(language) = js_sys::Reflect::get(&navigator, &"language".into())
        && let Some(language) = language.as_string()
    {
        values.push(language);
    }
    values
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}
