//! Library root for the `websh` crate.
//!
//! `src/main.rs` remains the binary entrypoint (it mounts the Leptos app),
//! but all modules live here so integration tests under `tests/` can reach
//! them via `use websh::...`.

pub mod app;
pub mod components;
pub mod config;
pub mod core;
pub mod models;
pub mod utils;
