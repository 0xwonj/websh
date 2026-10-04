//! Advanced command parser with variable expansion, history, and pipes.
//!
//! Supports:
//! - Variable expansion: `$VAR`, `${VAR}`
//! - History expansion: `!!` (last command), `!n` (nth command), `!-n` (nth from last)
//! - Pipe operator: `cmd1 | cmd2`
//! - Quote handling: `"string with spaces"`, `'literal string'`

mod expand;
mod lexer;

pub use lexer::{Lexer, Token};

use std::collections::BTreeMap;

use expand::expand_tokens;
use thiserror::Error;

/// Structured error type for shell pipeline parsing failures.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ShellParseError {
    /// Pipe at the beginning of input: `| grep foo`
    #[error("syntax error near token {}: unexpected '|'", position + 1)]
    UnexpectedPipe { position: usize },
    /// Empty stage between pipes: `ls | | grep`
    #[error("syntax error near token {}: empty pipe stage", position + 1)]
    EmptyPipeStage { position: usize },
    /// Pipe at the end with no following command: `ls |`
    #[error("syntax error near token {}: unexpected end after '|'", position + 1)]
    TrailingPipe { position: usize },
    /// Unclosed single or double quote starting at `position`.
    #[error(
        "syntax error: unclosed {} quote starting at position {position}",
        if *kind == '"' { "double" } else { "single" }
    )]
    UnclosedQuote { kind: char, position: usize },
}

/// A single command in a pipeline
#[derive(Debug, Clone)]
pub struct ParsedCommand {
    pub name: String,
    pub args: Vec<String>,
}

/// A pipeline of commands connected by pipes
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub commands: Vec<ParsedCommand>,
    /// Syntax error (e.g., empty pipe stage)
    pub error: Option<ShellParseError>,
}

impl Pipeline {
    /// A safely quoted command line containing the arguments actually executed.
    /// History stores this form so replay never expands variables or history twice.
    pub fn command_line(&self) -> String {
        self.commands
            .iter()
            .map(|command| {
                std::iter::once(&command.name)
                    .chain(&command.args)
                    .map(|word| quote_word(word))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// Whether this input consists only of current commands and pipe filters.
    /// Call before echoing or retaining input supplied by the user.
    pub fn is_supported(&self) -> bool {
        self.error.is_none()
            && self.commands.iter().enumerate().all(|(index, command)| {
                if index == 0 {
                    !matches!(
                        super::Command::parse(&command.name, &command.args),
                        super::Command::Unknown(_)
                    )
                } else {
                    matches!(
                        command.name.to_ascii_lowercase().as_str(),
                        "grep" | "head" | "tail" | "wc"
                    )
                }
            })
    }

    /// Check if the pipeline is empty
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Check if pipeline has a syntax error.
    #[cfg(test)]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

fn quote_word(word: &str) -> String {
    if !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c))
    {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\"'\"'"))
    }
}

/// Parse input with variable and history expansion, then build pipeline
pub fn parse_input(input: &str, history: &[String]) -> Pipeline {
    parse_input_with_env(input, history, &BTreeMap::new())
}

/// Parse input using a target-provided environment snapshot for variable
/// expansion.
pub fn parse_input_with_env(
    input: &str,
    history: &[String],
    env: &BTreeMap<String, String>,
) -> Pipeline {
    let mut lexer = Lexer::new_with_env(input, env);
    let tokens: Vec<Token> = (&mut lexer).collect();

    if let Some(err) = lexer.error().cloned() {
        return Pipeline {
            commands: vec![],
            error: Some(err),
        };
    }

    // Expand variables and history
    let expanded = expand_tokens(tokens, history);

    // Split into pipeline stages
    parse_pipeline(expanded)
}

fn parse_pipeline(tokens: Vec<Token>) -> Pipeline {
    let mut commands = Vec::new();
    let mut current_words = Vec::new();
    let mut error: Option<ShellParseError> = None;
    let mut expect_command = false; // true after seeing a pipe
    let mut last_pipe_pos = 0;

    for (idx, token) in tokens.into_iter().enumerate() {
        match token {
            Token::Word(w) => {
                // Preserve empty words: the lexer already drops words that
                // should disappear (unquoted `$UNDEF`). Remaining empties
                // come from explicit quoting like `""` or `"$UNDEF"` and
                // must stay as argv slots.
                current_words.push(w);
                expect_command = false;
            }
            Token::Pipe => {
                if current_words.is_empty() {
                    // Empty stage before pipe (e.g., "| grep" or "ls | | grep")
                    if commands.is_empty() {
                        error = Some(ShellParseError::UnexpectedPipe { position: idx });
                    } else {
                        error = Some(ShellParseError::EmptyPipeStage { position: idx });
                    }
                    break;
                }
                commands.push(words_to_command(&current_words));
                current_words.clear();
                expect_command = true;
                last_pipe_pos = idx;
            }
            _ => {}
        }
    }

    // Check for trailing pipe (e.g., "ls |")
    if error.is_none() && expect_command && current_words.is_empty() {
        error = Some(ShellParseError::TrailingPipe {
            position: last_pipe_pos,
        });
    }

    if !current_words.is_empty() {
        commands.push(words_to_command(&current_words));
    }

    Pipeline { commands, error }
}

