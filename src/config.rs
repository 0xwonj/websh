//! Application configuration.
//!
//! Centralizes all configuration constants used throughout the application.
//! Text assets are loaded at compile time using `include_str!`.

// =============================================================================
// Text Assets (loaded at compile time)
// =============================================================================

/// ASCII banner displayed after boot sequence.
pub const ASCII_BANNER: &str = include_str!("../assets/text/banner.txt");

/// ASCII profile card for `whoami` command.
pub const ASCII_PROFILE: &str = include_str!("../assets/text/profile.txt");

/// Help text for `help` command.
pub const HELP_TEXT: &str = include_str!("../assets/text/help.txt");

// =============================================================================
// Application Metadata
// =============================================================================

/// Application name displayed in terminal.
pub const APP_NAME: &str = "wonjae.eth";

/// Application version.
pub const APP_VERSION: &str = "0.1.0";

/// User tagline displayed after boot.
pub const APP_TAGLINE: &str =
    "Applied Cryptography Researcher | Zero-Knowledge Proofs | Blockchain Security";

// =============================================================================
// Filesystem Configuration
// =============================================================================

/// Profile file name (relative to mount root).
pub const PROFILE_FILE: &str = ".profile";

// =============================================================================
// Network Configuration
// =============================================================================

/// Fetch request timeout in milliseconds.
pub const FETCH_TIMEOUT_MS: i32 = 10000;

/// Allowed domains for external link redirects (security).
/// Links to other domains will be blocked.
pub const ALLOWED_REDIRECT_DOMAINS: &[&str] = &[
    "github.com",
    "twitter.com",
    "x.com",
    "linkedin.com",
    "etherscan.io",
    "arbiscan.io",
    "optimistic.etherscan.io",
    "basescan.org",
    "polygonscan.com",
    "medium.com",
    "mirror.xyz",
    "notion.so",
    "docs.google.com",
    "drive.google.com",
    "youtube.com",
    "youtu.be",
];

// =============================================================================
// Wallet Configuration
// =============================================================================

/// localStorage key for wallet session persistence.
pub const WALLET_SESSION_KEY: &str = "wallet_session";

/// Wallet connection timeout in milliseconds.
pub const WALLET_TIMEOUT_MS: i32 = 2000;

// =============================================================================
// Admin Configuration
// =============================================================================

/// Admin wallet addresses (case-insensitive).
///
/// These addresses have write access to the filesystem when authenticated.
/// Add your wallet address here to enable admin features.
pub const ADMIN_ADDRESSES: &[&str] = &[
    // Add admin wallet addresses here:
    "0x2c4b04a4aeb6e18c2f8a5c8b4a3f62c0cf33795a",
];

/// localStorage key for GitHub Personal Access Token.
pub const GITHUB_TOKEN_KEY: &str = "github_token";

/// localStorage key for pending changes.
pub const PENDING_CHANGES_KEY: &str = "pending_changes";

// =============================================================================
// Environment Variables
// =============================================================================

/// Prefix for user environment variables in localStorage.
pub const USER_VAR_PREFIX: &str = "user.";

/// Default user variables initialized on first visit.
pub const DEFAULT_USER_VARS: &[(&str, &str)] =
    &[("THEME", "dark"), ("LANG", "en"), ("EDITOR", "vim")];

// =============================================================================
// Terminal Configuration
// =============================================================================

/// Maximum number of terminal output lines to keep in history.
pub const MAX_TERMINAL_HISTORY: usize = 1000;

/// Maximum number of command history entries to keep.
pub const MAX_COMMAND_HISTORY: usize = 100;

/// Pipe filter defaults.
pub mod pipe_filters {
    /// Default number of lines for `head` command.
    pub const DEFAULT_HEAD_LINES: usize = 10;
    /// Default number of lines for `tail` command.
    pub const DEFAULT_TAIL_LINES: usize = 10;
}

/// Display truncation limits.
pub mod display {
    /// Maximum length of variable value before truncation in `export` output.
    pub const MAX_VAR_DISPLAY_LEN: usize = 60;
    /// Length of truncated preview (with "..." appended).
    pub const TRUNCATED_PREVIEW_LEN: usize = 57;
}

// =============================================================================
// Boot Sequence Configuration
// =============================================================================

/// Boot sequence animation delay constants (milliseconds).
pub mod boot_delays {
    /// Delay after kernel init message.
    pub const KERNEL_INIT: i32 = 30;
    /// Delay after WASM runtime message.
    pub const WASM_RUNTIME: i32 = 20;
    /// Delay after boot complete message.
    pub const BOOT_COMPLETE: i32 = 40;
}

// =============================================================================
// Time Constants
// =============================================================================

/// Milliseconds per second for time formatting.
pub const MS_PER_SECOND: f64 = 1000.0;

// =============================================================================
// Cache Configuration
// =============================================================================

/// Session cache configuration.
pub mod cache {
    /// sessionStorage key for manifest cache.
    pub const MANIFEST_KEY: &str = "manifest_cache";
}

// =============================================================================
// UI Configuration
// =============================================================================

/// Icon theme selection.
///
/// Available themes:
/// - `Bootstrap` - Familiar, slightly bolder (default)
/// - `Lucide` - Minimal, thin strokes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum IconTheme {
    #[default]
    Bootstrap,
    Lucide,
}

/// Current icon theme used throughout the application.
/// Change this value to switch icon styles globally.
pub const ICON_THEME: IconTheme = IconTheme::Bootstrap;

// =============================================================================
// Mount Configuration
// =============================================================================

use crate::models::Mount;

/// Get the configured mounts for the application.
///
/// This function defines all available filesystem mounts.
/// The first mount in the list is considered the home mount.
///
/// # Customization
///
/// To add additional mounts, add more entries to the vector:
/// ```ignore
/// use crate::models::{Mount, Storage};
/// vec![
///     Mount::github_with_prefix("~", "user", "repo", "main", "~"),
///     Mount::new("work", Storage::github("company", "repo", "main")),
///     Mount::new("data", Storage::ipfs("QmXyz123")),
/// ]
/// ```
pub fn configured_mounts() -> Vec<Mount> {
    vec![Mount::github_with_prefix("~", "0xwonj", "db", "main", "~")]
}

/// Get the default (home) mount.
///
/// Returns the first configured mount, which is typically the home mount ("~").
/// Panics if no mounts are configured.
pub fn default_mount() -> Mount {
    configured_mounts()
        .into_iter()
        .next()
        .expect("At least one mount must be configured")
}

/// Get the default base URL for content fetching.
///
/// Returns the content base URL of the default mount.
pub fn default_base_url() -> String {
    default_mount().content_base_url()
}
