//! Shell command model and result types.

//! Command execution result type.

use crate::engine::filesystem::RouteRequest;
use crate::engine::shell::OutputLine;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ViewMode {
    #[default]
    Terminal,
    Explorer,
}

/// Side effect requested by a command's execution.
///
/// Commands return side effects as data; the UI layer (or executor) is
/// responsible for actually performing them. This keeps command logic
/// testable without UI signals or async runtimes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideEffect {
    /// Navigate to a new route.
    Navigate(RouteRequest),
    /// Initiate wallet login (async).
    Login,
    /// Perform wallet logout.
    Logout,
    /// Switch view mode.
    SwitchView(ViewMode),
    /// Switch view mode and navigate in one step.
    SwitchViewAndNavigate(ViewMode, RouteRequest),
    /// Apply a global color palette.
    SetTheme { theme: String },
    /// Request the target to list available color palettes.
    ListThemes,
    /// Set a target-owned user environment variable.
    SetEnvVar { key: String, value: String },
    /// Remove a target-owned user environment variable.
    UnsetEnvVar { key: String },
    /// Reset the terminal output ring buffer.
    ClearHistory,

    ReloadRuntimeMount {
        mount_root: crate::domain::VirtualPath,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationEffect {
    Navigate(RouteRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEffect {
    ReloadRuntimeMount {
        mount_root: crate::domain::VirtualPath,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthEffect {
    Login,
    Logout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThemeEffect {
    SetTheme { theme: String },
    ListThemes,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvironmentEffect {
    SetEnvVar { key: String, value: String },
    UnsetEnvVar { key: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewEffect {
    SwitchView(ViewMode),
    SwitchViewAndNavigate(ViewMode, RouteRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemEffect {
    ClearHistory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellEffect {
    Navigation(NavigationEffect),
    Runtime(RuntimeEffect),
    Auth(AuthEffect),
    Theme(ThemeEffect),
    Environment(EnvironmentEffect),
    View(ViewEffect),
    System(SystemEffect),
}

impl From<SideEffect> for ShellEffect {
    fn from(effect: SideEffect) -> Self {
        match effect {
            SideEffect::Navigate(route) => Self::Navigation(NavigationEffect::Navigate(route)),
            SideEffect::Login => Self::Auth(AuthEffect::Login),
            SideEffect::Logout => Self::Auth(AuthEffect::Logout),
            SideEffect::SwitchView(mode) => Self::View(ViewEffect::SwitchView(mode)),
            SideEffect::SwitchViewAndNavigate(mode, route) => {
                Self::View(ViewEffect::SwitchViewAndNavigate(mode, route))
            }
            SideEffect::SetTheme { theme } => Self::Theme(ThemeEffect::SetTheme { theme }),
            SideEffect::ListThemes => Self::Theme(ThemeEffect::ListThemes),
            SideEffect::SetEnvVar { key, value } => {
                Self::Environment(EnvironmentEffect::SetEnvVar { key, value })
            }
            SideEffect::UnsetEnvVar { key } => {
                Self::Environment(EnvironmentEffect::UnsetEnvVar { key })
            }
            SideEffect::ClearHistory => Self::System(SystemEffect::ClearHistory),
            SideEffect::ReloadRuntimeMount { mount_root } => {
                Self::Runtime(RuntimeEffect::ReloadRuntimeMount { mount_root })
            }
        }
    }
}

impl SideEffect {
    pub fn into_grouped(self) -> ShellEffect {
        self.into()
    }
}

/// Result of executing a command.
///
/// Carries output lines, a POSIX-style exit code, and requested side effects
/// (navigation, wallet action, state mutation, view switch).
#[derive(Clone, Debug)]
pub struct CommandResult {
    /// Output lines to display.
    pub output: Vec<OutputLine>,
    /// POSIX exit code. 0 = success, non-zero = error.
    pub exit_code: i32,
    /// Side effects to perform after display.
    pub side_effects: Vec<SideEffect>,
}

impl CommandResult {
    /// Success with output, no side effect.
    pub fn output(lines: Vec<OutputLine>) -> Self {
        Self {
            output: lines,
            exit_code: 0,
            side_effects: Vec::new(),
        }
    }

    /// Error output with exit_code=1.
    pub fn error_line(message: impl Into<String>) -> Self {
        Self {
            output: vec![OutputLine::error(message.into())],
            exit_code: 1,
            side_effects: Vec::new(),
        }
    }

    /// Success, no output, no side effect.
    pub fn empty() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effects: Vec::new(),
        }
    }

    pub fn navigate(route: RouteRequest) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::Navigate(route)],
        }
    }

    pub fn login() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::Login],
        }
    }

    pub fn logout() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::Logout],
        }
    }

    pub fn switch_view(mode: ViewMode) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::SwitchView(mode)],
        }
    }

    pub fn open_explorer(route: RouteRequest) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effects: vec![SideEffect::SwitchViewAndNavigate(ViewMode::Explorer, route)],
        }
    }

    /// Override the exit code (chainable).
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }

    /// Append a target side effect (chainable).
    pub fn with_side_effect(mut self, effect: SideEffect) -> Self {
        self.side_effects.push(effect);
        self
    }
}

use std::fmt;

use std::collections::BTreeMap;

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

/// Target-provided shell execution context.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExecutionContext {
    pub system_info: SystemInfo,
    pub env: BTreeMap<String, String>,
    pub shell_text: ShellText,
}

/// Optional system facts supplied by the runtime shell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SystemInfo {
    pub uptime: Option<String>,
    pub user_agent: Option<String>,
}

/// Target-owned static shell text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellText {
    pub profile: &'static str,
    pub help: &'static str,
}