fn words_to_command(words: &[String]) -> ParsedCommand {
    ParsedCommand {
        name: words.first().cloned().unwrap_or_default(),
        args: words.iter().skip(1).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pipeline() {
        let pipeline = parse_input("ls | grep blog | head -5", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands.len(), 3);
        assert_eq!(pipeline.commands[0].name, "ls");
        assert_eq!(pipeline.commands[1].name, "grep");
        assert_eq!(pipeline.commands[1].args, vec!["blog"]);
        assert_eq!(pipeline.commands[2].name, "head");
        assert_eq!(pipeline.commands[2].args, vec!["-5"]);
    }

    #[test]
    fn test_history_expansion() {
        let history = vec!["ls -la".to_string(), "pwd".to_string()];
        let pipeline = parse_input("!!", &history);
        assert_eq!(pipeline.commands.len(), 1);
        assert_eq!(pipeline.commands[0].name, "pwd");
    }

    #[test]
    fn test_history_index_expansion() {
        let history = vec!["ls -la".to_string(), "pwd".to_string()];
        let pipeline = parse_input("!0", &history);
        assert_eq!(pipeline.commands.len(), 1);
        assert_eq!(pipeline.commands[0].name, "ls");
        assert_eq!(pipeline.commands[0].args, vec!["-la"]);
    }

    #[test]
    fn empty_pipeline_stages_report_their_position() {
        for (input, expected) in [
            (
                "| grep foo",
                ShellParseError::UnexpectedPipe { position: 0 },
            ),
            (
                "ls | | grep foo",
                ShellParseError::EmptyPipeStage { position: 2 },
            ),
            ("ls |", ShellParseError::TrailingPipe { position: 1 }),
        ] {
            assert_eq!(parse_input(input, &[]).error, Some(expected), "{input}");
        }
    }

    #[test]
    fn unclosed_quotes_are_parse_errors() {
        for (input, expected) in [("echo 'hello", '\''), ("echo \"world", '"')] {
            assert!(
                matches!(parse_input(input, &[]).error, Some(ShellParseError::UnclosedQuote { kind, .. }) if kind == expected),
                "{input}"
            );
        }
    }

    #[test]
    fn supported_input_uses_the_expanded_command_and_filter_grammar() {
        for input in [
            "",
            "'ls' | GREP foo | head -2",
            "echo 'unknown command'",
            "echo a > b",
            "REFRESH /mempool",
        ] {
            assert!(parse_input(input, &[]).is_supported(), "{input}");
        }
        for input in [
            "unknown payload",
            "echo hello | unknown payload",
            "ls | echo hi",
            "ls |",
            "echo 'unclosed",
        ] {
            assert!(!parse_input(input, &[]).is_supported(), "{input}");
        }
        let env = BTreeMap::from([("CMD".into(), "unknown".into())]);
        assert!(!parse_input_with_env("$CMD payload", &[], &env).is_supported());
        assert!(!parse_input("!!", &["unknown payload".into()]).is_supported());
    }

    #[test]
    fn history_repeats_executed_commands_without_nested_expansion() {
        let env = BTreeMap::from([("MESSAGE".into(), "literal $OTHER | !!".into())]);
        let mut history = vec![parse_input_with_env("echo \"$MESSAGE\"", &[], &env).command_line()];
        for input in ["!!", "!!", "!-1", "!0"] {
            let replay = parse_input_with_env(input, &history, &BTreeMap::new());
            assert!(replay.is_supported());
            assert_eq!(replay.commands.len(), 1);
            assert_eq!(replay.commands[0].name, "echo");
            assert_eq!(replay.commands[0].args, ["literal $OTHER | !!"]);
            history.push(replay.command_line());
        }
    }

    #[test]
    fn command_line_preserves_quoted_arguments_and_pipeline_boundaries() {
        let pipeline = Pipeline {
            commands: vec![
                ParsedCommand {
                    name: "echo".into(),
                    args: vec![
                        String::new(),
                        "don't $expand !0 | split\\this\n한국어".into(),
                    ],
                },
                ParsedCommand {
                    name: "grep".into(),
                    args: vec!["$expand".into()],
                },
            ],
            error: None,
        };
        let replay = parse_input(&pipeline.command_line(), &[]);
        assert_eq!(replay.commands.len(), pipeline.commands.len());
        for (actual, expected) in replay.commands.iter().zip(&pipeline.commands) {
            assert_eq!(actual.name, expected.name);
            assert_eq!(actual.args, expected.args);
        }
        assert_eq!(parse_input("ls -l docs", &[]).command_line(), "ls -l docs");
    }
}
