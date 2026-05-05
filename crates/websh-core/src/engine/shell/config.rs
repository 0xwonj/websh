//! Shell command defaults.

/// Pipe filter defaults.
pub(crate) mod pipe_filters {
    /// Default number of lines for `head` command.
    pub const DEFAULT_HEAD_LINES: usize = 10;
    /// Default number of lines for `tail` command.
    pub const DEFAULT_TAIL_LINES: usize = 10;
}
