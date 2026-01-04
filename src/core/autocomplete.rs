//! Tab autocomplete functionality for terminal commands and paths.
//!
//! This module provides intelligent autocompletion for:
//! - Command names (e.g., "cl" → "clear")
//! - Directory paths for `cd`, `ls` commands
//! - File paths for `cat`, `less`, `more` commands
//!
//! The autocomplete system supports:
//! - Single match: Complete immediately
//! - Multiple matches: Show common prefix and all options
//! - Ghost text hints while typing

use crate::core::{Command, VirtualFs};
use crate::models::VirtualPath;

// ============================================================================
// Public Types
// ============================================================================

/// Result of an autocomplete attempt.
#[derive(Clone, Debug, PartialEq)]
pub enum AutocompleteResult {
    /// Single exact match - complete with this value.
    Single(String),
    /// Multiple matches - (common_prefix, all_matches).
    Multiple(String, Vec<String>),
    /// No matches found.
    None,
}

// ============================================================================
// Configuration
// ============================================================================

/// Commands that accept directory paths as arguments.
const DIR_COMMANDS: &[&str] = &["cd", "ls"];

/// Commands that accept file paths as arguments.
const FILE_COMMANDS: &[&str] = &["cat", "less", "more"];

// ============================================================================
// Completion Context
// ============================================================================

/// Determines what type of completion is needed for a command.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CompletionMode {
    /// Complete command names only.
    Command,
    /// Complete directory paths (for cd, ls).
    DirectoryPath,
    /// Complete file paths (for cat, less, more).
    FilePath,
    /// No completion available.
    None,
}

impl CompletionMode {
    /// Determine completion mode from input.
    fn from_input(input: &str) -> (Self, Vec<&str>) {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();

        if parts.len() == 1 {
            return (Self::Command, parts);
        }

        let cmd_lower = parts[0].to_lowercase();
        let mode = if DIR_COMMANDS.contains(&cmd_lower.as_str()) {
            Self::DirectoryPath
        } else if FILE_COMMANDS.contains(&cmd_lower.as_str()) {
            Self::FilePath
        } else {
            Self::None
        };

        (mode, parts)
    }

    /// Returns true if this mode only matches directories.
    fn dirs_only(self) -> bool {
        matches!(self, Self::DirectoryPath)
    }
}

// ============================================================================
// Path Parsing
// ============================================================================

/// Parsed path components for autocomplete.
struct ParsedPath<'a> {
    /// Directory prefix (e.g., "projects/" or "").
    dir_part: &'a str,
    /// Filename/directory name being completed.
    name_part: &'a str,
    /// Resolved search directory path.
    search_dir: VirtualPath,
}

impl<'a> ParsedPath<'a> {
    /// Parse a partial path and resolve the search directory.
    fn parse(partial: &'a str, current_path: &VirtualPath, fs: &VirtualFs) -> Option<Self> {
        let (dir_part, name_part) = match partial.rfind('/') {
            Some(idx) => (&partial[..=idx], &partial[idx + 1..]),
            None => ("", partial),
        };

        let search_dir = if dir_part.is_empty() {
            current_path.clone()
        } else {
            fs.resolve_path(current_path, dir_part.trim_end_matches('/'))?
        };

        Some(Self {
            dir_part,
            name_part,
            search_dir,
        })
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Perform autocomplete on Tab press.
///
/// Returns a completion result based on the current input and filesystem state.
pub fn autocomplete(input: &str, current_path: &VirtualPath, fs: &VirtualFs) -> AutocompleteResult {
    let input = input.trim_start();
    if input.is_empty() {
        return AutocompleteResult::None;
    }

    let (mode, parts) = CompletionMode::from_input(input);

    match mode {
        CompletionMode::Command => complete_command(parts[0]),
        CompletionMode::DirectoryPath | CompletionMode::FilePath => {
            complete_path(parts[0], parts[1], current_path, fs, mode.dirs_only())
        }
        CompletionMode::None => AutocompleteResult::None,
    }
}

/// Get autocomplete suggestion for ghost text hint (while typing).
///
/// Returns the suffix that would complete the current input.
pub fn get_hint(input: &str, current_path: &VirtualPath, fs: &VirtualFs) -> Option<String> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }

    let (mode, parts) = CompletionMode::from_input(input);

    match mode {
        CompletionMode::Command => get_command_hint(parts[0]),
        CompletionMode::DirectoryPath | CompletionMode::FilePath => {
            get_path_hint(parts[1], current_path, fs, mode.dirs_only())
        }
        CompletionMode::None => None,
    }
}

// ============================================================================
// Command Completion
// ============================================================================

/// Complete command name.
fn complete_command(partial: &str) -> AutocompleteResult {
    let partial_lower = partial.to_lowercase();
    let matches: Vec<String> = Command::names()
        .iter()
        .filter(|cmd| cmd.starts_with(&partial_lower))
        .map(|s| s.to_string())
        .collect();

    match matches.len() {
        0 => AutocompleteResult::None,
        1 => AutocompleteResult::Single(format!("{} ", matches[0])),
        _ => {
            let common = find_common_prefix(&matches);
            AutocompleteResult::Multiple(common, matches)
        }
    }
}

/// Get hint for command name completion.
fn get_command_hint(partial: &str) -> Option<String> {
    let partial_lower = partial.to_lowercase();
    Command::names()
        .iter()
        .find(|cmd| cmd.starts_with(&partial_lower) && **cmd != partial_lower)
        .map(|cmd| cmd[partial.len()..].to_string())
}

