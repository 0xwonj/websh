use std::collections::BTreeMap;

use crate::engine::shell::OutputLine;
use crate::engine::shell::{CommandResult, SideEffect};

/// Execute `export` command against a target-provided environment snapshot.
///
/// Each element of `assignments` is processed independently:
///   - `KEY=value` -> request setting the variable
///   - `KEY` alone -> print `KEY=<value>` if set (silent otherwise)
///
/// An empty list prints all user variables. Invalid assignments emit an error
/// line and set exit_code=1; subsequent assignments are still processed.
pub(super) fn execute_export(
    assignments: Vec<String>,
    env: &BTreeMap<String, String>,
) -> CommandResult {
    if assignments.is_empty() {
        let mut output = vec![OutputLine::empty()];
        for line in format_export_output(env) {
            output.push(OutputLine::text(line));
        }
        output.push(OutputLine::empty());
        return CommandResult::output(output);
    }

    let mut output: Vec<OutputLine> = Vec::new();
    let mut side_effects = Vec::new();
    let mut exit_code = 0;

    for arg in assignments {
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if is_valid_var_name(key) {
                side_effects.push(SideEffect::SetEnvVar {
                    key: key.to_string(),
                    value: value.to_string(),
                });
            } else {
                output.push(OutputLine::error(
                    "export: invalid variable name (use letters, numbers, underscores)",
                ));
                if exit_code == 0 {
                    exit_code = 1;
                }
            }
        } else {
            let key = arg.trim();
            if !is_valid_var_name(key) {
                output.push(OutputLine::error(
                    "export: invalid variable name (use letters, numbers, underscores)",
                ));
                if exit_code == 0 {
                    exit_code = 1;
                }
                continue;
            }
            if let Some(value) = env.get(key) {
                output.push(OutputLine::text(format!("{}={}", key, value)));
            }
        }
    }

    CommandResult {
        output,
        exit_code,
        side_effects,
    }
}

/// Execute `unset` command.
pub(super) fn execute_unset(key: String, env: &BTreeMap<String, String>) -> CommandResult {
    let key = key.trim();
    if !is_valid_var_name(key) {
        return CommandResult::error_line(
            "unset: invalid variable name (use letters, numbers, underscores)",
        );
    }

    if env.contains_key(key) {
        CommandResult::empty().with_side_effect(SideEffect::UnsetEnvVar {
            key: key.to_string(),
        })
    } else {
        CommandResult::empty()
    }
}

pub(super) fn is_valid_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    let mut chars = name.chars();
    let first = chars.next().unwrap();

    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn format_export_output(env: &BTreeMap<String, String>) -> Vec<String> {
    let mut lines = Vec::new();

    for (key, value) in env {
        lines.push(format!("declare -x {}=\"{}\"", key, value));
    }

    if lines.is_empty() {
        lines.push("# No user variables set".to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::shell::OutputLineData;

    fn env() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("EDITOR".to_string(), "vim".to_string()),
            ("LANG".to_string(), "en".to_string()),
        ])
    }

    fn line_text(line: &OutputLine) -> &str {
        match &line.data {
            OutputLineData::Text(text)
            | OutputLineData::Error(text)
            | OutputLineData::Success(text)
            | OutputLineData::Info(text)
            | OutputLineData::Ascii(text) => text,
            OutputLineData::Command { .. }
            | OutputLineData::Empty
            | OutputLineData::ListEntry { .. } => "",
        }
    }

    #[test]
    fn valid_var_names() {
        assert!(is_valid_var_name("FOO"));
        assert!(is_valid_var_name("foo"));
        assert!(is_valid_var_name("_foo"));
        assert!(is_valid_var_name("FOO_BAR"));
        assert!(is_valid_var_name("foo123"));
        assert!(is_valid_var_name("_123"));
        assert!(is_valid_var_name("a"));
        assert!(is_valid_var_name("_"));
    }

    #[test]
    fn invalid_var_names() {
        assert!(!is_valid_var_name(""));
        assert!(!is_valid_var_name("123"));
        assert!(!is_valid_var_name("1foo"));
        assert!(!is_valid_var_name("foo-bar"));
        assert!(!is_valid_var_name("foo.bar"));
        assert!(!is_valid_var_name("foo bar"));
        assert!(!is_valid_var_name("foo=bar"));
    }

    #[test]
    fn export_lists_snapshot_vars() {
        let result = execute_export(Vec::new(), &env());
        let text = result.output.iter().map(line_text).collect::<Vec<_>>();
        assert!(text.contains(&"declare -x EDITOR=\"vim\""));
        assert!(text.contains(&"declare -x LANG=\"en\""));
        assert!(result.side_effects.is_empty());
    }

    #[test]
    fn export_bare_key_reads_snapshot() {
        let result = execute_export(vec!["EDITOR".to_string()], &env());
        assert_eq!(line_text(&result.output[0]), "EDITOR=vim");
        assert!(result.side_effects.is_empty());
    }

    #[test]
    fn export_assignment_requests_set_env_side_effect() {
        let result = execute_export(vec!["EDITOR=nano".to_string()], &env());
        assert_eq!(
            result.side_effects,
            vec![SideEffect::SetEnvVar {
                key: "EDITOR".to_string(),
                value: "nano".to_string()
            }]
        );
    }

    #[test]
    fn export_multiple_assignments_preserve_order() {
        let result = execute_export(
            vec!["EDITOR=nano".to_string(), "PAGER=less".to_string()],
            &env(),
        );
        assert_eq!(
            result.side_effects,
            vec![
                SideEffect::SetEnvVar {
                    key: "EDITOR".to_string(),
                    value: "nano".to_string()
                },
                SideEffect::SetEnvVar {
                    key: "PAGER".to_string(),
                    value: "less".to_string()
                },
            ]
        );
    }

    #[test]
    fn export_invalid_name_errors_without_side_effect() {
        let result = execute_export(vec!["1BAD=value".to_string()], &env());
        assert_eq!(result.exit_code, 1);
        assert!(line_text(&result.output[0]).contains("invalid variable name"));
        assert!(result.side_effects.is_empty());
    }

    #[test]
    fn unset_existing_var_requests_unset_side_effect() {
        let result = execute_unset("EDITOR".to_string(), &env());
        assert_eq!(
            result.side_effects,
            vec![SideEffect::UnsetEnvVar {
                key: "EDITOR".to_string()
            }]
        );
    }

    #[test]
    fn unset_missing_var_is_noop() {
        let result = execute_unset("PAGER".to_string(), &env());
        assert!(result.side_effects.is_empty());
    }
}
