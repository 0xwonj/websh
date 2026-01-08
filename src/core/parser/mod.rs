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

use expand::expand_tokens;
use std::fmt;

// =============================================================================
// Parse Error
// =============================================================================

/// Structured error type for parsing failures
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    /// Pipe at the beginning of input: `| grep foo`
    UnexpectedPipe { position: usize },
    /// Empty stage between pipes: `ls | | grep`
    EmptyPipeStage { position: usize },
    /// Pipe at the end with no following command: `ls |`
    TrailingPipe { position: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedPipe { position } => {
                write!(
                    f,
                    "syntax error near token {}: unexpected '|'",
                    position + 1
                )
            }
            Self::EmptyPipeStage { position } => {
                write!(
                    f,
                    "syntax error near token {}: empty pipe stage",
                    position + 1
                )
            }
            Self::TrailingPipe { position } => {
                write!(
                    f,
                    "syntax error near token {}: unexpected end after '|'",
                    position + 1
                )
            }
        }
    }
}

impl std::error::Error for ParseError {}

// =============================================================================
// Pipeline Representation
// =============================================================================

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
    pub error: Option<ParseError>,
}

impl Pipeline {
    /// Check if the pipeline is empty
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Get the first command name (if any)
    #[allow(dead_code)]
    pub fn first_command_name(&self) -> Option<&str> {
        self.commands.first().map(|c| c.name.as_str())
    }

    /// Check if pipeline has a syntax error.
    #[cfg(test)]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

// =============================================================================
// Parser
// =============================================================================

/// Parse input with variable and history expansion, then build pipeline
pub fn parse_input(input: &str, history: &[String]) -> Pipeline {
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();

    // Expand variables and history
    let expanded = expand_tokens(tokens, history);

    // Split into pipeline stages
    parse_pipeline(expanded)
}

fn parse_pipeline(tokens: Vec<Token>) -> Pipeline {
    let mut commands = Vec::new();
    let mut current_words = Vec::new();
    let mut error: Option<ParseError> = None;
    let mut expect_command = false; // true after seeing a pipe
    let mut last_pipe_pos = 0;

    for (idx, token) in tokens.into_iter().enumerate() {
        match token {
            Token::Word(w) if !w.is_empty() => {
                current_words.push(w);
                expect_command = false;
            }
            Token::Pipe => {
                if current_words.is_empty() {
                    // Empty stage before pipe (e.g., "| grep" or "ls | | grep")
                    if commands.is_empty() {
                        error = Some(ParseError::UnexpectedPipe { position: idx });
                    } else {
                        error = Some(ParseError::EmptyPipeStage { position: idx });
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
        error = Some(ParseError::TrailingPipe {
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pipeline() {
        let pipeline = parse_input("ls | grep blog | head -5", &[]);
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
    fn test_empty_pipe_leading() {
        let pipeline = parse_input("| grep foo", &[]);
        assert!(pipeline.has_error());
        assert_eq!(
            pipeline.error,
            Some(ParseError::UnexpectedPipe { position: 0 })
        );
    }

    #[test]
    fn test_empty_pipe_middle() {
        let pipeline = parse_input("ls | | grep foo", &[]);
        assert!(pipeline.has_error());
        // tokens: ["ls", "|", "|", "grep", "foo"], second pipe at index 2
        assert_eq!(
            pipeline.error,
            Some(ParseError::EmptyPipeStage { position: 2 })
        );
    }

    #[test]
    fn test_empty_pipe_trailing() {
        let pipeline = parse_input("ls |", &[]);
        assert!(pipeline.has_error());
        // tokens: ["ls", "|"], pipe at index 1
        assert_eq!(
            pipeline.error,
            Some(ParseError::TrailingPipe { position: 1 })
        );
    }

    #[test]
    fn test_valid_pipeline_no_error() {
        let pipeline = parse_input("ls | grep foo | head -5", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands.len(), 3);
    }
}
