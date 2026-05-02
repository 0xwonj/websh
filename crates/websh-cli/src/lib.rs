//! Native build-time CLI: clap dispatchers + engine modules.
//!
//! Engines live under `cli/<subcommand>` for now (mirroring the legacy
//! crate's layout); the migration's Conventions document `cli/` as
//! thin clap parsers + dispatch into `engine/` modules. That further
//! split is tracked as a Phase C follow-up.

pub mod cli;

pub use cli::run;
