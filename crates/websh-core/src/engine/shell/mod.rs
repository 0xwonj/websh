//! Command parsing and execution.

pub(crate) mod autocomplete;
pub(crate) mod config;
mod executor;
mod filters;
mod model;
mod output;
pub(crate) mod parser;
mod pipeline;

pub use autocomplete::{AutocompleteResult, autocomplete, get_hint};
pub use executor::{execute_command, execute_command_with_context};
pub use filters::apply_filter;
pub use model::{
    AuthEffect, Command, CommandResult, EnvironmentEffect, ExecutionContext, NavigationEffect,
    PathArg, RuntimeEffect, ShellEffect, ShellText, SideEffect, SystemEffect, SystemInfo,
    ThemeEffect, ViewEffect, ViewMode,
};
pub use output::{ListFormat, OutputLine, OutputLineData, OutputLineId, TextStyle};
pub use parser::{parse_input, parse_input_with_env};
pub use pipeline::{execute_pipeline, execute_pipeline_with_context};
