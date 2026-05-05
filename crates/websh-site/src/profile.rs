//! Deployed shell copy and profile text.

use websh_core::shell::ShellText;

pub const ASCII_BANNER: &str = include_str!("../assets/text/banner.txt");
pub const ASCII_PROFILE: &str = include_str!("../assets/text/profile.txt");
pub const HELP_TEXT: &str = include_str!("../assets/text/help.txt");

pub const SHELL_TEXT: ShellText = ShellText::new(ASCII_PROFILE, HELP_TEXT);
