//! Library root for the `websh` crate.
//!
//! `src/main.rs` remains the binary entrypoint (it mounts the Leptos app),
//! but all modules live here so integration tests under `tests/` can reach
//! them via `use websh::...`.

pub mod app;
pub mod components;
pub use websh_core::domain as models;
pub use websh_core::{config, content_routes};

pub mod crypto {
    pub use websh_core::attestation::{artifact as attestation, ledger, subject};
    pub use websh_core::crypto::{ack, eth, pgp};
}

pub mod core;
pub use websh_core::mempool;
pub mod utils;

#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
