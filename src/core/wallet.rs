//! Wallet connection logic using web-sys.
//!
//! Provides EIP-1193 wallet connectivity through direct JavaScript interop via
//! Reflect API.

use js_sys::{Array, Function, Object, Promise, Reflect};
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen_futures::JsFuture;

use crate::config::WALLET_TIMEOUT_MS;
use crate::core::error::{EnvironmentError, WalletError};
use crate::utils::{RaceResult, dom, fetch_json, race_with_timeout};

/// Get the `window.ethereum` object injected by an EIP-1193 wallet.
fn get_ethereum() -> Result<Object, WalletError> {
    let window = dom::window().ok_or(WalletError::NoWindow)?;
    Reflect::get(&window, &"ethereum".into())
        .ok()
        .and_then(|v| v.dyn_into::<Object>().ok())
        .ok_or(WalletError::NotInstalled)
}

/// Helper to call ethereum.request({ method: ... })
async fn ethereum_request(method: &str) -> Result<JsValue, WalletError> {
    let ethereum = get_ethereum()?;

    // Create { method: "..." } object
    let args = Object::new();
    Reflect::set(&args, &"method".into(), &method.into())
        .map_err(|_| WalletError::RequestCreationFailed)?;

    // Get the request function
    let request = Reflect::get(&ethereum, &"request".into())
        .map_err(|_| WalletError::RequestCreationFailed)?
        .dyn_into::<Function>()
        .map_err(|_| WalletError::RequestCreationFailed)?;

    // Call ethereum.request(args)
    let promise: Promise = request
        .call1(&ethereum, &args)
        .map_err(|_| WalletError::RequestCreationFailed)?
        .into();

    JsFuture::from(promise)
        .await
        .map_err(|e| WalletError::RequestRejected(format!("{:?}", e)))
}

/// Check if an EIP-1193 wallet is installed.
pub fn is_available() -> bool {
    get_ethereum().is_ok()
}

/// Get current chain ID
pub async fn get_chain_id() -> Option<u64> {
    let result = ethereum_request("eth_chainId").await.ok()?;
    let hex_str = result.as_string()?;
    u64::from_str_radix(hex_str.trim_start_matches("0x"), 16).ok()
}

/// Convert chain ID to network name
pub fn chain_name(chain_id: u64) -> &'static str {
    match chain_id {
        1 => "Ethereum",
        11155111 => "Sepolia",
        17000 => "Holesky",
        42161 => "Arbitrum",
        10 => "Optimism",
        8453 => "Base",
        137 => "Polygon",
        56 => "BNB Chain",
        43114 => "Avalanche",
        324 => "zkSync Era",
        59144 => "Linea",
        534352 => "Scroll",
        _ => "Unknown",
    }
}

/// Request wallet connection (shows the wallet approval popup).
pub async fn connect() -> Result<String, WalletError> {
    let result = ethereum_request("eth_requestAccounts").await?;
    let accounts = Array::from(&result);

    accounts.get(0).as_string().ok_or(WalletError::NoAccount)
}

/// Get currently connected account (no popup) with timeout
pub async fn get_account() -> Option<String> {
    // Create the eth_accounts request promise
    let ethereum = get_ethereum().ok()?;

    let args = Object::new();
    Reflect::set(&args, &"method".into(), &"eth_accounts".into()).ok()?;

    let request_fn = Reflect::get(&ethereum, &"request".into())
        .ok()?
        .dyn_into::<Function>()
        .ok()?;

    let request_promise: Promise = request_fn.call1(&ethereum, &args).ok()?.into();

    // Race against timeout using shared utility
    match race_with_timeout(request_promise, WALLET_TIMEOUT_MS).await {
        RaceResult::Completed(result) => Array::from(&result).get(0).as_string(),
        RaceResult::TimedOut | RaceResult::Error(_) => None,
    }
}

/// ENS API response structure
#[derive(Deserialize)]
struct EnsResponse {
    name: Option<String>,
}

/// Resolve ENS name for an address using ENS API
pub async fn resolve_ens(address: &str) -> Option<String> {
    let url = format!("https://api.ensideas.com/ens/resolve/{}", address);

    match fetch_json::<EnsResponse>(&url).await {
        Ok(response) => response.name,
        Err(_) => None,
    }
}

/// Check if user has previously logged in.
pub fn has_session() -> bool {
    crate::core::runtime::state::has_wallet_session()
}

/// Save login session.
pub fn save_session()
-> Result<crate::core::runtime::RuntimeStateSnapshot, crate::core::error::EnvironmentError> {
    crate::core::runtime::state::set_wallet_session(true)
}

/// Clear login session.
pub fn clear_session()
-> Result<crate::core::runtime::RuntimeStateSnapshot, crate::core::error::EnvironmentError> {
    crate::core::runtime::state::set_wallet_session(false)
}

/// Outcome of a successful wallet connection initiated through the UI's
/// `connect_with_session` orchestrator.
///
/// `session_persist_error` is a soft failure: the wallet is still connected
/// for the current session, but auto-reconnect on next page load may not
/// work.
#[derive(Debug, Clone)]
pub struct ConnectOutcome {
    pub address: String,
    pub chain_id: Option<u64>,
    pub ens_name: Option<String>,
    pub session_persist_error: Option<EnvironmentError>,
}

/// Register a callback for when the connected account changes.
///
/// The callback receives `Some(address)` when an account is connected,
/// or `None` when disconnected.
///
/// # Note
/// The closure is intentionally leaked using `forget()` since this is a
/// single-page application where the listener should persist for the
/// entire lifetime of the page.
pub fn on_accounts_changed(callback: impl Fn(Option<String>) + 'static) -> Result<(), WalletError> {
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

    closure.forget();
    Ok(())
}

/// Register a callback for when the connected chain changes.
///
/// The callback receives the new chain ID as a hex string (e.g., "0x1" for mainnet).
///
/// # Note
/// The closure is intentionally leaked using `forget()` since this is a
/// single-page application where the listener should persist for the
/// entire lifetime of the page.
pub fn on_chain_changed(callback: impl Fn(String) + 'static) -> Result<(), WalletError> {
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

    closure.forget();
    Ok(())
}
