//! Core business logic for the terminal application.
//!
//! This module provides:
//! - [`Command`] parsing and [`execute_pipeline`] execution
//! - mounted subtree assembly for the canonical global filesystem engine
//! - [`autocomplete`] and [`get_hint`] for tab completion

pub mod admin;
mod autocomplete;
pub mod changes;
mod commands;
pub(crate) mod engine;
pub mod error;
pub mod merge;
pub mod parser;
pub mod runtime;
pub mod storage;

pub use crate::models::DirEntry;
pub use autocomplete::{AutocompleteResult, autocomplete, get_hint};
pub use commands::{Command, SideEffect, execute_pipeline};
pub use parser::parse_input;
pub use runtime::{env, wallet};
