//! Browser fetch helpers with timeout support.

use js_sys::{Array, Promise};
use serde::de::DeserializeOwned;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, Request, RequestInit, RequestMode, Response};

use crate::config::FETCH_TIMEOUT_MS;

#[derive(Debug, Clone, thiserror::Error)]
pub enum FetchError {
    #[error("browser window not available")]
    NoWindow,
    #[error("failed to create request")]
    RequestCreationFailed,
    #[error("failed to create abort controller")]
    AbortControllerFailed,
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("HTTP error: {0}")]
    HttpError(u16),
    #[error("failed to read response")]
    ResponseReadFailed,
    #[error("invalid response content")]
    InvalidContent,
    #[error("JSON parse error: {0}")]
    JsonParseError(String),
    #[error("request timed out")]
    Timeout,
}

#[derive(Debug)]
pub enum RaceResult {
    Completed(JsValue),
    TimedOut,
    Error(String),
}

pub async fn race_with_timeout(promise: Promise, timeout_ms: i32) -> RaceResult {
    let Some(window) = web_sys::window() else {
        return RaceResult::Error("Window not available".to_string());
    };

    let timeout_promise = Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, timeout_ms);
    });

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

pub async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, FetchError> {
    let text = fetch_url(url).await?;
    serde_json::from_str(&text).map_err(|e| FetchError::JsonParseError(e.to_string()))
}

pub async fn fetch_content(url: &str) -> Result<String, FetchError> {
    fetch_url(url).await
}

async fn fetch_url(url: &str) -> Result<String, FetchError> {
    let window = web_sys::window().ok_or(FetchError::NoWindow)?;

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::Cors);
    let abort = AbortController::new().map_err(|_| FetchError::AbortControllerFailed)?;
    let signal = abort.signal();
    opts.set_signal(Some(&signal));

    let request = Request::new_with_str_and_init(url, &opts)
        .map_err(|_| FetchError::RequestCreationFailed)?;

    let fetch_promise = window.fetch_with_request(&request);

    match race_with_timeout(fetch_promise, FETCH_TIMEOUT_MS).await {
        RaceResult::TimedOut => {
            abort.abort();
            Err(FetchError::Timeout)
        }
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
