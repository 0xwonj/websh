//! Browser app: Leptos UI compiled to wasm32-unknown-unknown. The crate
//! is wasm-only — its public surface is gated to `target_arch = "wasm32"`
//! so `cargo check --workspace` on a host triple sees an empty crate.

#![cfg(target_arch = "wasm32")]

pub mod app;
pub mod config;
pub mod features;
pub mod platform;
pub mod render;
pub mod runtime;
pub mod shared;
