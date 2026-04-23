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
pub use result::{CommandResult, SideEffect};

use std::fmt;

use crate::app::TerminalState;
use crate::core::changes::ChangeSet;
use crate::core::engine::GlobalFs;
use crate::core::parser::Pipeline;
use crate::models::{RuntimeMount, VirtualPath, WalletState};

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
    Cat(Option<PathArg>),
    Whoami,
    Id,
    Help,
    Clear,
    Echo(String),
    /// `export` command. Each element is one raw `KEY=value` assignment
    /// (or a bare `KEY` for display). Empty Vec prints all variables.
    Export(Vec<String>),
    Unset(Option<String>),
    Login,
    Logout,
    /// Switch to explorer view mode with optional path
    Explorer(Option<PathArg>),

    // Phase 4 — write / sync commands. Parsing wired in Task 4.2 / 4.3;
    // these variants are reachable directly from tests for now.
    Touch {
        path: PathArg,
    },
    Mkdir {
        path: PathArg,
    },
    Rm {
        path: PathArg,
        recursive: bool,
    },
    Rmdir {
        path: PathArg,
    },
    Edit {
        path: PathArg,
    },
    Sync(SyncSubcommand),
    EchoRedirect {
        body: String,
        path: PathArg,
    },

    Unknown(String),
}

/// `sync` subcommands — surface the in-progress change set, commit, refresh,
/// or set/clear the auth token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncSubcommand {
    Status,
    Commit { message: String },
    Refresh,
    Auth(AuthAction),
}

/// Auth token actions for `sync auth`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthAction {
    Set { token: String },
    Clear,
}

