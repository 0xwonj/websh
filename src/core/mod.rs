//! Core business logic for the terminal application.
//!
//! This module provides:
//! - [`Command`] parsing and [`execute_pipeline`] execution
//! - [`VirtualFs`] virtual filesystem management
//! - [`autocomplete`] and [`get_hint`] for tab completion

mod autocomplete;
mod commands;
pub mod env;
pub mod error;
mod filesystem;
pub mod parser;
pub mod wallet;

pub use autocomplete::{AutocompleteResult, autocomplete, get_hint};
pub use commands::{Command, execute_pipeline};
pub use filesystem::{DirEntry, VirtualFs};
pub use parser::parse_input;
