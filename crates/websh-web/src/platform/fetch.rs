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
    #[error("response exceeds its allowed byte length")]
    TooLarge,
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

async fn fetch_url(url: &str) -> Result<String, FetchError> {
    let response = fetch_text(url, FETCH_TIMEOUT_MS as u32, web_sys::RequestCache::Default).await?;
    if !(200..300).contains(&response.status) {
        return Err(FetchError::HttpError(response.status));
    }
    Ok(response.body)
}

pub struct TextResponse {
    pub status: u16,
    pub retry_after: Option<u64>,
    pub body: String,
}

pub async fn fetch_text(
    url: &str,
    timeout_ms: u32,
    cache: web_sys::RequestCache,
) -> Result<TextResponse, FetchError> {
    let response = fetch_bytes(url, timeout_ms, cache).await?;
    Ok(TextResponse {
        status: response.status,
        retry_after: response.retry_after,
        body: String::from_utf8(response.body).map_err(|_| FetchError::InvalidContent)?,
    })
}

pub struct BytesResponse {
    pub status: u16,
    pub retry_after: Option<u64>,
    pub body: Vec<u8>,
}

pub async fn fetch_bytes(
    url: &str,
    timeout_ms: u32,
    cache: web_sys::RequestCache,
) -> Result<BytesResponse, FetchError> {
    fetch_bytes_bounded(url, timeout_ms, cache, 16 * 1024 * 1024).await
}

/// A single deadline and byte budget cover headers and every decoded body chunk.
pub async fn fetch_bytes_bounded(
    url: &str,
    timeout_ms: u32,
    cache: web_sys::RequestCache,
    max_bytes: usize,
) -> Result<BytesResponse, FetchError> {
    use futures_util::future::{Either, select};
    let window = web_sys::window().ok_or(FetchError::NoWindow)?;
    let abort = AbortController::new().map_err(|_| FetchError::AbortControllerFailed)?;
    let options = RequestInit::new();
    options.set_method("GET");
    options.set_mode(RequestMode::Cors);
    options.set_credentials(web_sys::RequestCredentials::Omit);
    options.set_cache(cache);
    options.set_signal(Some(&abort.signal()));
    let request = Request::new_with_str_and_init(url, &options)
        .map_err(|_| FetchError::RequestCreationFailed)?;
    let operation = Box::pin(async move {
        let response: Response = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|_| FetchError::NetworkError("fetch failed".into()))?
            .dyn_into()
            .map_err(|_| FetchError::InvalidContent)?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("Retry-After")
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok());
        let mut body = Vec::new();
        if (200..300).contains(&status)
            && let Some(stream) = response.body()
        {
            let reader = stream
                .get_reader()
                .dyn_into::<web_sys::ReadableStreamDefaultReader>()
                .map_err(|_| FetchError::ResponseReadFailed)?;
            loop {
                let item = JsFuture::from(reader.read())
                    .await
                    .map_err(|_| FetchError::ResponseReadFailed)?;
                if js_sys::Reflect::get(&item, &JsValue::from_str("done"))
                    .map_err(|_| FetchError::ResponseReadFailed)?
                    .as_bool()
                    == Some(true)
                {
                    break;
                }
                let chunk = js_sys::Reflect::get(&item, &JsValue::from_str("value"))
                    .map_err(|_| FetchError::ResponseReadFailed)?;
                let chunk = js_sys::Uint8Array::new(&chunk);
                if body.len().saturating_add(chunk.length() as usize) > max_bytes {
                    let _ = reader.cancel();
                    return Err(FetchError::TooLarge);
                }
                body.extend_from_slice(&chunk.to_vec());
            }
        }
        Ok(BytesResponse {
            status,
            retry_after,
            body,
        })
    });
    match select(
        operation,
        Box::pin(gloo_timers::future::TimeoutFuture::new(timeout_ms)),
    )
    .await
    {
        Either::Left((result, _)) => {
            if result.is_err() {
                abort.abort();
            }
            result
        }
        Either::Right(_) => {
            abort.abort();
            Err(FetchError::Timeout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen(
        inline_js = "export function stallBody() { const old = globalThis.fetch; globalThis.fetch = async () => new Response(new ReadableStream({start() {}}), {status: 200}); return () => { globalThis.fetch = old; }; }"
    )]
    extern "C" {
        fn stallBody() -> js_sys::Function;
    }

    #[wasm_bindgen_test(async)]
    async fn deadline_includes_body_after_successful_headers() {
        let restore = stallBody();
        let result = fetch_text("/test-stalled-manifest", 25, web_sys::RequestCache::NoCache).await;
        restore.call0(&JsValue::UNDEFINED).unwrap();
        assert!(matches!(result, Err(FetchError::Timeout)));
    }
}
