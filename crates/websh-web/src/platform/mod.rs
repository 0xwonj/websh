//! Browser platform helpers and wasm glue.

pub mod asset;
pub mod breakpoints;
pub mod dom;
pub mod fetch;
pub mod redirect;
pub mod time;
#[cfg(target_arch = "wasm32")]
pub mod wasm_cleanup;

pub use asset::{BrowserAssetUrl, object_url_for_bytes};
pub use fetch::{RaceResult, fetch_content, fetch_json, race_with_timeout};
pub use time::current_timestamp;