// ============================================================================
// Path Completion
// ============================================================================

/// Complete file/directory path.
fn complete_path(
    cmd: &str,
    partial: &str,
    current_path: &VirtualPath,
    fs: &VirtualFs,
    dirs_only: bool,
) -> AutocompleteResult {
    let Some(parsed) = ParsedPath::parse(partial, current_path, fs) else {
        return AutocompleteResult::None;
    };

    let Some(entries) = fs.list_dir(parsed.search_dir.as_str()) else {
        return AutocompleteResult::None;
    };

    let matches = get_matching_entries(&entries, parsed.name_part, dirs_only);
    build_path_result(cmd, &parsed, matches)
}

/// Get hint for path completion.
fn get_path_hint(
    partial: &str,
    current_path: &VirtualPath,
    fs: &VirtualFs,
    dirs_only: bool,
) -> Option<String> {
    let parsed = ParsedPath::parse(partial, current_path, fs)?;
    let entries = fs.list_dir(parsed.search_dir.as_str())?;
    let matches = get_matching_entries(&entries, parsed.name_part, dirs_only);

    // Find first match that extends current input
    let name_lower = parsed.name_part.to_lowercase();
    matches
        .iter()
        .find(|(name, _)| name.to_lowercase() != name_lower)
        .map(|(name, is_dir)| {
            let suffix = if *is_dir { "/" } else { "" };
            format!("{}{}", &name[parsed.name_part.len()..], suffix)
        })
}

/// Get filtered entries matching the partial name.
fn get_matching_entries<'a>(
    entries: &'a [(String, bool, String)],
    name_part: &str,
    dirs_only: bool,
) -> Vec<(&'a String, bool)> {
    let name_lower = name_part.to_lowercase();
    entries
        .iter()
        .filter(|(name, is_dir, _)| {
            if dirs_only && !is_dir {
                return false;
            }
            name.to_lowercase().starts_with(&name_lower)
        })
        .map(|(name, is_dir, _)| (name, *is_dir))
        .collect()
}

/// Build the autocomplete result from matched paths.
fn build_path_result(
    cmd: &str,
    parsed: &ParsedPath,
    matches: Vec<(&String, bool)>,
) -> AutocompleteResult {
    // Build full paths with directory info
    let full_matches: Vec<(String, bool)> = matches
        .iter()
        .map(|(name, is_dir)| {
            let full_path = format!("{}{}", parsed.dir_part, name);
            (full_path, *is_dir)
        })
        .collect();

    match full_matches.len() {
        0 => AutocompleteResult::None,
        1 => {
            let (path, is_dir) = &full_matches[0];
            let suffix = if *is_dir { "/" } else { " " };
            AutocompleteResult::Single(format!("{} {}{}", cmd, path, suffix))
        }
        _ => {
            let paths: Vec<String> = full_matches.iter().map(|(p, _)| p.clone()).collect();
            let common = find_common_prefix(&paths);

            let display_names: Vec<String> = full_matches
                .iter()
                .map(|(path, is_dir)| {
                    let name = path.rsplit('/').next().unwrap_or(path);
                    if *is_dir {
                        format!("{}/", name)
                    } else {
                        name.to_string()
                    }
                })
                .collect();

            let common_with_cmd = format!("{} {}", cmd, common);
            AutocompleteResult::Multiple(common_with_cmd, display_names)
        }
    }
}

// ============================================================================
// Utilities
// ============================================================================

/// Find the common prefix of multiple strings (case-insensitive).
fn find_common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }
    if strings.len() == 1 {
        return strings[0].clone();
    }

    let first = &strings[0];
    let mut prefix_len = first.len();

    for s in &strings[1..] {
        prefix_len = first
            .chars()
            .zip(s.chars())
            .take(prefix_len)
            .take_while(|(a, b)| a.to_lowercase().eq(b.to_lowercase()))
            .count();
    }

    first[..prefix_len].to_string()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_completion_single() {
        match complete_command("cle") {
            AutocompleteResult::Single(s) => assert_eq!(s, "clear "),
            _ => panic!("Expected single match"),
        }
    }

    #[test]
    fn test_command_completion_multiple() {
        match complete_command("c") {
            AutocompleteResult::Multiple(common, matches) => {
                assert_eq!(common, "c");
                assert!(matches.contains(&"cat".to_string()));
                assert!(matches.contains(&"cd".to_string()));
                assert!(matches.contains(&"clear".to_string()));
            }
            _ => panic!("Expected multiple matches"),
        }
    }

    #[test]
    fn test_no_match() {
        assert_eq!(complete_command("xyz"), AutocompleteResult::None);
    }

    #[test]
    fn test_common_prefix() {
        let strings = vec![
            "hello".to_string(),
            "help".to_string(),
            "helicopter".to_string(),
        ];
        assert_eq!(find_common_prefix(&strings), "hel");
    }

    #[test]
    fn test_completion_mode() {
        let (mode, _) = CompletionMode::from_input("cd");
        assert_eq!(mode, CompletionMode::Command);

        let (mode, _) = CompletionMode::from_input("cd some/path");
        assert_eq!(mode, CompletionMode::DirectoryPath);

        let (mode, _) = CompletionMode::from_input("cat file.txt");
        assert_eq!(mode, CompletionMode::FilePath);

        let (mode, _) = CompletionMode::from_input("whoami arg");
        assert_eq!(mode, CompletionMode::None);
    }
}
