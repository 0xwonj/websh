//! Native build-time CLI: clap dispatchers + engine modules.

use std::error::Error;

pub mod cli;
pub(crate) mod commands;
pub(crate) mod infra;
pub(crate) mod workflows;

pub(crate) type CliResult<T = ()> = Result<T, Box<dyn Error>>;

pub use cli::run;
