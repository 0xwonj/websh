//! Pipe filter commands (grep, head, tail, wc).
//!
//! These filters operate on output lines from other commands,
//! enabling Unix-style piping: `ls | grep foo | head -5`

use crate::config::pipe_filters;
use crate::models::{OutputLine, OutputLineData};

/// Apply a filter command to output lines.
///
/// # Supported filters
/// - `grep <pattern>`: Filter lines containing pattern (case-insensitive)
/// - `head [-n]`: Take first n lines (default: 10)
/// - `tail [-n]`: Take last n lines (default: 10)
/// - `wc`: Count non-empty lines
pub fn apply_filter(cmd: &str, args: &[String], lines: Vec<OutputLine>) -> Vec<OutputLine> {
    match cmd.to_lowercase().as_str() {
        "grep" => filter_grep(args, lines),
        "head" => filter_head(args, lines),
        "tail" => filter_tail(args, lines),
        "wc" => filter_wc(lines),
        _ => vec![OutputLine::error(format!(
            "Pipe: unknown filter '{}'. Supported: grep, head, tail, wc",
            cmd
        ))],
    }
}

/// Filter lines containing a pattern (case-insensitive).
fn filter_grep(args: &[String], lines: Vec<OutputLine>) -> Vec<OutputLine> {
    let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return vec![OutputLine::error("grep: missing pattern")];
    }

    let pattern_lower = pattern.to_lowercase();
    lines
        .into_iter()
        .filter(|line| line_contains(&line.data, &pattern_lower))
        .collect()
}

/// Check if an output line contains the pattern.
fn line_contains(data: &OutputLineData, pattern: &str) -> bool {
    match data {
        OutputLineData::Text(s)
        | OutputLineData::Error(s)
        | OutputLineData::Success(s)
        | OutputLineData::Info(s)
        | OutputLineData::Ascii(s) => s.to_lowercase().contains(pattern),
        OutputLineData::ListEntry { name, .. } => name.to_lowercase().contains(pattern),
        OutputLineData::Command { input, .. } => input.to_lowercase().contains(pattern),
        OutputLineData::Empty => false,
    }
}

/// Take first n lines.
fn filter_head(args: &[String], lines: Vec<OutputLine>) -> Vec<OutputLine> {
    let n = parse_count_arg(args, pipe_filters::DEFAULT_HEAD_LINES);
    lines.into_iter().take(n).collect()
}

/// Take last n lines.
fn filter_tail(args: &[String], lines: Vec<OutputLine>) -> Vec<OutputLine> {
    let n = parse_count_arg(args, pipe_filters::DEFAULT_TAIL_LINES);
    let len = lines.len();
    lines.into_iter().skip(len.saturating_sub(n)).collect()
}

/// Count non-empty lines.
fn filter_wc(lines: Vec<OutputLine>) -> Vec<OutputLine> {
    let count = lines
        .iter()
        .filter(|l| !matches!(l.data, OutputLineData::Empty))
        .count();
    vec![OutputLine::text(format!("{}", count))]
}

/// Parse a count argument like "5" or "-5".
fn parse_count_arg(args: &[String], default: usize) -> usize {
    args.first()
        .and_then(|s| s.trim_start_matches('-').parse().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    fn test_lines() -> Vec<OutputLine> {
        vec![
            OutputLine::text("apple"),
            OutputLine::text("banana"),
            OutputLine::text("cherry"),
            OutputLine::text("date"),
            OutputLine::text("elderberry"),
        ]
    }

    #[test]
    fn test_grep_filter() {
        let lines = test_lines();
        let result = apply_filter("grep", &args(&["an"]), lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_case_insensitive() {
        let lines = vec![OutputLine::text("APPLE"), OutputLine::text("banana")];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "APPLE"));
    }

    #[test]
    fn test_grep_missing_pattern() {
        let lines = test_lines();
        let result = apply_filter("grep", &[], lines);
        assert_eq!(result.len(), 1);
        assert!(
            matches!(&result[0].data, OutputLineData::Error(s) if s.contains("missing pattern"))
        );
    }

    #[test]
    fn test_grep_list_entry() {
        let lines = vec![
            OutputLine::dir_entry("project-alpha", "Alpha project"),
            OutputLine::dir_entry("project-beta", "Beta project"),
        ];
        let result = apply_filter("grep", &args(&["alpha"]), lines);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_head_filter() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["3"]), lines);
        assert_eq!(result.len(), 3);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "apple"));
        assert!(matches!(&result[2].data, OutputLineData::Text(s) if s == "cherry"));
    }

    #[test]
    fn test_head_with_dash() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["-2"]), lines);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_head_default() {
        let lines = test_lines();
        let result = apply_filter("head", &[], lines);
        assert_eq!(result.len(), 5); // Default 10, but only 5 lines
    }

    #[test]
    fn test_tail_filter() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["2"]), lines);
        assert_eq!(result.len(), 2);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "date"));
        assert!(matches!(&result[1].data, OutputLineData::Text(s) if s == "elderberry"));
    }

    #[test]
    fn test_tail_with_dash() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["-3"]), lines);
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_wc_filter() {
        let lines = test_lines();
        let result = apply_filter("wc", &[], lines);
        assert_eq!(result.len(), 1);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "5"));
    }

    #[test]
    fn test_wc_excludes_empty() {
        let lines = vec![
            OutputLine::text("line1"),
            OutputLine::empty(),
            OutputLine::text("line2"),
            OutputLine::empty(),
        ];
        let result = apply_filter("wc", &[], lines);
        assert!(matches!(&result[0].data, OutputLineData::Text(s) if s == "2"));
    }

    #[test]
    fn test_unknown_filter() {
        let lines = test_lines();
        let result = apply_filter("unknown", &[], lines);
        assert_eq!(result.len(), 1);
        assert!(
            matches!(&result[0].data, OutputLineData::Error(s) if s.contains("unknown filter"))
        );
    }
}
