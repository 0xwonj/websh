//! Command execution result type.

use crate::core::engine::RouteRequest;
use crate::models::{OutputLine, ViewMode};

/// Side effect requested by a command's execution.
///
/// Commands return side effects as data; the UI layer (or executor) is
/// responsible for actually performing them. This keeps command logic
/// testable without Leptos signals or async runtimes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SideEffect {
    /// Navigate to a new route.
    Navigate(RouteRequest),
    /// Initiate wallet login (async).
    Login,
    /// Perform wallet logout.
    Logout,
    /// Switch view mode (e.g., Terminal <-> Explorer).
    SwitchView(ViewMode),
    /// Switch view mode and navigate in one step.
    SwitchViewAndNavigate(ViewMode, RouteRequest),

    // Filesystem mutations
    ApplyChange {
        path: crate::models::VirtualPath,
        change: crate::core::changes::ChangeType,
    },
    StageChange {
        path: crate::models::VirtualPath,
    },
    UnstageChange {
        path: crate::models::VirtualPath,
    },
    DiscardChange {
        path: crate::models::VirtualPath,
    },
    StageAll,
    UnstageAll,
    Commit {
        message: String,
        expected_head: Option<String>,
        mount_root: crate::models::VirtualPath,
    },
    ReloadRuntimeMount {
        mount_root: crate::models::VirtualPath,
    },
    SetAuthToken {
        token: String,
    },
    ClearAuthToken,
    InvalidateRuntimeState,
    OpenEditor {
        path: crate::models::VirtualPath,
    },
}

/// Result of executing a command.
///
/// Carries output lines, a POSIX-style exit code, and an optional side
/// effect (navigation, wallet action, view switch).
#[derive(Clone, Debug)]
pub struct CommandResult {
    /// Output lines to display.
    pub output: Vec<OutputLine>,
    /// POSIX exit code. 0 = success, non-zero = error.
    pub exit_code: i32,
    /// Side effect to perform after display (if any).
    pub side_effect: Option<SideEffect>,
}

impl CommandResult {
    // --- Primary constructors ---

    /// Success with output, no side effect.
    pub fn output(lines: Vec<OutputLine>) -> Self {
        Self {
            output: lines,
            exit_code: 0,
            side_effect: None,
        }
    }

    /// Error output with exit_code=1.
    pub fn error_line(message: impl Into<String>) -> Self {
        Self {
            output: vec![OutputLine::error(message.into())],
            exit_code: 1,
            side_effect: None,
        }
    }

    /// Success, no output, no side effect.
    pub fn empty() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: None,
        }
    }

    // --- Side-effect constructors ---

    pub fn navigate(route: RouteRequest) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::Navigate(route)),
        }
    }

    pub fn login() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::Login),
        }
    }

    pub fn logout() -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::Logout),
        }
    }

    pub fn switch_view(mode: ViewMode) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::SwitchView(mode)),
        }
    }

    pub fn open_explorer(route: RouteRequest) -> Self {
        Self {
            output: vec![],
            exit_code: 0,
            side_effect: Some(SideEffect::SwitchViewAndNavigate(ViewMode::Explorer, route)),
        }
    }

    // --- Builder methods ---

    /// Override the exit code (chainable).
    pub fn with_exit_code(mut self, code: i32) -> Self {
        self.exit_code = code;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::engine::RouteRequest;
    use crate::models::{OutputLine, ViewMode};

    #[test]
    fn test_output_constructor() {
        let r = CommandResult::output(vec![OutputLine::text("hi")]);
        assert_eq!(r.exit_code, 0);
        assert!(r.side_effect.is_none());
        assert_eq!(r.output.len(), 1);
    }

    #[test]
    fn test_error_line_constructor() {
        let r = CommandResult::error_line("boom");
        assert_eq!(r.exit_code, 1);
        assert!(r.side_effect.is_none());
        assert_eq!(r.output.len(), 1);
    }

    #[test]
    fn test_navigate_constructor() {
        let route = RouteRequest::new("/fs/site/blog");
        let r = CommandResult::navigate(route.clone());
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.side_effect, Some(SideEffect::Navigate(route)));
    }

    #[test]
    fn test_login_constructor() {
        let r = CommandResult::login();
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.side_effect, Some(SideEffect::Login));
    }

    #[test]
    fn test_logout_constructor() {
        let r = CommandResult::logout();
        assert_eq!(r.side_effect, Some(SideEffect::Logout));
    }

    #[test]
    fn test_switch_view_constructor() {
        let r = CommandResult::switch_view(ViewMode::Explorer);
        assert_eq!(
            r.side_effect,
            Some(SideEffect::SwitchView(ViewMode::Explorer))
        );
    }

    #[test]
    fn test_open_explorer_constructor() {
        let route = RouteRequest::new("/fs/site/blog");
        let r = CommandResult::open_explorer(route.clone());
        assert_eq!(
            r.side_effect,
            Some(SideEffect::SwitchViewAndNavigate(ViewMode::Explorer, route))
        );
    }

    #[test]
    fn test_with_exit_code() {
        let r = CommandResult::empty().with_exit_code(127);
        assert_eq!(r.exit_code, 127);
    }
}
