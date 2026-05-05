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
    let lines = vec![OutputLine::text("Apple"), OutputLine::text("apple")];
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
    let lines = vec![OutputLine::text("Apple"), OutputLine::text("banana")];
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
    let result = apply_filter("head", &args(&["-3"]), lines);
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
    let result = apply_filter("tail", &args(&["-2"]), lines);
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

#[test]
fn test_head_double_dash_rejected() {
    let lines = test_lines();
    let result = apply_filter("head", &args(&["--5"]), lines);
    assert_eq!(result.exit_code, 2);
}

#[test]
fn test_head_n_flag() {
    let lines = test_lines();
    let result = apply_filter("head", &args(&["-n", "3"]), lines);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.output.len(), 3);
}

#[test]
fn test_head_non_numeric_rejected() {
    let lines = test_lines();
    let result = apply_filter("head", &args(&["-abc"]), lines);
    assert_eq!(result.exit_code, 2);
}

#[test]
fn test_tail_double_dash_rejected() {
    let lines = test_lines();
    let result = apply_filter("tail", &args(&["--2"]), lines);
    assert_eq!(result.exit_code, 2);
}

#[test]
fn test_tail_n_flag() {
    let lines = test_lines();
    let result = apply_filter("tail", &args(&["-n", "2"]), lines);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.output.len(), 2);
}

#[test]
fn test_grep_fixed_strings_short_flag() {
    // Without -F, parens are regex metachars
    let lines = vec![OutputLine::text("hello (world)")];
    let result = apply_filter("grep", &args(&["-F", "(world)"]), lines);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.output.len(), 1);
}

#[test]
fn test_grep_fixed_strings_long_flag() {
    let lines = vec![OutputLine::text("a.b.c")];
    let result = apply_filter("grep", &args(&["--fixed-strings", "a.b"]), lines);
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_grep_fixed_strings_combined_with_i() {
    let lines = vec![
        OutputLine::text("HELLO.WORLD"),
        OutputLine::text("no match here"),
    ];
    let result = apply_filter("grep", &args(&["-iF", "hello.world"]), lines);
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.output.len(), 1);
}

#[test]
fn test_grep_extra_positional_error_message() {
    let lines = vec![OutputLine::text("x")];
    let result = apply_filter("grep", &args(&["pat1", "pat2"]), lines);
    assert_eq!(result.exit_code, 2);
    let msg = match &result.output[0].data {
        OutputLineData::Error(s) => s.clone(),
        _ => panic!("expected error"),
    };
    assert!(msg.contains("extra argument"), "msg: {}", msg);
}
