//! Core business logic for the terminal application.
//!
//! This module provides:
//! - [`Command`] parsing and [`execute_pipeline`] execution
//! - [`VirtualFs`] virtual filesystem management
//! - [`MergedFs`] merged filesystem with pending changes
//! - [`FsState`] reactive filesystem state for Leptos
//! - [`autocomplete`] and [`get_hint`] for tab completion
//! - [`storage`] module for admin write operations
//! - [`admin`] module for admin identification

pub mod admin;
mod autocomplete;
mod commands;
pub mod env;
pub mod error;
pub mod filesystem;
pub mod parser;
pub mod storage;
pub mod wallet;

pub use autocomplete::{AutocompleteResult, autocomplete, get_hint};
pub use commands::{Command, execute_pipeline};
pub use filesystem::{DirEntry, FsState, MergedFs, VirtualFs};
pub use parser::parse_input;