impl ShellText {
    pub const fn new(profile: &'static str, help: &'static str) -> Self {
        Self { profile, help }
    }
}

impl Default for ShellText {
    fn default() -> Self {
        Self::new("", "")
    }
}

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
    Theme(Option<String>),
    Clear,
    Echo(String),
    /// `export` command. Each element is one raw `KEY=value` assignment
    /// (or a bare `KEY` for display). Empty Vec prints all variables.
    Export(Vec<String>),
    Unset(Option<String>),
    Login,
    Logout,

    Refresh(Option<PathArg>),

    Unknown(String),
}

impl Command {
    /// Get all available command names for autocomplete.
    ///
    /// Includes both regular commands and pipe filter commands.
    pub fn names() -> &'static [&'static str] {
        &[
            "cat", "cd", "clear", "cls", "echo", "export", "grep", "head", "help", "id", "login",
            "logout", "ls", "pwd", "refresh", "tail", "theme", "unset", "wc", "whoami",
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
            "theme" => {
                if args.len() > 1 {
                    return Self::Unknown("theme".to_string());
                }
                Self::Theme(args.first().cloned())
            }
            "clear" | "cls" => Self::Clear,
            "echo" => Self::Echo(args.join(" ")),
            "export" => Self::Export(args.to_vec()),
            "unset" => Self::Unset(args.first().cloned()),
            "login" => Self::Login,
            "logout" => Self::Logout,
            "refresh" if args.len() <= 1 => Self::Refresh(args.first().map(PathArg::new)),
            _ => Self::Unknown(name.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{BootstrapSiteSource, RuntimeMount};
    use crate::engine::shell::execute_pipeline;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    fn bootstrap_source() -> BootstrapSiteSource {
        BootstrapSiteSource {
            repo_with_owner: "example/site",
            branch: "main",
            content_root: "content",
            gateway: "self",
        }
    }

    fn runtime_mounts() -> [RuntimeMount; 1] {
        [crate::engine::runtime::boot::bootstrap_runtime_mount(
            &bootstrap_source(),
        )]
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
    fn test_parse_theme() {
        assert!(matches!(Command::parse("theme", &[]), Command::Theme(None)));
        assert!(matches!(
            Command::parse("theme", &args(&["black-ink"])),
            Command::Theme(Some(ref theme)) if theme == "black-ink"
        ));
        assert!(matches!(
            Command::parse("theme", &args(&["a", "b"])),
            Command::Unknown(ref cmd) if cmd == "theme"
        ));
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
        assert!(names.contains(&"theme"));
        assert!(!names.contains(&"explorer"));
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
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();

        let pipeline = parse_input("login", &[]);
        let result = execute_pipeline(&pipeline, &wallet, &runtime_mounts(), &fs, &cwd);
        assert_eq!(
            result.side_effects.first().cloned(),
            Some(super::SideEffect::Login)
        );
    }

    #[test]
    fn test_pipeline_drops_side_effect_when_piped() {
        // When a command has filters attached, side effects are discarded.
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();

        let pipeline = parse_input("help | head -1", &[]);
        let result = execute_pipeline(&pipeline, &wallet, &runtime_mounts(), &fs, &cwd);
        assert!(result.side_effects.first().cloned().is_none());
    }

    #[test]
    fn test_pipeline_exit_code_is_last_stage() {
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();

        // `help | grep xyzzy` should exit 1 (grep no match)
        let pipeline = parse_input("help | grep xyzzy", &[]);
        let result = execute_pipeline(&pipeline, &wallet, &runtime_mounts(), &fs, &cwd);
        assert_eq!(result.exit_code, 1);
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
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();

        // Pipe with nothing on the right-hand side → parse error
        let pipeline = parse_input("ls |", &[]);
        let result = execute_pipeline(&pipeline, &wallet, &runtime_mounts(), &fs, &cwd);
        assert_eq!(result.exit_code, 2);
    }
    #[test]
    fn test_output_constructor() {
        let r = CommandResult::output(vec![OutputLine::text("hi")]);
        assert_eq!(r.exit_code, 0);
        assert!(r.side_effects.is_empty());
        assert_eq!(r.output.len(), 1);
    }

    #[test]
    fn test_error_line_constructor() {
        let r = CommandResult::error_line("boom");
        assert_eq!(r.exit_code, 1);
        assert!(r.side_effects.is_empty());
        assert_eq!(r.output.len(), 1);
    }

    #[test]
    fn test_navigate_constructor() {
        let route = RouteRequest::new("/websh/blog");
        let r = CommandResult::navigate(route.clone());
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.side_effects, vec![SideEffect::Navigate(route)]);
    }

    #[test]
    fn test_login_constructor() {
        let r = CommandResult::login();
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.side_effects, vec![SideEffect::Login]);
    }

    #[test]
    fn test_logout_constructor() {
        let r = CommandResult::logout();
        assert_eq!(r.side_effects, vec![SideEffect::Logout]);
    }

    #[test]
    fn test_with_exit_code() {
        let r = CommandResult::empty().with_exit_code(127);
        assert_eq!(r.exit_code, 127);
    }
}

#[cfg(test)]
mod read_only_tests {
    use super::*;
    #[test]
    fn output_operators_are_literal_text() {
        for input in [
            "echo hello > note.md",
            "echo hello >> note.md",
            "echo hello \">\" note.md",
        ] {
            let pipeline = crate::shell::parse_input(input, &[]);
            let command = &pipeline.commands[0];
            match Command::parse(&command.name, &command.args) {
                Command::Echo(text) => assert_eq!(text, command.args.join(" ")),
                other => panic!("unexpected command {other:?}"),
            }
        }
    }
}
