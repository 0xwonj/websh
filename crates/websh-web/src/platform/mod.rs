//! Browser platform helpers and wasm glue.

pub mod asset;
pub mod breakpoints;
pub mod dom;
pub mod fetch;
mod js;
pub mod redirect;
pub mod time;
pub(crate) mod wallet;
pub mod wasm_cleanup;

pub use asset::{BrowserAssetError, BrowserAssetUrl, object_url_for_bytes};
pub use js::js_value_message;
