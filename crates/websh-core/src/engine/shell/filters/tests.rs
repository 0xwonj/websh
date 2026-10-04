use super::*;

fn filter(name: &str, args: &[&str], lines: &[&str]) -> CommandResult {
    apply_filter(
        name,
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        lines.iter().map(|s| OutputLine::text(*s)).collect(),
    )
}

fn text(result: &CommandResult) -> Vec<&str> {
    result
        .output
        .iter()
        .map(|line| match &line.data {
            OutputLineData::Text(text) => text.as_str(),
            other => panic!("expected text output, got {other:?}"),
        })
        .collect()
}

#[test]
fn grep_applies_regex_case_and_inversion_options() {
    for (args, expected) in [
        (&["apple"][..], &["apple"][..]),
        (&["^b"][..], &["banana"][..]),
        (&["-i", "apple"][..], &["Apple", "apple"][..]),
        (&["-v", "apple"][..], &["Apple", "banana"][..]),
        (&["-iv", "apple"][..], &["banana"][..]),
        (&["-E", "a.*e"][..], &["apple"][..]),
    ] {
        let result = filter("grep", args, &["Apple", "apple", "banana"]);
        assert_eq!(result.exit_code, 0, "{args:?}");
        assert_eq!(text(&result), expected, "{args:?}");
    }
}

#[test]
fn grep_fixed_strings_treats_regex_characters_literally() {
    for flag in ["-F", "--fixed-strings"] {
        let result = filter("grep", &[flag, "a.b"], &["a.b", "axb"]);
        assert_eq!(result.exit_code, 0, "{flag}");
        assert_eq!(text(&result), ["a.b"], "{flag}");
    }
    let result = filter("grep", &["-iF", "a.b"], &["A.B", "AxB"]);
    assert_eq!(result.exit_code, 0);
    assert_eq!(text(&result), ["A.B"]);
}

#[test]
fn grep_no_match_has_empty_output_and_exit_one() {
    let result = filter("grep", &["apple"], &["APPLE", "banana"]);
    assert_eq!(result.exit_code, 1);
    assert!(result.output.is_empty());
}

#[test]
fn grep_reports_invalid_arguments_with_exit_two() {
    for (args, message) in [
        (&[][..], "missing pattern"),
        (&["("][..], "invalid regex"),
        (&["-x", "pattern"][..], "unknown option"),
        (&["first", "second"][..], "extra argument"),
    ] {
        let result = filter("grep", args, &["anything"]);
        assert_eq!(result.exit_code, 2, "{args:?}");
        assert!(
            matches!(&result.output[..], [OutputLine { data: OutputLineData::Error(error), .. }] if error.contains(message)),
            "{args:?}: {:?}",
            result.output,
        );
    }
}

#[test]
fn grep_matches_directory_entries_without_changing_their_presentation() {
    let matching = OutputLine::dir_entry("project-alpha", "Alpha project");
    let result = apply_filter(
        "grep",
        &["alpha".into()],
        vec![
            matching.clone(),
            OutputLine::dir_entry("project-beta", "Beta project"),
        ],
    );
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.output, vec![matching]);
}

#[test]
fn head_and_tail_select_the_requested_end_of_the_input() {
    let lines = ["apple", "banana", "cherry", "date", "elderberry"];
    for (name, expected) in [("head", &lines[..2]), ("tail", &lines[3..])] {
        for args in [&["-2"][..], &["-n", "2"][..]] {
            let result = filter(name, args, &lines);
            assert_eq!(result.exit_code, 0, "{name} {args:?}");
            assert_eq!(text(&result), expected, "{name} {args:?}");
        }
        let result = filter(name, &[], &lines);
        assert_eq!(result.exit_code, 0, "{name}");
        assert_eq!(
            text(&result),
            lines,
            "{name} keeps inputs shorter than ten lines"
        );
    }
    let longer = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"];
    for (name, expected) in [("head", &longer[..10]), ("tail", &longer[2..])] {
        let result = filter(name, &[], &longer);
        assert_eq!(result.exit_code, 0, "{name}");
        assert_eq!(text(&result), expected, "{name} defaults to ten lines");
    }
}

#[test]
fn head_and_tail_reject_invalid_counts() {
    for name in ["head", "tail"] {
        for args in [&["--2"][..], &["-abc"][..]] {
            assert_eq!(
                filter(name, args, &["line"]).exit_code,
                2,
                "{name} {args:?}"
            );
        }
    }
}

#[test]
fn wc_counts_nonempty_output_entries() {
    let result = apply_filter(
        "wc",
        &[],
        vec![
            OutputLine::text("one"),
            OutputLine::empty(),
            OutputLine::text("two"),
        ],
    );
    assert_eq!(result.exit_code, 0);
    assert_eq!(text(&result), ["2"]);
}

#[test]
fn unknown_filter_reports_exit_127() {
    let result = filter("unknown", &[], &["line"]);
    assert_eq!(result.exit_code, 127);
    assert!(
        matches!(&result.output[..], [OutputLine { data: OutputLineData::Error(error), .. }] if error.contains("unknown filter"))
    );
}
