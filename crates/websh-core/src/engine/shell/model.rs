//! Shell command model and result types.

//! Command execution result type.

use crate::engine::filesystem::RouteRequest;
use crate::engine::shell::{AccessPolicy, OutputLine};

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
    SetTheme {
        theme: String,
    },
    /// Request the target to list available color palettes.
    ListThemes,
    /// Set a target-owned user environment variable.
    SetEnvVar {
        key: String,
        value: String,
    },
    /// Remove a target-owned user environment variable.
    UnsetEnvVar {
        key: String,
    },
    /// Reset the terminal output ring buffer.
    ClearHistory,

    // Filesystem mutations
    ApplyChange {
        path: crate::domain::VirtualPath,
        change: Box<crate::domain::ChangeType>,
    },
    StageChange {
        path: crate::domain::VirtualPath,
    },
    UnstageChange {
        path: crate::domain::VirtualPath,
    },
    DiscardChange {
        path: crate::domain::VirtualPath,
    },
    StageAll,
    UnstageAll,
    Commit {
        message: String,
        mount_root: crate::domain::VirtualPath,
    },
    ReloadRuntimeMount {
        mount_root: crate::domain::VirtualPath,
    },
    SetAuthToken {
        token: String,
    },
    ClearAuthToken,
    InvalidateRuntimeState,
    OpenEditor {
        path: crate::domain::VirtualPath,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationEffect {
    Navigate(RouteRequest),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesystemEffect {
    ApplyChange {
        path: crate::domain::VirtualPath,
        change: Box<crate::domain::ChangeType>,
    },
    StageChange {
        path: crate::domain::VirtualPath,
    },
    UnstageChange {
        path: crate::domain::VirtualPath,
    },
    DiscardChange {
        path: crate::domain::VirtualPath,
    },
    StageAll,
    UnstageAll,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEffect {
    Commit {
        message: String,
        mount_root: crate::domain::VirtualPath,
    },
    ReloadRuntimeMount {
        mount_root: crate::domain::VirtualPath,
    },
    InvalidateRuntimeState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthEffect {
    Login,
    Logout,
    SetAuthToken { token: String },
    ClearAuthToken,
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
pub enum EditorEffect {
    OpenEditor { path: crate::domain::VirtualPath },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemEffect {
    ClearHistory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellEffect {
    Navigation(NavigationEffect),
    Filesystem(FilesystemEffect),
    Runtime(RuntimeEffect),
    Auth(AuthEffect),
    Theme(ThemeEffect),
    Environment(EnvironmentEffect),
    View(ViewEffect),
    Editor(EditorEffect),
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
            SideEffect::ApplyChange { path, change } => {
                Self::Filesystem(FilesystemEffect::ApplyChange { path, change })
            }
            SideEffect::StageChange { path } => {
                Self::Filesystem(FilesystemEffect::StageChange { path })
            }
            SideEffect::UnstageChange { path } => {
                Self::Filesystem(FilesystemEffect::UnstageChange { path })
            }
            SideEffect::DiscardChange { path } => {
                Self::Filesystem(FilesystemEffect::DiscardChange { path })
            }
            SideEffect::StageAll => Self::Filesystem(FilesystemEffect::StageAll),
            SideEffect::UnstageAll => Self::Filesystem(FilesystemEffect::UnstageAll),
            SideEffect::Commit {
                message,
                mount_root,
            } => Self::Runtime(RuntimeEffect::Commit {
                message,
                mount_root,
            }),
            SideEffect::ReloadRuntimeMount { mount_root } => {
                Self::Runtime(RuntimeEffect::ReloadRuntimeMount { mount_root })
            }
            SideEffect::SetAuthToken { token } => Self::Auth(AuthEffect::SetAuthToken { token }),
            SideEffect::ClearAuthToken => Self::Auth(AuthEffect::ClearAuthToken),
            SideEffect::InvalidateRuntimeState => {
                Self::Runtime(RuntimeEffect::InvalidateRuntimeState)
            }
            SideEffect::OpenEditor { path } => Self::Editor(EditorEffect::OpenEditor { path }),
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
    pub access_policy: AccessPolicy,
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

    // Write / sync commands.
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
            "cat", "cd", "clear", "cls", "echo", "edit", "export", "grep", "head", "help", "id",
            "login", "logout", "ls", "mkdir", "pwd", "rm", "rmdir", "sync", "tail", "theme",
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
            "theme" => {
                if args.len() > 1 {
                    return Self::Unknown("theme".to_string());
                }
                Self::Theme(args.first().cloned())
            }
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
            writable: true,
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
        use crate::domain::ChangeSet;
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();
        let changes = ChangeSet::new();

        let pipeline = parse_input("login", &[]);
        let result = execute_pipeline(
            &pipeline,
            &wallet,
            &runtime_mounts(),
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert_eq!(
            result.side_effects.first().cloned(),
            Some(super::SideEffect::Login)
        );
    }

    #[test]
    fn test_pipeline_drops_side_effect_when_piped() {
        // When a command has filters attached, side effects are discarded.
        use crate::domain::ChangeSet;
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();
        let changes = ChangeSet::new();

        let pipeline = parse_input("help | head -1", &[]);
        let result = execute_pipeline(
            &pipeline,
            &wallet,
            &runtime_mounts(),
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert!(result.side_effects.first().cloned().is_none());
    }

    #[test]
    fn test_pipeline_exit_code_is_last_stage() {
        use crate::domain::ChangeSet;
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();
        let changes = ChangeSet::new();

        // `help | grep xyzzy` should exit 1 (grep no match)
        let pipeline = parse_input("help | grep xyzzy", &[]);
        let result = execute_pipeline(
            &pipeline,
            &wallet,
            &runtime_mounts(),
            &fs,
            &cwd,
            &changes,
            None,
        );
        assert_eq!(result.exit_code, 1);
    }

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
        use crate::engine::shell::parser::parse_input;

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
        use crate::domain::ChangeSet;
        use crate::domain::{VirtualPath, WalletState};
        use crate::engine::filesystem::GlobalFs;
        use crate::engine::shell::parser::parse_input;

        let wallet = WalletState::Disconnected;
        let fs = GlobalFs::empty();
        let cwd = VirtualPath::root();
        let changes = ChangeSet::new();

        // Pipe with nothing on the right-hand side → parse error
        let pipeline = parse_input("ls |", &[]);
        let result = execute_pipeline(
            &pipeline,
            &wallet,
            &runtime_mounts(),
            &fs,
            &cwd,
            &changes,
            None,
        );
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
