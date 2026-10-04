//! Native build-time CLI: command adapters, workflows, and host infrastructure.

pub mod cli;
pub(crate) mod commands;
pub(crate) mod infra;
pub(crate) mod workflows;

pub(crate) type CliResult<T = ()> = anyhow::Result<T>;

pub use cli::run;

#[cfg(test)]
#[path = "../tests/support/fs.rs"]
mod test_support;
