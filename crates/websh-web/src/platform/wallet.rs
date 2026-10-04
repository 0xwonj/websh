//! Browser wallet runtime adapter.

use js_sys::{Array, Function, Object, Promise, Reflect};
use serde::Deserialize;
use thiserror::Error;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen_futures::JsFuture;

use crate::config::WALLET_TIMEOUT_MS;
use crate::platform::fetch::{RaceResult, fetch_json, race_with_timeout};
use crate::platform::js_value_message;

#[derive(Debug, Clone, Error)]
pub enum WalletError {
    #[error("browser window not available")]
    NoWindow,
    #[error("no wallet provider detected; install a browser wallet extension")]
    NotInstalled,
    #[error("failed to create wallet request")]
    RequestCreationFailed,
    #[error("wallet request rejected: {0}")]
    RequestRejected(String),
    #[error("no account returned from wallet")]
    NoAccount,
}

fn get_ethereum() -> Result<Object, WalletError> {
    let window = web_sys::window().ok_or(WalletError::NoWindow)?;
    Reflect::get(&window, &"ethereum".into())
        .ok()
        .and_then(|v| v.dyn_into::<Object>().ok())
        .ok_or(WalletError::NotInstalled)
}

async fn request(method: &str, interactive: bool) -> Result<JsValue, WalletError> {
    let ethereum = get_ethereum()?;
    let args = Object::new();
    Reflect::set(&args, &"method".into(), &method.into())
        .map_err(|_| WalletError::RequestCreationFailed)?;
    let request = Reflect::get(&ethereum, &"request".into())
        .map_err(|_| WalletError::RequestCreationFailed)?
        .dyn_into::<Function>()
        .map_err(|_| WalletError::RequestCreationFailed)?;
    let promise: Promise = request
        .call1(&ethereum, &args)
        .map_err(|_| WalletError::RequestCreationFailed)?
        .into();
    if interactive {
        JsFuture::from(promise)
            .await
            .map_err(|error| WalletError::RequestRejected(js_value_message(&error)))
    } else {
        match race_with_timeout(promise, WALLET_TIMEOUT_MS).await {
            RaceResult::Completed(value) => Ok(value),
            RaceResult::TimedOut => Err(WalletError::RequestRejected("request timed out".into())),
            RaceResult::Error(error) => Err(WalletError::RequestRejected(error)),
        }
    }
}

pub fn is_available() -> bool {
    get_ethereum().is_ok()
}

pub async fn chain_id() -> Option<u64> {
    let value = request("eth_chainId", false).await.ok()?.as_string()?;
    u64::from_str_radix(value.trim_start_matches("0x"), 16).ok()
}

pub async fn connect() -> Result<String, WalletError> {
    let value = request("eth_requestAccounts", true).await?;
    Array::from(&value)
        .get(0)
        .as_string()
        .ok_or(WalletError::NoAccount)
}

pub async fn account() -> Option<String> {
    let value = request("eth_accounts", false).await.ok()?;
    Array::from(&value).get(0).as_string()
}

#[derive(Deserialize)]
struct EnsResponse {
    name: Option<String>,
}

pub async fn resolve_ens(address: &str) -> Option<String> {
    let url = format!("https://api.ensideas.com/ens/resolve/{address}");

    match fetch_json::<EnsResponse>(&url).await {
        Ok(response) => response.name,
        Err(_) => None,
    }
}

pub struct Listeners {
    _accounts: Listener,
    _chain: Listener,
}

impl Listeners {
    pub fn new(accounts: Listener, chain: Listener) -> Self {
        Self {
            _accounts: accounts,
            _chain: chain,
        }
    }
}

pub struct Listener {
    ethereum: Object,
    event: &'static str,
    closure: Closure<dyn Fn(JsValue)>,
}

impl Drop for Listener {
    fn drop(&mut self) {
        remove_wallet_listener(&self.ethereum, self.event, self.closure.as_ref());
    }
}

pub fn on_accounts_changed(
    callback: impl Fn(Option<String>) + 'static,
) -> Result<Listener, WalletError> {
    let ethereum = get_ethereum()?;

    let closure = Closure::wrap(Box::new(move |accounts: JsValue| {
        let account = Array::from(&accounts).get(0).as_string();
        callback(account);
    }) as Box<dyn Fn(JsValue)>);

    let on_fn = Reflect::get(&ethereum, &"on".into())
        .map_err(|_| WalletError::RequestCreationFailed)?
        .dyn_into::<Function>()
        .map_err(|_| WalletError::RequestCreationFailed)?;

    on_fn
        .call2(&ethereum, &"accountsChanged".into(), closure.as_ref())
        .map_err(|_| WalletError::RequestCreationFailed)?;

    Ok(Listener {
        ethereum,
        event: "accountsChanged",
        closure,
    })
}

pub fn on_chain_changed(callback: impl Fn(String) + 'static) -> Result<Listener, WalletError> {
    let ethereum = get_ethereum()?;

    let closure = Closure::wrap(Box::new(move |chain_id: JsValue| {
        if let Some(id) = chain_id.as_string() {
            callback(id);
        }
    }) as Box<dyn Fn(JsValue)>);

    let on_fn = Reflect::get(&ethereum, &"on".into())
        .map_err(|_| WalletError::RequestCreationFailed)?
        .dyn_into::<Function>()
        .map_err(|_| WalletError::RequestCreationFailed)?;

    on_fn
        .call2(&ethereum, &"chainChanged".into(), closure.as_ref())
        .map_err(|_| WalletError::RequestCreationFailed)?;

    Ok(Listener {
        ethereum,
        event: "chainChanged",
        closure,
    })
}

fn remove_wallet_listener(ethereum: &Object, event: &'static str, closure: &JsValue) {
    if let Ok(value) = Reflect::get(ethereum, &"removeListener".into())
        && let Ok(function) = value.dyn_into::<Function>()
    {
        let _ = function.call2(ethereum, &event.into(), closure);
    }
}
