use std::fmt;

use leptos::prelude::*;

use crate::app::TerminalState;
use crate::config::{ASCII_PROFILE, HELP_TEXT, PROFILE_PATH, pipe_filters};
use crate::core::parser::Pipeline;
use crate::core::{VirtualFs, env, wallet};
use crate::models::{OutputLine, OutputLineData, ScreenMode, WalletState};
use crate::utils::sysinfo;

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
    Ls(Option<PathArg>),
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
    Unknown(String),
}

impl Command {
    /// Get all available command names for autocomplete.
    ///
    /// Includes both regular commands and pipe filter commands.
    pub fn names() -> &'static [&'static str] {
        &[
            "cat", "cd", "clear", "cls", "echo", "export", "grep", "head", "help", "id", "less",
            "login", "logout", "ls", "more", "pwd", "tail", "unset", "wc", "whoami",
        ]
    }

    /// Parse command from name and arguments.
    pub fn parse(name: &str, args: &[String]) -> Self {
        match name.to_lowercase().as_str() {
            "ls" => Self::Ls(args.first().map(PathArg::new)),
            "cd" => Self::Cd(
                args.first()
                    .map(PathArg::new)
                    .unwrap_or_else(|| PathArg::new("~")),
            ),
            "pwd" => Self::Pwd,
            "cat" | "less" | "more" => {
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
            _ => Self::Unknown(name.to_string()),
        }
    }
}

/// Execute a parsed command and return output lines.
///
/// This function may have side effects on the terminal state (e.g., changing
/// directories, clearing history, opening reader mode).
///
/// # Arguments
///
/// * `cmd` - The parsed command to execute
/// * `state` - Terminal state (for path navigation and screen mode)
/// * `wallet_state` - Current wallet connection state
/// * `fs` - Virtual filesystem
pub fn execute_command(
    cmd: Command,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &VirtualFs,
) -> Vec<OutputLine> {
    match cmd {
        Command::Ls(path) => {
            let current = state.current_path.get();
            let target = path.as_ref().map(|p| p.as_str()).unwrap_or(".");
            let resolved = fs.resolve_path(&current, target);

            match resolved {
                Some(resolved_path) => {
                    if let Some(entries) = fs.list_dir(resolved_path.as_str()) {
                        let mut lines = vec![];
                        for (name, is_dir, desc) in entries {
                            if is_dir {
                                lines.push(OutputLine::dir_entry(&name, desc));
                            } else {
                                lines.push(OutputLine::file_entry(&name, desc));
                            }
                        }
                        lines
                    } else {
                        vec![OutputLine::error(format!(
                            "ls: cannot access '{}': Not a directory",
                            target
                        ))]
                    }
                }
                None => {
                    vec![OutputLine::error(format!(
                        "ls: cannot access '{}': No such file or directory",
                        target
                    ))]
                }
            }
        }

        Command::Cd(path) => {
            let current = state.current_path.get();
            match fs.resolve_path(&current, path.as_str()) {
                Some(new_path) if fs.is_directory(new_path.as_str()) => {
                    state.current_path.set(new_path);
                    vec![]
                }
                Some(_) => {
                    vec![OutputLine::error(format!("cd: not a directory: {}", path))]
                }
                None => {
                    vec![OutputLine::error(format!(
                        "cd: no such file or directory: {}",
                        path
                    ))]
                }
            }
        }

        Command::Pwd => {
            vec![OutputLine::text(state.current_path.get().to_string())]
        }

        Command::Cat(file) => {
            let current = state.current_path.get();
            let resolved = fs.resolve_path(&current, file.as_str());

            match resolved {
                Some(resolved_path) => {
                    if fs.is_directory(resolved_path.as_str()) {
                        vec![OutputLine::error(format!("cat: {}: Is a directory", file))]
                    } else if let Some(content_path) =
                        fs.get_file_content_path(resolved_path.as_str())
                    {
                        // Set screen mode to reader - content will be loaded async
                        state.screen_mode.set(ScreenMode::Reader {
                            content: content_path.clone(),
                            title: file.to_string(),
                        });
                        vec![OutputLine::info(format!("Opening {}...", file))]
                    } else if resolved_path.as_str() == PROFILE_PATH {
                        // Dynamic .profile from environment variables
                        let content = env::generate_profile();
                        let mut lines = vec![OutputLine::empty()];
                        for line in content.lines() {
                            lines.push(OutputLine::text(line));
                        }
                        lines.push(OutputLine::empty());
                        lines
                    } else {
                        vec![OutputLine::error(format!(
                            "cat: {}: No content available",
                            file
                        ))]
                    }
                }
                None => {
                    vec![OutputLine::error(format!(
                        "cat: {}: No such file or directory",
                        file
                    ))]
                }
            }
        }

        Command::Whoami => {
            vec![OutputLine::ascii(ASCII_PROFILE.to_string())]
        }

        Command::Id => {
            let mut lines = vec![OutputLine::empty()];

            // User identity
            match wallet_state {
                WalletState::Connected {
                    address, ens_name, ..
                } => {
                    if let Some(ens) = ens_name {
                        lines.push(OutputLine::text(format!("uid={} ({})", address, ens)));
                    } else {
                        lines.push(OutputLine::text(format!("uid={}", address)));
                    }
                    lines.push(OutputLine::text("gid=visitor"));
                    lines.push(OutputLine::text("status=connected"));
                }
                WalletState::Disconnected => {
                    lines.push(OutputLine::text("uid=guest"));
                    lines.push(OutputLine::text("gid=anonymous"));
                    lines.push(OutputLine::text("status=disconnected"));
                }
                WalletState::Connecting => {
                    lines.push(OutputLine::text("uid=..."));
                    lines.push(OutputLine::text("status=connecting"));
                }
            }

            // Network info
            if let Some(chain_id) = wallet_state.chain_id() {
                lines.push(OutputLine::text(format!(
                    "network={}",
                    wallet::chain_name(chain_id)
                )));
                lines.push(OutputLine::text(format!("chain_id={}", chain_id)));
            } else {
                lines.push(OutputLine::text("network=none"));
            }

            // Session uptime
            if let Some(uptime) = sysinfo::get_uptime() {
                lines.push(OutputLine::text(format!("uptime={}", uptime)));
            }

            // Browser info
            if let Some(window) = web_sys::window()
                && let Ok(ua) = window.navigator().user_agent()
            {
                lines.push(OutputLine::text(format!("user_agent={}", ua)));
            }

            lines.push(OutputLine::empty());
            lines
        }

        Command::Help => HELP_TEXT.lines().map(OutputLine::text).collect(),

        Command::Clear => {
            state.clear_history();
            vec![]
        }

        Command::Echo(text) => {
            vec![OutputLine::text(text)]
        }

        Command::Export(arg) => {
            match arg {
                None => {
                    // No argument: show all variables
                    let lines = env::format_export_output();
                    let mut output = vec![OutputLine::empty()];
                    for line in lines {
                        output.push(OutputLine::text(line));
                    }
                    output.push(OutputLine::empty());
                    output
                }
                Some(assignment) => {
                    // Parse KEY=value
                    if let Some((key, value)) = assignment.split_once('=') {
                        let key = key.trim();
                        let value = value.trim().trim_matches('"').trim_matches('\'');

                        match env::set_user_var(key, value) {
                            Ok(()) => vec![],
                            Err(e) => vec![OutputLine::error(format!("export: {}", e))],
                        }
                    } else {
                        // Just a key without value - show current value
                        let key = assignment.trim();
                        if let Some(value) = env::get_user_var(key) {
                            vec![OutputLine::text(format!("{}={}", key, value))]
                        } else {
                            vec![]
                        }
                    }
                }
            }
        }

        Command::Unset(key) => {
            if env::get_user_var(&key).is_some() {
                match env::unset_user_var(&key) {
                    Ok(()) => vec![],
                    Err(e) => vec![OutputLine::error(format!("unset: {}", e))],
                }
            } else {
                vec![] // Silently succeed if variable doesn't exist
            }
        }

        Command::Unknown(cmd) => {
            vec![OutputLine::error(format!(
                "Command not found: {}. Type 'help' for available commands.",
                cmd
            ))]
        }

        // Login/Logout are handled asynchronously in shell.rs
        Command::Login | Command::Logout => vec![],
    }
}

