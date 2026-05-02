//! Browser app: Leptos UI compiled to wasm32-unknown-unknown. The crate
//! is wasm-only — its public surface is gated to `target_arch = "wasm32"`
//! so `cargo check --workspace` on a host triple sees an empty crate.

#![cfg(target_arch = "wasm32")]

pub mod app;
pub mod components;
pub use websh_core::domain as models;
pub use websh_core::{config, content_routes, mempool};

pub mod crypto {
    pub use websh_core::attestation::{artifact as attestation, ledger, subject};
    pub use websh_core::crypto::{ack, eth, pgp};
}

pub mod core {
    pub use websh_core::admin;
    pub use websh_core::domain::DirEntry;
    pub use websh_core::domain::changes;
    pub use websh_core::filesystem as engine;
    pub use websh_core::filesystem::merge;
    pub use websh_core::runtime;
    pub use websh_core::shell as commands;
    pub use websh_core::shell::parser;
    pub use websh_core::shell::{
        AutocompleteResult, Command, CommandResult, SideEffect, autocomplete, execute_pipeline,
        get_hint, parse_input,
    };
    pub use websh_core::storage;

    pub use websh_core::runtime::env;
    pub use websh_core::runtime::wallet;
}

pub mod utils;
