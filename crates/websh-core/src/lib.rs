//! Pure-Rust shared library: domain types, engines, ports.
//!
//! Compiles for both `wasm32-unknown-unknown` and the host triple. Hosts
//! everything the browser app and CLI both need. Populated incrementally
//! by the migration.

pub mod attestation;
pub mod config;
pub mod content_routes;
pub mod crypto;
pub mod domain;
pub mod mempool;
pub mod theme;
pub mod utils;

#[doc(hidden)]
pub mod models {
    pub use crate::domain::*;
}
