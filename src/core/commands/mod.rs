//! Command parsing and execution.
//!
//! This module provides:
//! - `Command` enum for parsed terminal commands
//! - `CommandResult` for command execution results
//! - `execute_pipeline` for executing commands with pipe support
//!
//! # Architecture
//!
//! Commands are parsed from user input into the `Command` enum,
//! then executed via `execute_command`. Pipes are handled by
//! `execute_pipeline`, which applies filter commands (grep, head, tail, wc).

mod execute;
mod filters;
mod result;

pub use execute::execute_command;
pub use filters::apply_filter;
pub use result::CommandResult;

use std::fmt;

use crate::app::TerminalState;
use crate::core::VirtualFs;
use crate::core::parser::Pipeline;
use crate::models::{AppRoute, OutputLine, WalletState};

// =============================================================================
// Path Argument Type
// =============================================================================

/// A path argument passed to a command (e.g., `cd foo`, `cat bar.md`).
///
/// This newtype distinguishes path arguments from general strings,
/// providing type safety and clearer intent in the command parsing layer.
/// The path is stored as-is (not validated) since validation happens
/// during execution against the virtual filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathArg(String);

impl PathArg {
    /// Create a new path argument from a string.
    pub fn new(path: impl Into<String>) -> Self {
        Self(path.into())
    }

    /// Get the path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PathArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for PathArg {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PathArg {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl PartialEq<str> for PathArg {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for PathArg {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

// =============================================================================
// Command Enum
// =============================================================================

/// Parsed terminal command
#[derive(Clone, Debug)]
pub enum Command {
    /// List directory contents. bool = long format (-l)
    Ls {
        path: Option<PathArg>,
        long: bool,
    },
    Cd(PathArg),
    Pwd,
    Cat(PathArg),
    Whoami,
    Id,
    Help,
    Clear,
    Echo(String),
    Export(Option<String>),
    Unset(String),
    Login,
    Logout,
    /// Switch to explorer view mode with optional path
    #[allow(dead_code)]
    Explorer(Option<PathArg>),
    Unknown(String),
}

impl Command {
    /// Get all available command names for autocomplete.
    ///
    /// Includes both regular commands and pipe filter commands.
    pub fn names() -> &'static [&'static str] {
        &[
            "cat", "cd", "clear", "cls", "echo", "explorer", "export", "grep", "head", "help",
            "id", "login", "logout", "ls", "pwd", "tail", "unset", "wc", "whoami",
        ]
    }

    /// Parse command from name and arguments.
    pub fn parse(name: &str, args: &[String]) -> Self {
        match name.to_lowercase().as_str() {
            "ls" => {
                let mut long = false;
                let mut path = None;
                for arg in args {
                    if arg == "-l" {
                        long = true;
                    } else if path.is_none() {
                        path = Some(PathArg::new(arg));
                    }
                }
                Self::Ls { path, long }
            }
            "cd" => Self::Cd(
                args.first()
                    .map(PathArg::new)
                    .unwrap_or_else(|| PathArg::new("~")),
            ),
            "pwd" => Self::Pwd,
            "cat" => {
                if let Some(file) = args.first() {
                    Self::Cat(PathArg::new(file))
                } else {
                    Self::Unknown("cat: missing file operand".to_string())
                }
            }
            "whoami" => Self::Whoami,
            "id" => Self::Id,
            "help" | "?" => Self::Help,
            "clear" | "cls" => Self::Clear,
            "echo" => Self::Echo(args.join(" ")),
            "export" => {
                if args.is_empty() {
                    Self::Export(None)
                } else {
                    Self::Export(Some(args.join(" ")))
                }
            }
            "unset" => {
                if let Some(key) = args.first() {
                    Self::Unset(key.clone())
                } else {
                    Self::Unknown("unset: missing variable name".to_string())
                }
            }
            "login" => Self::Login,
            "logout" => Self::Logout,
            "explorer" => Self::Explorer(args.first().map(PathArg::new)),
            _ => Self::Unknown(name.to_string()),
        }
    }
}

// =============================================================================
// Pipeline Execution
// =============================================================================

