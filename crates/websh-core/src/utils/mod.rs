//! Cross-platform leaf utilities used by both the browser app and the CLI.
//!
//! Wasm-bound modules (`fetch`, `sysinfo`) are gated to `target_arch = "wasm32"`
//! so the host toolchain can compile this crate without pulling in browser
//! dependencies.

pub mod asset;
pub mod dom;
#[cfg(target_arch = "wasm32")]
pub mod fetch;
pub mod format;
pub mod ring_buffer;
#[cfg(target_arch = "wasm32")]
pub mod sysinfo;
pub mod time;
pub mod url;

pub use asset::{data_url_for_bytes, media_type_for_path, object_url_for_bytes};
#[cfg(target_arch = "wasm32")]
pub use fetch::{RaceResult, fetch_content, fetch_json, race_with_timeout};
pub use ring_buffer::RingBuffer;
pub use time::current_timestamp;
pub use url::{UrlValidation, validate_redirect_url};
