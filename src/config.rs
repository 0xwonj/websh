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

/// Home directory path.
pub const HOME_DIR: &str = "/home/wonjae";

/// User profile file path.
pub const PROFILE_PATH: &str = "/home/wonjae/.profile";

// =============================================================================
// Network Configuration
// =============================================================================

/// Base URL for fetching content from external repository.
pub const CONTENT_BASE_URL: &str = "https://raw.githubusercontent.com/0xwonj/db/main";

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

/// Ethereum address display format constants.
pub mod eth_address {
    /// Length of address prefix to show (e.g., "0x1234").
    pub const PREFIX_LEN: usize = 6;
    /// Start index of address suffix (for 42-char address).
    pub const SUFFIX_START: usize = 38;
    /// Full length of an Ethereum address with 0x prefix.
    pub const FULL_LEN: usize = 42;
}

// =============================================================================
// Environment Variables
// =============================================================================

/// Prefix for user environment variables in localStorage.
pub const USER_VAR_PREFIX: &str = "user.";

/// Default user variables initialized on first visit.
pub const DEFAULT_USER_VARS: &[(&str, &str)] = &[
    ("THEME", "dark"),
    ("LANG", "en"),
    ("EDITOR", "vim"),
];

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
