//! Pipe filter commands (grep, head, tail, wc).
//!
//! These filters operate on output lines from other commands,
//! enabling Unix-style piping: `ls | grep foo | head -5`

use crate::config::pipe_filters;
use crate::models::{OutputLine, OutputLineData};

use super::CommandResult;

/// Apply a filter command to output lines.
pub fn apply_filter(cmd: &str, args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    match cmd.to_lowercase().as_str() {
        "grep" => filter_grep(args, lines),
        "head" => filter_head(args, lines),
        "tail" => filter_tail(args, lines),
        "wc" => filter_wc(lines),
        _ => CommandResult::error_line(format!(
            "Pipe: unknown filter '{}'. Supported: grep, head, tail, wc",
            cmd
        ))
        .with_exit_code(127),
    }
}

fn filter_grep(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let pattern = args.first().map(|s| s.as_str()).unwrap_or("");
    if pattern.is_empty() {
        return CommandResult::error_line("grep: missing pattern").with_exit_code(2);
    }

    let pattern_lower = pattern.to_lowercase();
    let matched: Vec<OutputLine> = lines
        .into_iter()
        .filter(|line| line_contains(&line.data, &pattern_lower))
        .collect();

    let exit_code = if matched.is_empty() { 1 } else { 0 };
    CommandResult::output(matched).with_exit_code(exit_code)
}

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

fn filter_head(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let n = parse_count_arg(args, pipe_filters::DEFAULT_HEAD_LINES);
    CommandResult::output(lines.into_iter().take(n).collect())
}

fn filter_tail(args: &[String], lines: Vec<OutputLine>) -> CommandResult {
    let n = parse_count_arg(args, pipe_filters::DEFAULT_TAIL_LINES);
    let len = lines.len();
    CommandResult::output(lines.into_iter().skip(len.saturating_sub(n)).collect())
}

fn filter_wc(lines: Vec<OutputLine>) -> CommandResult {
    let count = lines
        .iter()
        .filter(|l| !matches!(l.data, OutputLineData::Empty))
        .count();
    CommandResult::output(vec![OutputLine::text(format!("{}", count))])
}

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
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_case_insensitive() {
        let lines = vec![OutputLine::text("APPLE"), OutputLine::text("banana")];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "APPLE"));
    }

    #[test]
    fn test_grep_missing_pattern() {
        let lines = test_lines();
        let result = apply_filter("grep", &[], lines);
        assert_eq!(result.exit_code, 2);
        assert_eq!(result.output.len(), 1);
        assert!(
            matches!(&result.output[0].data, OutputLineData::Error(s) if s.contains("missing pattern"))
        );
    }

    #[test]
    fn test_grep_list_entry() {
        let lines = vec![
            OutputLine::dir_entry("project-alpha", "Alpha project"),
            OutputLine::dir_entry("project-beta", "Beta project"),
        ];
        let result = apply_filter("grep", &args(&["alpha"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
    }

    #[test]
    fn test_head_filter() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["3"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 3);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "apple"));
        assert!(matches!(&result.output[2].data, OutputLineData::Text(s) if s == "cherry"));
    }

    #[test]
    fn test_head_with_dash() {
        let lines = test_lines();
        let result = apply_filter("head", &args(&["-2"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
    }

    #[test]
    fn test_head_default() {
        let lines = test_lines();
        let result = apply_filter("head", &[], lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 5); // Default 10, but only 5 lines
    }

    #[test]
    fn test_tail_filter() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["2"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "date"));
        assert!(matches!(&result.output[1].data, OutputLineData::Text(s) if s == "elderberry"));
    }

    #[test]
    fn test_tail_with_dash() {
        let lines = test_lines();
        let result = apply_filter("tail", &args(&["-3"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 3);
    }

    #[test]
    fn test_wc_filter() {
        let lines = test_lines();
        let result = apply_filter("wc", &[], lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "5"));
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
        assert_eq!(result.exit_code, 0);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "2"));
    }

    #[test]
    fn test_unknown_filter() {
        let lines = test_lines();
        let result = apply_filter("unknown", &[], lines);
        assert_eq!(result.exit_code, 127);
        assert_eq!(result.output.len(), 1);
        assert!(
            matches!(&result.output[0].data, OutputLineData::Error(s) if s.contains("unknown filter"))
        );
    }

    #[test]
    fn test_grep_no_match_exit_1() {
        let lines = test_lines();
        let result = apply_filter("grep", &args(&["xyzzy"]), lines);
        assert_eq!(result.exit_code, 1);
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_grep_missing_pattern_exit_2() {
        let lines = test_lines();
        let result = apply_filter("grep", &[], lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_unknown_filter_exit_127() {
        let lines = test_lines();
        let result = apply_filter("zzz", &[], lines);
        assert_eq!(result.exit_code, 127);
    }
}