/// Execute a pipeline of commands with pipe filtering.
///
/// A pipeline consists of a main command followed by optional filter commands
/// separated by `|`. For example: `ls | grep foo | head -5`
pub fn execute_pipeline(
    pipeline: &Pipeline,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &VirtualFs,
    current_route: &AppRoute,
) -> CommandResult {
    // Check for syntax errors first
    if let Some(ref err) = pipeline.error {
        return CommandResult::output(vec![OutputLine::error(err.to_string())]);
    }

    if pipeline.is_empty() {
        return CommandResult::empty();
    }

    // Execute first command
    let first = &pipeline.commands[0];
    let cmd = Command::parse(&first.name, &first.args);
    let result = execute_command(cmd, state, wallet_state, fs, current_route);

    // If there are no filters, return directly (preserving navigation)
    if pipeline.commands.len() == 1 {
        return result;
    }

    // Apply pipe filters (navigation is discarded when piping)
    let mut lines = result.output;
    for filter_cmd in pipeline.commands.iter().skip(1) {
        lines = apply_filter(&filter_cmd.name, &filter_cmd.args, lines);
    }

    CommandResult::output(lines)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_ls() {
        assert!(matches!(
            Command::parse("ls", &[]),
            Command::Ls {
                path: None,
                long: false
            }
        ));
        assert!(matches!(
            Command::parse("ls", &args(&["projects"])),
            Command::Ls { path: Some(ref p), long: false } if p == "projects"
        ));
        assert!(matches!(
            Command::parse("ls", &args(&["-l"])),
            Command::Ls {
                path: None,
                long: true
            }
        ));
        assert!(matches!(
            Command::parse("ls", &args(&["-l", "blog"])),
            Command::Ls { path: Some(ref p), long: true } if p == "blog"
        ));
    }

    #[test]
    fn test_parse_cd() {
        assert!(matches!(
            Command::parse("cd", &[]),
            Command::Cd(ref p) if p == "~"
        ));
        assert!(matches!(
            Command::parse("cd", &args(&["/home"])),
            Command::Cd(ref p) if p == "/home"
        ));
    }

    #[test]
    fn test_parse_cat() {
        assert!(matches!(
            Command::parse("cat", &args(&["file.md"])),
            Command::Cat(ref f) if f == "file.md"
        ));
    }

    #[test]
    fn test_parse_explorer() {
        assert!(matches!(
            Command::parse("explorer", &[]),
            Command::Explorer(None)
        ));
        assert!(matches!(
            Command::parse("explorer", &args(&["/home"])),
            Command::Explorer(Some(ref p)) if p == "/home"
        ));
        assert!(matches!(
            Command::parse("explorer", &args(&["projects"])),
            Command::Explorer(Some(ref p)) if p == "projects"
        ));
    }

    #[test]
    fn test_parse_cat_missing_file() {
        assert!(matches!(Command::parse("cat", &[]), Command::Unknown(_)));
    }

    #[test]
    fn test_parse_export() {
        assert!(matches!(
            Command::parse("export", &[]),
            Command::Export(None)
        ));
        assert!(matches!(
            Command::parse("export", &args(&["FOO=bar"])),
            Command::Export(Some(ref s)) if s == "FOO=bar"
        ));
    }

    #[test]
    fn test_parse_unset() {
        assert!(matches!(
            Command::parse("unset", &args(&["FOO"])),
            Command::Unset(ref k) if k == "FOO"
        ));
        assert!(matches!(Command::parse("unset", &[]), Command::Unknown(_)));
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert!(matches!(
            Command::parse("LS", &[]),
            Command::Ls {
                path: None,
                long: false
            }
        ));
        assert!(matches!(
            Command::parse("CD", &args(&["/"])),
            Command::Cd(_)
        ));
        assert!(matches!(Command::parse("HELP", &[]), Command::Help));
        assert!(matches!(Command::parse("CleAr", &[]), Command::Clear));
    }

    #[test]
    fn test_parse_aliases() {
        assert!(matches!(Command::parse("?", &[]), Command::Help));
        assert!(matches!(Command::parse("cls", &[]), Command::Clear));
    }

    #[test]
    fn test_parse_unknown() {
        assert!(matches!(
            Command::parse("foobar", &[]),
            Command::Unknown(ref c) if c == "foobar"
        ));
    }

    #[test]
    fn test_command_names() {
        let names = Command::names();
        assert!(names.contains(&"ls"));
        assert!(names.contains(&"cd"));
        assert!(names.contains(&"cat"));
        assert!(names.contains(&"help"));
        assert!(names.contains(&"login"));
        assert!(names.contains(&"logout"));
        assert!(names.contains(&"explorer"));
        // Filter commands should be included for autocomplete
        assert!(names.contains(&"grep"));
        assert!(names.contains(&"head"));
        assert!(names.contains(&"tail"));
        assert!(names.contains(&"wc"));
        // less and more should NOT be in the list
        assert!(!names.contains(&"less"));
        assert!(!names.contains(&"more"));
    }
}