/// Execute a pipeline of commands with pipe filtering
pub fn execute_pipeline(
    pipeline: &Pipeline,
    state: &TerminalState,
    wallet_state: &WalletState,
    fs: &VirtualFs,
) -> Vec<OutputLine> {
    // Check for syntax errors first
    if let Some(ref err) = pipeline.error {
        return vec![OutputLine::error(err.to_string())];
    }

    if pipeline.is_empty() {
        return vec![];
    }

    // Execute first command
    let first = &pipeline.commands[0];
    let cmd = Command::parse(&first.name, &first.args);
    let mut lines = execute_command(cmd, state, wallet_state, fs);

    // Apply pipe filters
    for filter_cmd in pipeline.commands.iter().skip(1) {
        lines = apply_filter(&filter_cmd.name, &filter_cmd.args, lines);
    }

    lines
}

/// Apply a filter command to output lines (for pipe support)
fn apply_filter(cmd: &str, args: &[String], lines: Vec<OutputLine>) -> Vec<OutputLine> {
    match cmd.to_lowercase().as_str() {
        "grep" => {
            let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
            if pattern.is_empty() {
                return vec![OutputLine::error("grep: missing pattern")];
            }
            let pattern_lower = pattern.to_lowercase();
            lines
                .into_iter()
                .filter(|line| match &line.data {
                    OutputLineData::Text(s)
                    | OutputLineData::Error(s)
                    | OutputLineData::Success(s)
                    | OutputLineData::Info(s)
                    | OutputLineData::Ascii(s) => s.to_lowercase().contains(&pattern_lower),
                    OutputLineData::ListEntry {
                        name, description, ..
                    } => {
                        name.to_lowercase().contains(&pattern_lower)
                            || description.to_lowercase().contains(&pattern_lower)
                    }
                    OutputLineData::Command { input, .. } => {
                        input.to_lowercase().contains(&pattern_lower)
                    }
                    OutputLineData::Empty => false,
                })
                .collect()
        }
        "head" => {
            let n: usize = args
                .first()
                .and_then(|s| s.trim_start_matches('-').parse().ok())
                .unwrap_or(pipe_filters::DEFAULT_HEAD_LINES);
            lines.into_iter().take(n).collect()
        }
        "tail" => {
            let n: usize = args
                .first()
                .and_then(|s| s.trim_start_matches('-').parse().ok())
                .unwrap_or(pipe_filters::DEFAULT_TAIL_LINES);
            let len = lines.len();
            lines.into_iter().skip(len.saturating_sub(n)).collect()
        }
        "wc" => {
            // Count lines (excluding Empty)
            let count = lines
                .iter()
                .filter(|l| !matches!(l.data, OutputLineData::Empty))
                .count();
            vec![OutputLine::text(format!("{}", count))]
        }
        _ => {
            vec![OutputLine::error(format!(
                "Pipe: unknown filter '{}'. Supported: grep, head, tail, wc",
                cmd
            ))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn test_parse_ls() {
        assert!(matches!(Command::parse("ls", &[]), Command::Ls(None)));
        assert!(matches!(
            Command::parse("ls", &args(&["projects"])),
            Command::Ls(Some(ref p)) if p == "projects"
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
    fn test_parse_cat_variations() {
        assert!(matches!(
            Command::parse("cat", &args(&["file.md"])),
            Command::Cat(ref f) if f == "file.md"
        ));
        assert!(matches!(
            Command::parse("less", &args(&["file.md"])),
            Command::Cat(ref f) if f == "file.md"
        ));
        assert!(matches!(
            Command::parse("more", &args(&["file.md"])),
            Command::Cat(ref f) if f == "file.md"
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
        assert!(matches!(Command::parse("LS", &[]), Command::Ls(None)));
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
        // Filter commands should be included for autocomplete
        assert!(names.contains(&"grep"));
        assert!(names.contains(&"head"));
        assert!(names.contains(&"tail"));
        assert!(names.contains(&"wc"));
    }

    // =========================================================================
    // Filter Tests
    // =========================================================================

    fn test_lines() -> Vec<OutputLine> {
        vec![
            OutputLine::text("apple"),
            OutputLine::text("banana"),
            OutputLine::text("cherry"),
            OutputLine::text("date"),
            OutputLine::text("elderberry"),
        ]
    }

    #[test]
    fn test_grep_filter() {
        let lines = test_lines();
        let result = apply_filter("grep", &args(&["an"]), lines);
        assert_eq!(result.len(), 1); // only banana matches "an"
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_case_insensitive() {
        let lines = vec![OutputLine::text("APPLE"), OutputLine::text("banana")];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "APPLE"));
    }

    #[test]
    fn test_grep_missing_pattern() {
        let lines = test_lines();
        let result = apply_filter("grep", &[], lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Error(s) if s.contains("missing pattern")));
    }

    #[test]
    fn test_head_filter() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["3"]), lines);
        assert_eq!(result.len(), 3);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "apple"));
        assert!(matches!(&result[2].data, OutputLineData::Text(s) if s == "cherry"));
    }

    #[test]
    fn test_head_with_dash() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["-2"]), lines);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_head_default() {
        // Default is 10, but we only have 5 lines
        let lines = test_lines();
        let result = apply_filter("head", &[], lines);
        assert_eq!(result.len(), 5);
    }

    #[test]
    fn test_tail_filter() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["2"]), lines);
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "date"));
        assert!(matches!(&result[1].data, OutputLineData::Text(s) if s == "elderberry"));
    }

    #[test]
    fn test_tail_with_dash() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["-3"]), lines);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_wc_filter() {
        let lines = test_lines();
        let result = apply_filter("wc", &[], lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "5"));
    }

    #[test]
    fn test_wc_excludes_empty() {
        let lines = vec![
            OutputLine::text("line1"),
            OutputLine::empty(),
            OutputLine::text("line2"),
            OutputLine::empty(),
        ];
        let result = apply_filter("wc", &[], lines);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "2"));
    }

    #[test]
    fn test_unknown_filter() {
        let lines = test_lines();
        let result = apply_filter("unknown", &[], lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Error(s) if s.contains("unknown filter")));
    }

    #[test]
    fn test_grep_list_entry() {
        let lines = vec![
            OutputLine::dir_entry("project-alpha", "Alpha project"),
            OutputLine::dir_entry("project-beta", "Beta testing"),
        ];
        let result = apply_filter("grep", &args(&["alpha"]), lines);
        assert_eq!(result.len(), 1);
    }
}