impl Command {
    /// Get all available command names for autocomplete.
    ///
    /// Includes both regular commands and pipe filter commands.
    pub fn names() -> &'static [&'static str] {
        &[
            "cat", "cd", "clear", "cls", "echo", "edit", "explorer", "export", "grep", "head",
            "help", "id", "login", "logout", "ls", "mkdir", "pwd", "rm", "rmdir", "sync", "tail",
            "touch", "unset", "wc", "whoami",
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
            "cat" => Self::Cat(args.first().map(PathArg::new)),
            "whoami" => Self::Whoami,
            "id" => Self::Id,
            "help" | "?" => Self::Help,
            "clear" | "cls" => Self::Clear,
            "echo" => {
                // Scan args for a whole-token redirect operator ">".
                // The lexer strips quotes, so a quoted `">"` arrives as a
                // Word equal to ">" too — but our callers only produce
                // plain `>` as a redirect in tests and practice. Quoted
                // `>` is exceedingly unusual and, if it ever occurs, is
                // still parsed as a redirect here; that matches the
                // tokenizer's declared contract (quotes are lost after
                // lexing).
                if let Some(idx) = args.iter().position(|a| a == ">") {
                    let body = args[..idx].join(" ");
                    let targets = &args[idx + 1..];
                    if body.is_empty() || targets.len() != 1 {
                        return Self::Unknown("echo".to_string());
                    }
                    Self::EchoRedirect {
                        body,
                        path: PathArg::new(&targets[0]),
                    }
                } else {
                    Self::Echo(args.join(" "))
                }
            }
            "export" => Self::Export(args.to_vec()),
            "unset" => Self::Unset(args.first().cloned()),
            "login" => Self::Login,
            "logout" => Self::Logout,
            "explorer" => Self::Explorer(args.first().map(PathArg::new)),
            "touch" => {
                if args.len() != 1 {
                    return Self::Unknown("touch".to_string());
                }
                Self::Touch {
                    path: PathArg::new(&args[0]),
                }
            }
            "mkdir" => {
                if args.len() != 1 {
                    return Self::Unknown("mkdir".to_string());
                }
                Self::Mkdir {
                    path: PathArg::new(&args[0]),
                }
            }
            "rmdir" => {
                if args.len() != 1 {
                    return Self::Unknown("rmdir".to_string());
                }
                Self::Rmdir {
                    path: PathArg::new(&args[0]),
                }
            }
            "rm" => {
                let mut recursive = false;
                let mut paths: Vec<&String> = Vec::new();
                for arg in args {
                    match arg.as_str() {
                        "-r" | "-rf" | "--recursive" => recursive = true,
                        _ => paths.push(arg),
                    }
                }
                if paths.len() != 1 {
                    return Self::Unknown("rm".to_string());
                }
                Self::Rm {
                    path: PathArg::new(paths[0]),
                    recursive,
                }
            }
            "edit" => {
                if args.len() != 1 {
                    return Self::Unknown("edit".to_string());
                }
                Self::Edit {
                    path: PathArg::new(&args[0]),
                }
            }
            "sync" => match args.first().map(String::as_str) {
                None => Self::Sync(SyncSubcommand::Status),
                Some("status") if args.len() == 1 => Self::Sync(SyncSubcommand::Status),
                Some("refresh") if args.len() == 1 => Self::Sync(SyncSubcommand::Refresh),
                Some("commit") => {
                    if args.len() < 2 {
                        return Self::Unknown("sync".to_string());
                    }
                    let message = args[1..].join(" ");
                    if message.is_empty() {
                        return Self::Unknown("sync".to_string());
                    }
                    Self::Sync(SyncSubcommand::Commit { message })
                }
                Some("auth") => match args.get(1).map(String::as_str) {
                    Some("set") => {
                        if args.len() != 3 {
                            return Self::Unknown("sync".to_string());
                        }
                        Self::Sync(SyncSubcommand::Auth(AuthAction::Set {
                            token: args[2].clone(),
                        }))
                    }
                    Some("clear") if args.len() == 2 => {
                        Self::Sync(SyncSubcommand::Auth(AuthAction::Clear))
                    }
                    _ => Self::Unknown("sync".to_string()),
                },
                _ => Self::Unknown("sync".to_string()),
            },
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
    runtime_mounts: &[RuntimeMount],
    fs: &GlobalFs,
    cwd: &VirtualPath,
    changes: &ChangeSet,
    remote_head: Option<&str>,
) -> CommandResult {
    if let Some(ref err) = pipeline.error {
        return CommandResult::error_line(err.to_string()).with_exit_code(2);
    }

    if pipeline.is_empty() {
        return CommandResult::empty();
    }

    // Execute first command.
    let first = &pipeline.commands[0];
    let cmd = Command::parse(&first.name, &first.args);
    let mut result = execute_command(
        cmd,
        state,
        wallet_state,
        runtime_mounts,
        fs,
        cwd,
        changes,
        remote_head,
    );

    if pipeline.commands.len() == 1 {
        return result;
    }

    // Pipeline mode: side effects are discarded (cannot navigate mid-pipe).
    result.side_effect = None;
    let mut current_lines = result.output;
    let mut current_exit = result.exit_code;

    for filter_cmd in pipeline.commands.iter().skip(1) {
        let stage = apply_filter(&filter_cmd.name, &filter_cmd.args, current_lines);
        current_lines = stage.output;
        current_exit = stage.exit_code;
    }

    CommandResult::output(current_lines).with_exit_code(current_exit)
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
            Command::Cat(Some(ref f)) if f == "file.md"
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
        assert!(matches!(Command::parse("cat", &[]), Command::Cat(None)));
    }

    #[test]
    fn test_parse_export() {
        assert!(matches!(
            Command::parse("export", &[]),
            Command::Export(ref v) if v.is_empty()
        ));
        assert!(matches!(
            Command::parse("export", &args(&["FOO=bar"])),
            Command::Export(ref v) if v.len() == 1 && v[0] == "FOO=bar"
        ));
    }

    #[test]
    fn test_parse_export_multi() {
        assert!(matches!(
            Command::parse("export", &args(&["FOO=a", "BAR=b"])),
            Command::Export(ref v) if v.len() == 2 && v[0] == "FOO=a" && v[1] == "BAR=b"
        ));
    }

    #[test]
    fn test_parse_unset() {
        assert!(matches!(
            Command::parse("unset", &args(&["FOO"])),
            Command::Unset(Some(ref k)) if k == "FOO"
        ));
        assert!(matches!(Command::parse("unset", &[]), Command::Unset(None)));
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

    #[test]
    fn test_pipeline_no_filters_preserves_side_effect() {
        // execute_pipeline should preserve SideEffect from first command
        // when there are no filters.
        use crate::app::TerminalState;
        use crate::core::changes::ChangeSet;
        use crate::core::engine::GlobalFs;
        use crate::core::parser::parse_input;
        use crate::models::{VirtualPath, WalletState};

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::from_absolute("/site").unwrap();
        let changes = ChangeSet::new();

        let pipeline = parse_input("login", &[]);
        let result = execute_pipeline(
            &pipeline,
            &state,
            &wallet,
            &[crate::core::storage::boot::bootstrap_runtime_mount()],
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert_eq!(result.side_effect, Some(super::SideEffect::Login));
    }

    #[test]
    fn test_pipeline_drops_side_effect_when_piped() {
        // When a command has filters attached, side effects are discarded.
        use crate::app::TerminalState;
        use crate::core::changes::ChangeSet;
        use crate::core::engine::GlobalFs;
        use crate::core::parser::parse_input;
        use crate::models::{VirtualPath, WalletState};

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::from_absolute("/site").unwrap();
        let changes = ChangeSet::new();

        let pipeline = parse_input("help | head -1", &[]);
        let result = execute_pipeline(
            &pipeline,
            &state,
            &wallet,
            &[crate::core::storage::boot::bootstrap_runtime_mount()],
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert!(result.side_effect.is_none());
    }

    #[test]
    fn test_pipeline_exit_code_is_last_stage() {
        use crate::app::TerminalState;
        use crate::core::changes::ChangeSet;
        use crate::core::engine::GlobalFs;
        use crate::core::parser::parse_input;
        use crate::models::{VirtualPath, WalletState};

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::from_absolute("/site").unwrap();
        let changes = ChangeSet::new();

        // `help | grep xyzzy` should exit 1 (grep no match)
        let pipeline = parse_input("help | grep xyzzy", &[]);
        let result = execute_pipeline(
            &pipeline,
            &state,
            &wallet,
            &[crate::core::storage::boot::bootstrap_runtime_mount()],
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

    // =========================================================================
    // Parse tests for Phase 4 write + sync commands (Task 4.2 / 4.3)
    // =========================================================================

    #[test]
    fn test_parse_touch_ok() {
        assert!(matches!(
            Command::parse("touch", &args(&["/tmp/a.md"])),
            Command::Touch { ref path } if path == "/tmp/a.md"
        ));
    }

    #[test]
    fn test_parse_touch_missing_operand() {
        assert!(matches!(
            Command::parse("touch", &[]),
            Command::Unknown(ref c) if c == "touch"
        ));
    }

    #[test]
    fn test_parse_touch_extra_args() {
        assert!(matches!(
            Command::parse("touch", &args(&["a", "b"])),
            Command::Unknown(ref c) if c == "touch"
        ));
    }

    #[test]
    fn test_parse_mkdir_ok() {
        assert!(matches!(
            Command::parse("mkdir", &args(&["/tmp/d"])),
            Command::Mkdir { ref path } if path == "/tmp/d"
        ));
    }

    #[test]
    fn test_parse_mkdir_missing() {
        assert!(matches!(
            Command::parse("mkdir", &[]),
            Command::Unknown(ref c) if c == "mkdir"
        ));
    }

    #[test]
    fn test_parse_rmdir_ok() {
        assert!(matches!(
            Command::parse("rmdir", &args(&["/tmp/d"])),
            Command::Rmdir { ref path } if path == "/tmp/d"
        ));
    }

    #[test]
    fn test_parse_rmdir_missing() {
        assert!(matches!(
            Command::parse("rmdir", &[]),
            Command::Unknown(ref c) if c == "rmdir"
        ));
    }

    #[test]
    fn test_parse_rm_simple() {
        assert!(matches!(
            Command::parse("rm", &args(&["/tmp/a.md"])),
            Command::Rm { ref path, recursive: false } if path == "/tmp/a.md"
        ));
    }

    #[test]
    fn test_parse_rm_short_r() {
        assert!(matches!(
            Command::parse("rm", &args(&["-r", "/tmp/d"])),
            Command::Rm { ref path, recursive: true } if path == "/tmp/d"
        ));
    }

    #[test]
    fn test_parse_rm_rf() {
        assert!(matches!(
            Command::parse("rm", &args(&["-rf", "/tmp/d"])),
            Command::Rm { ref path, recursive: true } if path == "/tmp/d"
        ));
    }

    #[test]
    fn test_parse_rm_long_recursive() {
        assert!(matches!(
            Command::parse("rm", &args(&["--recursive", "/tmp/d"])),
            Command::Rm { ref path, recursive: true } if path == "/tmp/d"
        ));
    }

    #[test]
    fn test_parse_rm_flag_after_path() {
        // `rm <path> -r` should also work: the flag is scanned anywhere.
        assert!(matches!(
            Command::parse("rm", &args(&["/tmp/d", "-r"])),
            Command::Rm { ref path, recursive: true } if path == "/tmp/d"
        ));
    }

    #[test]
    fn test_parse_rm_missing_path() {
        assert!(matches!(
            Command::parse("rm", &[]),
            Command::Unknown(ref c) if c == "rm"
        ));
    }

    #[test]
    fn test_parse_rm_flag_only() {
        assert!(matches!(
            Command::parse("rm", &args(&["-r"])),
            Command::Unknown(ref c) if c == "rm"
        ));
    }

    #[test]
    fn test_parse_rm_multiple_paths() {
        assert!(matches!(
            Command::parse("rm", &args(&["a", "b"])),
            Command::Unknown(ref c) if c == "rm"
        ));
    }

    #[test]
    fn test_parse_edit_ok() {
        assert!(matches!(
            Command::parse("edit", &args(&["/tmp/a.md"])),
            Command::Edit { ref path } if path == "/tmp/a.md"
        ));
    }

    #[test]
    fn test_parse_edit_missing() {
        assert!(matches!(
            Command::parse("edit", &[]),
            Command::Unknown(ref c) if c == "edit"
        ));
    }

    #[test]
    fn test_parse_sync_bare() {
        assert!(matches!(
            Command::parse("sync", &[]),
            Command::Sync(SyncSubcommand::Status)
        ));
    }

    #[test]
    fn test_parse_sync_status() {
        assert!(matches!(
            Command::parse("sync", &args(&["status"])),
            Command::Sync(SyncSubcommand::Status)
        ));
    }

    #[test]
    fn test_parse_sync_commit_message() {
        match Command::parse("sync", &args(&["commit", "fix", "typo"])) {
            Command::Sync(SyncSubcommand::Commit { message }) => {
                assert_eq!(message, "fix typo");
            }
            other => panic!("expected Sync(Commit), got {other:?}"),
        }
    }

    #[test]
    fn test_parse_sync_commit_no_message() {
        assert!(matches!(
            Command::parse("sync", &args(&["commit"])),
            Command::Unknown(ref c) if c == "sync"
        ));
    }

    #[test]
    fn test_parse_sync_refresh() {
        assert!(matches!(
            Command::parse("sync", &args(&["refresh"])),
            Command::Sync(SyncSubcommand::Refresh)
        ));
    }

    #[test]
    fn test_parse_sync_auth_set() {
        match Command::parse("sync", &args(&["auth", "set", "TOK123"])) {
            Command::Sync(SyncSubcommand::Auth(AuthAction::Set { token })) => {
                assert_eq!(token, "TOK123");
            }
            other => panic!("expected Sync(Auth(Set)), got {other:?}"),
        }
    }

    #[test]
    fn test_parse_sync_auth_set_missing_token() {
        assert!(matches!(
            Command::parse("sync", &args(&["auth", "set"])),
            Command::Unknown(ref c) if c == "sync"
        ));
    }

    #[test]
    fn test_parse_sync_auth_clear() {
        assert!(matches!(
            Command::parse("sync", &args(&["auth", "clear"])),
            Command::Sync(SyncSubcommand::Auth(AuthAction::Clear))
        ));
    }

    #[test]
    fn test_parse_sync_auth_bare() {
        assert!(matches!(
            Command::parse("sync", &args(&["auth"])),
            Command::Unknown(ref c) if c == "sync"
        ));
    }

    #[test]
    fn test_parse_sync_unknown_subcommand() {
        assert!(matches!(
            Command::parse("sync", &args(&["foo"])),
            Command::Unknown(ref c) if c == "sync"
        ));
    }

    #[test]
    fn test_parse_echo_redirect_single_word() {
        match Command::parse("echo", &args(&["hello", ">", "/tmp/a.md"])) {
            Command::EchoRedirect { body, path } => {
                assert_eq!(body, "hello");
                assert_eq!(path, PathArg::new("/tmp/a.md"));
            }
            other => panic!("expected EchoRedirect, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_echo_redirect_multi_word_body() {
        match Command::parse("echo", &args(&["hello", "world", ">", "/tmp/a.md"])) {
            Command::EchoRedirect { body, path } => {
                assert_eq!(body, "hello world");
                assert_eq!(path, PathArg::new("/tmp/a.md"));
            }
            other => panic!("expected EchoRedirect, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_echo_redirect_empty_body() {
        assert!(matches!(
            Command::parse("echo", &args(&[">", "/tmp/a.md"])),
            Command::Unknown(ref c) if c == "echo"
        ));
    }

    #[test]
    fn test_parse_echo_redirect_missing_target() {
        assert!(matches!(
            Command::parse("echo", &args(&["hello", ">"])),
            Command::Unknown(ref c) if c == "echo"
        ));
    }

    #[test]
    fn test_parse_echo_redirect_multiple_targets() {
        assert!(matches!(
            Command::parse("echo", &args(&["hello", ">", "a", "b"])),
            Command::Unknown(ref c) if c == "echo"
        ));
    }

    #[test]
    fn test_parse_echo_quoted_gt_is_body_via_lexer() {
        // End-to-end through the lexer: the `a > b` inside quotes must
        // tokenize as a single arg, so the parser shouldn't see a ">"
        // redirect token at all.
        use crate::core::parser::parse_input;

        let pipeline = parse_input("echo \"a > b\" > /tmp/a.md", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands.len(), 1);
        let parsed = &pipeline.commands[0];
        let cmd = Command::parse(&parsed.name, &parsed.args);
        match cmd {
            Command::EchoRedirect { body, path } => {
                assert_eq!(body, "a > b");
                assert_eq!(path, PathArg::new("/tmp/a.md"));
            }
            other => panic!("expected EchoRedirect, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_echo_plain_no_redirect() {
        assert!(matches!(
            Command::parse("echo", &args(&["hello"])),
            Command::Echo(ref s) if s == "hello"
        ));
    }

    #[test]
    fn test_parser_error_exit_2() {
        use crate::app::TerminalState;
        use crate::core::changes::ChangeSet;
        use crate::core::engine::GlobalFs;
        use crate::core::parser::parse_input;
        use crate::models::{VirtualPath, WalletState};

        let state = TerminalState::new();
        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::from_absolute("/site").unwrap();
        let changes = ChangeSet::new();

        // Pipe with nothing on the right-hand side → parse error
        let pipeline = parse_input("ls |", &[]);
        let result = execute_pipeline(
            &pipeline,
            &state,
            &wallet,
            &[crate::core::storage::boot::bootstrap_runtime_mount()],
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert_eq!(result.exit_code, 2);
    }
}
