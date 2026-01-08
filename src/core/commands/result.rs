//! Command execution result type.

use crate::models::{AppRoute, OutputLine};

/// Result of executing a command.
///
/// Commands can produce output and optionally request navigation to a new route.
#[derive(Clone, Debug)]
pub struct CommandResult {
    /// Output lines to display
    pub output: Vec<OutputLine>,
    /// Optional route to navigate to (e.g., for `cd` command)
    pub navigate_to: Option<AppRoute>,
}

impl CommandResult {
    /// Create a result with just output, no navigation.
    pub fn output(lines: Vec<OutputLine>) -> Self {
        Self {
            output: lines,
            navigate_to: None,
        }
    }

    /// Create a result with navigation and optional output.
    pub fn navigate(route: AppRoute) -> Self {
        Self {
            output: vec![],
            navigate_to: Some(route),
        }
    }

    /// Create an empty result (no output, no navigation).
    pub fn empty() -> Self {
        Self {
            output: vec![],
            navigate_to: None,
        }
    }
}
