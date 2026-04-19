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
    // Parse flags and pattern.
    let mut ignore_case = false;
    let mut invert = false;
    let mut pattern: Option<&str> = None;

    for arg in args {
        if arg.starts_with("--") {
            match arg.as_str() {
                "--ignore-case" => ignore_case = true,
                "--invert-match" => invert = true,
                "--extended-regexp" => {} // no-op: regex crate is always extended
                _ => {
                    return CommandResult::error_line(format!(
                        "grep: unknown option: {}",
                        arg
                    ))
                    .with_exit_code(2);
                }
            }
        } else if let Some(rest) = arg.strip_prefix('-') {
            if rest.is_empty() {
                // A bare "-" is not a flag; treat as pattern if pattern is None.
                if pattern.is_none() {
                    pattern = Some(arg.as_str());
                } else {
                    return CommandResult::error_line(
                        "grep: unexpected extra argument".to_string(),
                    )
                    .with_exit_code(2);
                }
            } else {
                for ch in rest.chars() {
                    match ch {
                        'i' => ignore_case = true,
                        'v' => invert = true,
                        'E' => {} // no-op
                        other => {
                            return CommandResult::error_line(format!(
                                "grep: unknown option: -{}",
                                other
                            ))
                            .with_exit_code(2);
                        }
                    }
                }
            }
        } else if pattern.is_none() {
            pattern = Some(arg.as_str());
        } else {
            // extra positional arg: not supported
            return CommandResult::error_line(
                "grep: unexpected extra argument".to_string(),
            )
            .with_exit_code(2);
        }
    }

    let Some(pat) = pattern else {
        return CommandResult::error_line("grep: missing pattern").with_exit_code(2);
    };

    // Compile regex (with case-insensitive flag if requested).
    let regex = match build_grep_regex(pat, ignore_case) {
        Ok(r) => r,
        Err(e) => {
            return CommandResult::error_line(format!("grep: invalid regex: {}", e))
                .with_exit_code(2);
        }
    };

    let matched: Vec<OutputLine> = lines
        .into_iter()
        .filter(|line| {
            let is_match = regex_matches_line(&regex, &line.data);
            is_match ^ invert
        })
        .collect();

    let exit_code = if matched.is_empty() { 1 } else { 0 };
    CommandResult::output(matched).with_exit_code(exit_code)
}

fn build_grep_regex(pattern: &str, ignore_case: bool) -> Result<regex::Regex, regex::Error> {
    regex::RegexBuilder::new(pattern)
        .case_insensitive(ignore_case)
        .build()
}

fn regex_matches_line(re: &regex::Regex, data: &OutputLineData) -> bool {
    match data {
        OutputLineData::Text(s)
        | OutputLineData::Error(s)
        | OutputLineData::Success(s)
        | OutputLineData::Info(s)
        | OutputLineData::Ascii(s) => re.is_match(s),
        OutputLineData::ListEntry { name, .. } => re.is_match(name),
        OutputLineData::Command { input, .. } => re.is_match(input),
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
    fn test_grep_case_sensitive_default_rejects_uppercase() {
        let lines = vec![OutputLine::text("APPLE"), OutputLine::text("banana")];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        // default is case-sensitive, so "APPLE" doesn't match
        assert_eq!(result.exit_code, 1);
        assert!(result.output.is_empty());
    }

    #[test]
    fn test_grep_regex_match() {
        let lines = vec![
            OutputLine::text("apple"),
            OutputLine::text("banana"),
            OutputLine::text("cherry"),
        ];
        let result = apply_filter("grep", &args(&["^b"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_case_sensitive_by_default() {
        let lines = vec![
            OutputLine::text("Apple"),
            OutputLine::text("apple"),
        ];
        let result = apply_filter("grep", &args(&["apple"]), lines);
        // default is case-sensitive now (was case-insensitive previously)
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "apple"));
    }

    #[test]
    fn test_grep_ignore_case_flag() {
        let lines = vec![
            OutputLine::text("Apple"),
            OutputLine::text("apple"),
            OutputLine::text("banana"),
        ];
        let result = apply_filter("grep", &args(&["-i", "apple"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
    }

    #[test]
    fn test_grep_invert_flag() {
        let lines = vec![
            OutputLine::text("apple"),
            OutputLine::text("banana"),
            OutputLine::text("cherry"),
        ];
        let result = apply_filter("grep", &args(&["-v", "apple"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 2);
    }

    #[test]
    fn test_grep_combined_short_flags() {
        let lines = vec![
            OutputLine::text("Apple"),
            OutputLine::text("banana"),
        ];
        let result = apply_filter("grep", &args(&["-iv", "apple"]), lines);
        // -i case-insensitive AND -v invert: "Apple" matches case-insensitive so is excluded;
        // "banana" doesn't match, so is kept
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
        assert!(matches!(&result.output[0].data, OutputLineData::Text(s) if s == "banana"));
    }

    #[test]
    fn test_grep_extended_flag_accepted() {
        // -E is accepted as alias (regex crate always uses extended syntax)
        let lines = vec![OutputLine::text("apple")];
        let result = apply_filter("grep", &args(&["-E", "a.*e"]), lines);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.len(), 1);
    }

    #[test]
    fn test_grep_invalid_regex_exit_2() {
        let lines = vec![OutputLine::text("anything")];
        // unbalanced parens
        let result = apply_filter("grep", &args(&["("]), lines);
        assert_eq!(result.exit_code, 2);
    }

    #[test]
    fn test_grep_unknown_flag_exit_2() {
        let lines = vec![OutputLine::text("anything")];
        let result = apply_filter("grep", &args(&["-x", "pat"]), lines);
        assert_eq!(result.exit_code, 2);
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
