//! Cross-platform leaf utilities used by both the browser app and the CLI.
//!
//! Wasm-bound modules are gated to `target_arch = "wasm32"` so the host
//! toolchain can compile this crate without pulling in browser dependencies.

pub mod asset;
pub mod format;

pub use asset::{data_url_for_bytes, media_type_for_path};
