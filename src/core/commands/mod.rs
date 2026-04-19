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
use crate::core::MergedFs;
use crate::core::parser::Pipeline;
use crate::core::storage::{PendingChanges, StagedChanges};
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

    // === Admin Commands ===
    /// Create a new file (admin only)
    Touch(PathArg),
    /// Create a new directory (admin only)
    Mkdir(PathArg),
    /// Remove a file (admin only)
    Rm(PathArg),
    /// Remove a directory (admin only)
    Rmdir(PathArg),

    // === Sync Commands (Git-like workflow) ===
    /// Show pending/staged changes
    SyncStatus,
    /// Stage changes for commit
    SyncAdd(Option<PathArg>),
    /// Unstage changes
    SyncReset(Option<PathArg>),
    /// Commit staged changes
    SyncCommit(Option<String>),
    /// Discard pending changes
    SyncDiscard(Option<PathArg>),
    /// Set authentication token
    SyncAuth {
        provider: String,
        token: Option<String>,
    },

    Unknown(String),
}

impl Command {
    /// Get all available command names for autocomplete.
    ///
    /// Includes both regular commands and pipe filter commands.
    pub fn names() -> &'static [&'static str] {
        &[
            "cat", "cd", "clear", "cls", "echo", "explorer", "export", "grep", "head", "help",
            "id", "login", "logout", "ls", "mkdir", "pwd", "rm", "rmdir", "sync", "tail", "touch",
            "unset", "wc", "whoami",
        ]
    }

    /// Get sync subcommand names for autocomplete.
    pub fn sync_subcommands() -> &'static [&'static str] {
        &["status", "add", "reset", "commit", "discard", "auth"]
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

            // Admin commands
            "touch" => {
                if let Some(path) = args.first() {
                    Self::Touch(PathArg::new(path))
                } else {
                    Self::Unknown("touch: missing file operand".to_string())
                }
            }
            "mkdir" => {
                if let Some(path) = args.first() {
                    Self::Mkdir(PathArg::new(path))
                } else {
                    Self::Unknown("mkdir: missing directory operand".to_string())
                }
            }
            "rm" => {
                if let Some(path) = args.first() {
                    Self::Rm(PathArg::new(path))
                } else {
                    Self::Unknown("rm: missing file operand".to_string())
                }
            }
            "rmdir" => {
                if let Some(path) = args.first() {
                    Self::Rmdir(PathArg::new(path))
                } else {
                    Self::Unknown("rmdir: missing directory operand".to_string())
                }
            }

            // Sync commands (git-like workflow)
            "sync" => Self::parse_sync(args),

            _ => Self::Unknown(name.to_string()),
        }
    }

    /// Parse sync subcommands.
    fn parse_sync(args: &[String]) -> Self {
        let subcommand = args.first().map(|s| s.to_lowercase());
        let sub_args = if args.len() > 1 { &args[1..] } else { &[] };

        match subcommand.as_deref() {
            Some("status") | Some("st") => Self::SyncStatus,
            Some("add") => Self::SyncAdd(sub_args.first().map(PathArg::new)),
            Some("reset") => Self::SyncReset(sub_args.first().map(PathArg::new)),
            Some("commit") | Some("ci") => {
                // Handle -m flag for commit message
                let message = if sub_args.first().map(|s| s.as_str()) == Some("-m") {
                    if sub_args.len() > 1 {
                        Some(sub_args[1..].join(" "))
                    } else {
                        None
                    }
                } else if !sub_args.is_empty() {
                    Some(sub_args.join(" "))
                } else {
                    None
                };
                Self::SyncCommit(message)
            }
            Some("discard") => Self::SyncDiscard(sub_args.first().map(PathArg::new)),
            Some("auth") => {
                let provider = sub_args.first().cloned().unwrap_or_default();
                let token = sub_args.get(1).cloned();
                Self::SyncAuth { provider, token }
            }
            Some(cmd) => Self::Unknown(format!("sync: unknown subcommand '{}'. Try 'sync status', 'sync add', 'sync commit', etc.", cmd)),
            None => Self::Unknown("sync: missing subcommand. Try 'sync status', 'sync add <path>', 'sync commit', etc.".to_string()),
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
///
/// The `pending` and `staged` parameters contain current changes for sync commands.
pub fn execute_pipeline(
    pipeline: &Pipeline,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &MergedFs,
    current_route: &AppRoute,
    pending: &PendingChanges,
    staged: &StagedChanges,
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
    let result = execute_command(cmd, state, wallet_state, fs, current_route, pending, staged);

    // If there are no filters, return directly (preserving navigation, pending, staged)
    if pipeline.commands.len() == 1 {
        return result;
    }

    // Apply pipe filters (navigation, pending, staged are discarded when piping)
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
