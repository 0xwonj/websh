//! Command execution result type.

use crate::core::storage::{PendingChanges, StagedChanges};
use crate::models::{AppRoute, OutputLine};

/// Result of executing a command.
///
/// Commands can produce output, request navigation, and update pending/staged changes.
#[derive(Clone, Debug)]
pub struct CommandResult {
    /// Output lines to display
    pub output: Vec<OutputLine>,
    /// Optional route to navigate to (e.g., for `cd` command)
    pub navigate_to: Option<AppRoute>,
    /// Updated pending changes (for admin commands like touch, mkdir, rm)
    pub pending: Option<PendingChanges>,
    /// Updated staged changes (for sync add/reset commands)
    pub staged: Option<StagedChanges>,
}

impl CommandResult {
    /// Create a result with just output, no navigation.
    pub fn output(lines: Vec<OutputLine>) -> Self {
        Self {
            output: lines,
            navigate_to: None,
            pending: None,
            staged: None,
        }
    }

    /// Create a result with navigation and optional output.
    pub fn navigate(route: AppRoute) -> Self {
        Self {
            output: vec![],
            navigate_to: Some(route),
            pending: None,
            staged: None,
        }
    }

    /// Create an empty result (no output, no navigation).
    pub fn empty() -> Self {
        Self {
            output: vec![],
            navigate_to: None,
            pending: None,
            staged: None,
        }
    }

    /// Create a result with output and updated pending changes.
    pub fn with_pending(lines: Vec<OutputLine>, pending: PendingChanges) -> Self {
        Self {
            output: lines,
            navigate_to: None,
            pending: Some(pending),
            staged: None,
        }
    }

    /// Create a result with output and updated staged changes.
    pub fn with_staged(lines: Vec<OutputLine>, staged: StagedChanges) -> Self {
        Self {
            output: lines,
            navigate_to: None,
            pending: None,
            staged: Some(staged),
        }
    }
}
