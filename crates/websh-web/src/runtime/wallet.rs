//! Wallet connection lifecycle. Only this owner can publish wallet state.

use crate::platform::wallet as provider;
#[cfg(test)]
mod tests;

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;
use websh_core::domain::WalletState;

use super::state::{EnvironmentError, Preferences};
pub use provider::WalletError;

#[derive(Debug, Clone)]
pub struct Connection {
    pub address: String,
    pub chain_id: Option<u64>,
    pub ens_name: Option<String>,
    pub persistence_error: Option<EnvironmentError>,
}

#[derive(Clone, Copy)]
pub struct Wallet {
    pub state: ReadSignal<WalletState>,
    value: RwSignal<WalletState>,
    request: StoredValue<u64>,
    chain_revision: StoredValue<u64>,
    listeners: StoredValue<Option<provider::Listeners>, LocalStorage>,
    preferences: Preferences,
}

impl Wallet {
    pub fn new(preferences: Preferences) -> Self {
        let value = RwSignal::new(WalletState::Disconnected);
        Self {
            state: value.read_only(),
            value,
            preferences,
            request: StoredValue::new(0),
            chain_revision: StoredValue::new(0),
            listeners: StoredValue::new_local(None),
        }
    }

    pub fn can_restore(self) -> bool {
        provider::is_available()
            && self.preferences.wallet_session()
            && self
                .value
                .with_untracked(|state| matches!(state, WalletState::Disconnected))
    }

    pub async fn connect(self) -> Result<Option<Connection>, WalletError> {
        if !provider::is_available() {
            return Err(WalletError::NotInstalled);
        }
        let request = self.begin();
        let address = match provider::connect().await {
            Ok(address) => address,
            Err(error) => {
                if !self.is_current(request) {
                    return Ok(None);
                }
                self.value.set(WalletState::Disconnected);
                return Err(error);
            }
        };
        Ok(self.finish(request, address).await)
    }

    pub async fn restore(self) -> Option<Connection> {
        if !self.can_restore() {
            return None;
        }
        let request = self.begin();
        match provider::account().await {
            Some(address) => self.finish(request, address).await,
            None => {
                if self.is_current(request) {
                    let _ = self.disconnect();
                }
                None
            }
        }
    }

    pub fn disconnect(self) -> Result<(), EnvironmentError> {
        self.advance();
        self.value.set(WalletState::Disconnected);
        self.preferences.set_wallet_session(false)
    }

    pub fn install_listeners(self) {
        if self.listeners.with_value(Option::is_some) {
            return;
        }
        let accounts =
            match provider::on_accounts_changed(move |address| self.account_changed(address)) {
                Ok(listener) => listener,
                Err(error) => {
                    leptos::logging::warn!("wallet: {error}");
                    return;
                }
            };
        let chain = match provider::on_chain_changed(move |value| {
            let chain = u64::from_str_radix(value.trim_start_matches("0x"), 16).ok();
            self.chain_changed(chain);
        }) {
            Ok(listener) => listener,
            Err(error) => {
                leptos::logging::warn!("wallet: {error}");
                return;
            }
        };
        self.listeners
            .set_value(Some(provider::Listeners::new(accounts, chain)));
    }

    fn advance(self) -> u64 {
        let request = self.request.get_value().wrapping_add(1);
        self.request.set_value(request);
        request
    }

    fn begin(self) -> u64 {
        let request = self.advance();
        self.value.set(WalletState::Connecting);
        request
    }

    fn is_current(self, request: u64) -> bool {
        self.request.get_value() == request
    }

    fn publish_account(self, request: u64, address: &str) -> bool {
        if !self.is_current(request) {
            return false;
        }
        self.value.set(WalletState::Connected {
            address: address.into(),
            chain_id: None,
            ens_name: None,
        });
        true
    }

    async fn finish(self, request: u64, address: String) -> Option<Connection> {
        if !self.publish_account(request, &address) {
            return None;
        }
        let persistence_error = self.preferences.set_wallet_session(true).err();
        self.enrich(request, &address).await;
        if !self.is_current(request) {
            return None;
        }
        self.value.with_untracked(|state| match state {
            WalletState::Connected {
                address,
                chain_id,
                ens_name,
            } => Some(Connection {
                address: address.clone(),
                chain_id: *chain_id,
                ens_name: ens_name.clone(),
                persistence_error,
            }),
            _ => None,
        })
    }

    async fn enrich(self, request: u64, address: &str) {
        let revision = self.chain_revision.get_value();
        let chain_id = provider::chain_id().await;
        if !self.is_current(request) {
            return;
        }
        if self.chain_revision.get_value() == revision {
            self.value.update(|state| {
                if let WalletState::Connected {
                    chain_id: current, ..
                } = state
                {
                    *current = chain_id;
                }
            });
        }
        self.resolve_name(request, address).await;
    }

    async fn resolve_name(self, request: u64, address: &str) {
        let name = provider::resolve_ens(address).await;
        self.publish_name(request, address, name);
    }

    fn publish_name(self, request: u64, address: &str, name: Option<String>) {
        if !self.is_current(request) {
            return;
        }
        self.value.update(|state| {
            if let WalletState::Connected {
                address: current,
                ens_name,
                ..
            } = state
                && current == address
            {
                *ens_name = name;
            }
        });
    }

    fn account_changed(self, address: Option<String>) {
        if self
            .value
            .with_untracked(|state| matches!(state, WalletState::Disconnected))
        {
            return;
        }
        let Some(address) = address else {
            let _ = self.disconnect();
            return;
        };
        let request = self.advance();
        self.publish_account(request, &address);
        let _ = self.preferences.set_wallet_session(true);
        spawn_local(async move {
            self.enrich(request, &address).await;
        });
    }

    fn chain_changed(self, chain: Option<u64>) {
        self.chain_revision
            .update_value(|revision| *revision = revision.wrapping_add(1));
        self.value.update(|state| {
            if let WalletState::Connected { chain_id, .. } = state {
                *chain_id = chain;
            }
        });
    }
}
