//! Network fetching utilities with timeout support.
//!
//! Provides async fetch functions with timeout racing and caching support.

use js_sys::{Array, Promise};
use serde::{Serialize, de::DeserializeOwned};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::config::FETCH_TIMEOUT_MS;
use crate::core::error::FetchError;
use crate::utils::cache;

// =============================================================================
// Promise Racing Utilities
// =============================================================================

/// Result of a promise race with timeout.
#[derive(Debug)]
pub enum RaceResult {
    /// The promise completed before timeout.
    Completed(JsValue),
    /// Timeout occurred before promise completed.
    TimedOut,
    /// Promise rejected with an error.
    Error(String),
}

/// Race a promise against a timeout.
///
/// This is a reusable utility for implementing timeout behavior on any
/// JavaScript Promise using `Promise.race`.
///
/// # Arguments
/// * `promise` - The promise to race against timeout
/// * `timeout_ms` - Timeout duration in milliseconds
///
/// # Returns
/// * `RaceResult::Completed` if promise resolves before timeout
/// * `RaceResult::TimedOut` if timeout occurs first
/// * `RaceResult::Error` if promise rejects
pub async fn race_with_timeout(promise: Promise, timeout_ms: i32) -> RaceResult {
    let Some(window) = web_sys::window() else {
        return RaceResult::Error("Window not available".to_string());
    };

    // Create timeout promise that resolves to undefined
    let timeout_promise = Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, timeout_ms);
    });

    // Race the promises
    let race_array = Array::new();
    race_array.push(&promise);
    race_array.push(&timeout_promise);
    let race_promise = Promise::race(&race_array);

    match JsFuture::from(race_promise).await {
        Ok(result) => {
            if result.is_undefined() {
                RaceResult::TimedOut
            } else {
                RaceResult::Completed(result)
            }
        }
        Err(e) => RaceResult::Error(e.as_string().unwrap_or_else(|| "Unknown error".to_string())),
    }
}

// =============================================================================
// Fetch Functions
// =============================================================================

/// Fetch and parse JSON from a URL.
pub async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, FetchError> {
    let text = fetch_url(url).await?;
    serde_json::from_str(&text).map_err(|e| FetchError::JsonParseError(e.to_string()))
}

/// Fetch and parse JSON with sessionStorage caching.
///
/// Tries to retrieve data from session cache first. If not found,
/// fetches from network and stores in cache for the current session.
/// Cache is automatically cleared when the browser tab is closed.
pub async fn fetch_json_cached<T>(url: &str, cache_key: &str) -> Result<T, FetchError>
where
    T: DeserializeOwned + Serialize,
{
    // Try cache first
    if let Some(cached) = cache::get::<T>(cache_key) {
        return Ok(cached);
    }

    // Fetch from network
    let data = fetch_json::<T>(url).await?;

    // Store in cache (ignore errors - caching is best-effort)
    let _ = cache::set(cache_key, &data);

    Ok(data)
}

/// Fetch text content from a URL.
///
/// This is a convenience wrapper around `fetch_url` that fetches text content.
/// The caller should construct the full URL (e.g., using mount's base_url + path).
pub async fn fetch_content(url: &str) -> Result<String, FetchError> {
    fetch_url(url).await
}

/// Fetch text from a URL using the Fetch API with timeout.
///
/// Uses [`race_with_timeout`] to implement timeout behavior. If the request
/// takes longer than `FETCH_TIMEOUT_MS`, returns `FetchError::Timeout`.
async fn fetch_url(url: &str) -> Result<String, FetchError> {
    let window = web_sys::window().ok_or(FetchError::NoWindow)?;

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|_| FetchError::RequestCreationFailed)?;

    // Create fetch promise and race against timeout
    let fetch_promise = window.fetch_with_request(&request);

    match race_with_timeout(fetch_promise, FETCH_TIMEOUT_MS).await {
        RaceResult::TimedOut => Err(FetchError::Timeout),
        RaceResult::Error(msg) => Err(FetchError::NetworkError(msg)),
        RaceResult::Completed(result) => {
            let resp: Response = result.dyn_into().map_err(|_| FetchError::InvalidContent)?;

            if !resp.ok() {
                return Err(FetchError::HttpError(resp.status()));
            }

            let text = JsFuture::from(resp.text().map_err(|_| FetchError::ResponseReadFailed)?)
                .await
                .map_err(|_| FetchError::ResponseReadFailed)?;

            text.as_string().ok_or(FetchError::InvalidContent)
        }
    }
}
