//! Advanced command parser with variable expansion, history, and pipes.
//!
//! Supports:
//! - Variable expansion: `$VAR`, `${VAR}`
//! - History expansion: `!!` (last command), `!n` (nth command), `!-n` (nth from last)
//! - Pipe operator: `cmd1 | cmd2`
//! - Quote handling: `"string with spaces"`, `'literal string'`

use crate::core::env;
use std::fmt;

// ============================================================================
// Parse Error
// ============================================================================

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
                write!(f, "syntax error near token {}: unexpected '|'", position + 1)
            }
            Self::EmptyPipeStage { position } => {
                write!(f, "syntax error near token {}: empty pipe stage", position + 1)
            }
            Self::TrailingPipe { position } => {
                write!(f, "syntax error near token {}: unexpected end after '|'", position + 1)
            }
        }
    }
}

impl std::error::Error for ParseError {}

// ============================================================================
// Token Types
// ============================================================================

/// Token types produced by the lexer
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A word (command name or argument)
    Word(String),
    /// Pipe operator `|`
    Pipe,
    /// Variable reference `$VAR` or `${VAR}`
    Variable(String),
    /// Last command `!!`
    HistoryLast,
    /// History by index `!n` or `!-n`
    HistoryIndex(i32),
}

/// Result of reading a variable name after `$`
enum VariableRead {
    /// Successfully read variable name
    Name(String),
    /// Empty variable (just `$` or `${}`)
    Empty,
    /// Unclosed brace `${...` without closing `}`
    UnclosedBrace(String),
}

// ============================================================================
// Lexer
// ============================================================================

/// Lexer for tokenizing shell input
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    pub fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    /// Tokenize the entire input into a vector
    ///
    /// This is a convenience method that collects all tokens.
    /// For lazy evaluation, use the `Iterator` implementation directly.
    pub fn tokenize(self) -> Vec<Token> {
        self.collect()
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            let c = self.current_char();
            if !c.is_whitespace() {
                break;
            }
            self.pos += c.len_utf8();
        }
    }

    fn current_char(&self) -> char {
        self.input[self.pos..].chars().next().unwrap_or('\0')
    }

    fn next_token(&mut self) -> Option<Token> {
        let c = self.current_char();

        match c {
            '|' => {
                self.pos += 1;
                Some(Token::Pipe)
            }
            '$' => self.parse_variable(),
            '!' => self.parse_history(),
            '"' => self.parse_double_quoted(),
            '\'' => self.parse_single_quoted(),
            _ => self.parse_word(),
        }
    }

    fn parse_variable(&mut self) -> Option<Token> {
        self.pos += 1; // skip $

        if self.pos >= self.input.len() {
            return Some(Token::Word("$".to_string()));
        }

        match self.read_variable_name() {
            VariableRead::Name(name) => Some(Token::Variable(name)),
            VariableRead::Empty => Some(Token::Word("$".to_string())),
            VariableRead::UnclosedBrace(partial) => Some(Token::Word(format!("${{{}", partial))),
        }
    }

    /// Read a variable name after the `$` has been consumed.
    /// Handles both `$VAR` and `${VAR}` syntax.
    fn read_variable_name(&mut self) -> VariableRead {
        // Handle ${VAR} syntax
        if self.current_char() == '{' {
            self.pos += 1;
            let start = self.pos;
            while self.pos < self.input.len() {
                let c = self.current_char();
                if c == '}' {
                    let name = self.input[start..self.pos].to_string();
                    self.pos += 1;
                    return if name.is_empty() {
                        VariableRead::Empty
                    } else {
                        VariableRead::Name(name)
                    };
                }
                self.pos += c.len_utf8();
            }
            // Unclosed brace
            return VariableRead::UnclosedBrace(self.input[start..].to_string());
        }

        // Handle $VAR syntax
        let start = self.pos;
        while self.pos < self.input.len() {
            let c = self.current_char();
            if !c.is_alphanumeric() && c != '_' {
                break;
            }
            self.pos += c.len_utf8();
        }

        let name = self.input[start..self.pos].to_string();
        if name.is_empty() {
            VariableRead::Empty
        } else {
            VariableRead::Name(name)
        }
    }

    fn parse_history(&mut self) -> Option<Token> {
        self.pos += 1; // skip first !

        if self.pos >= self.input.len() {
            return Some(Token::Word("!".to_string()));
        }

        // Check for !! (last command)
        if self.current_char() == '!' {
            self.pos += 1;
            return Some(Token::HistoryLast);
        }

        // Check for !n or !-n
        let start = self.pos;
        if self.current_char() == '-' {
            self.pos += 1;
        }

        while self.pos < self.input.len() {
            let c = self.current_char();
            if !c.is_ascii_digit() {
                break;
            }
            self.pos += 1;
        }

        let num_str = &self.input[start..self.pos];
        if let Ok(n) = num_str.parse::<i32>() {
            Some(Token::HistoryIndex(n))
        } else {
            // Not a valid history reference, treat as word starting with !
            self.pos = start;
            self.parse_word_with_prefix("!")
        }
    }

    fn parse_double_quoted(&mut self) -> Option<Token> {
        self.pos += 1; // skip opening "
        let mut result = String::new();

        while self.pos < self.input.len() {
            let c = self.current_char();
            self.pos += c.len_utf8();

            if c == '"' {
                break;
            } else if c == '\\' && self.pos < self.input.len() {
                // Handle escape sequences
                let escaped = self.current_char();
                self.pos += escaped.len_utf8();
                match escaped {
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    _ => result.push(escaped),
                }
            } else if c == '$' && self.pos < self.input.len() {
                // Variable expansion inside double quotes
                let used_braces = self.current_char() == '{';
                match self.read_variable_name() {
                    VariableRead::Name(name) => {
                        if let Some(value) = env::get_user_var(&name) {
                            result.push_str(&value);
                        } else {
                            // Keep original if variable not found
                            result.push('$');
                            if used_braces {
                                result.push_str(&format!("{{{}}}", name));
                            } else {
                                result.push_str(&name);
                            }
                        }
                    }
                    VariableRead::Empty => result.push('$'),
                    VariableRead::UnclosedBrace(partial) => {
                        result.push_str(&format!("${{{}", partial));
                    }
                }
            } else {
                result.push(c);
            }
        }

        Some(Token::Word(result))
    }

    fn parse_single_quoted(&mut self) -> Option<Token> {
        self.pos += 1; // skip opening '
        let start = self.pos;

        while self.pos < self.input.len() {
            let c = self.current_char();
            if c == '\'' {
                let content = self.input[start..self.pos].to_string();
                self.pos += 1;
                return Some(Token::Word(content));
            }
            self.pos += c.len_utf8();
        }

        // Unclosed quote, return what we have
        Some(Token::Word(self.input[start..].to_string()))
    }

    fn parse_word(&mut self) -> Option<Token> {
        self.parse_word_with_prefix("")
    }

    fn parse_word_with_prefix(&mut self, prefix: &str) -> Option<Token> {
        let start = self.pos;

        while self.pos < self.input.len() {
            let c = self.current_char();
            if c.is_whitespace() || c == '|' || c == '$' || c == '!' || c == '"' || c == '\'' {
                break;
            }
            self.pos += c.len_utf8();
        }

        let word = format!("{}{}", prefix, &self.input[start..self.pos]);
        if word.is_empty() {
            None
        } else {
            Some(Token::Word(word))
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.skip_whitespace();
        if self.pos >= self.input.len() {
            return None;
        }
        self.next_token()
    }
}

// ============================================================================
// Pipeline Representation
// ============================================================================

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
    pub fn first_command_name(&self) -> Option<&str> {
        self.commands.first().map(|c| c.name.as_str())
    }

    /// Check if pipeline has a syntax error.
    #[cfg(test)]
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }
}

// ============================================================================
// Parser
// ============================================================================

/// Parse input with variable and history expansion, then build pipeline
pub fn parse_input(input: &str, history: &[String]) -> Pipeline {
    let lexer = Lexer::new(input);
    let tokens = lexer.tokenize();

    // Expand variables and history
    let expanded = expand_tokens(tokens, history);

    // Split into pipeline stages
    parse_pipeline(expanded)
}

fn expand_tokens(tokens: Vec<Token>, history: &[String]) -> Vec<Token> {
    tokens
        .into_iter()
        .flat_map(|token| match token {
            Token::Variable(name) => {
                let value = env::get_user_var(&name).unwrap_or_default();
                vec![Token::Word(value)]
            }
            Token::HistoryLast => {
                let cmd = history.last().cloned().unwrap_or_default();
                // Re-tokenize the history command (without further history expansion)
                Lexer::new(&cmd)
                    .filter(|t| !matches!(t, Token::HistoryLast | Token::HistoryIndex(_)))
                    .collect()
            }
            Token::HistoryIndex(n) => {
                let cmd = if n >= 0 {
                    history.get(n as usize).cloned().unwrap_or_default()
                } else {
                    // Safe handling of negative index to prevent overflow
                    let idx = history.len().checked_add_signed(n as isize);
                    idx.and_then(|i| history.get(i).cloned())
                        .unwrap_or_default()
                };
                Lexer::new(&cmd)
                    .filter(|t| !matches!(t, Token::HistoryLast | Token::HistoryIndex(_)))
                    .collect()
            }
            other => vec![other],
        })
        .collect()
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
        error = Some(ParseError::TrailingPipe { position: last_pipe_pos });
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let lexer = Lexer::new("ls");
        let tokens = lexer.tokenize();
        assert_eq!(tokens, vec![Token::Word("ls".to_string())]);
    }

    #[test]
    fn test_command_with_args() {
        let lexer = Lexer::new("ls -la /home");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".to_string()),
                Token::Word("-la".to_string()),
                Token::Word("/home".to_string()),
            ]
        );
    }

    #[test]
    fn test_pipe() {
        let lexer = Lexer::new("ls | grep foo");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Word("ls".to_string()),
                Token::Pipe,
                Token::Word("grep".to_string()),
                Token::Word("foo".to_string()),
            ]
        );
    }

    #[test]
    fn test_variable() {
        let lexer = Lexer::new("echo $HOME");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Variable("HOME".to_string()),
            ]
        );
    }

    #[test]
    fn test_variable_braces() {
        let lexer = Lexer::new("echo ${HOME}");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Variable("HOME".to_string()),
            ]
        );
    }

    #[test]
    fn test_history_last() {
        let lexer = Lexer::new("!!");
        let tokens = lexer.tokenize();
        assert_eq!(tokens, vec![Token::HistoryLast]);
    }

    #[test]
    fn test_history_index() {
        let lexer = Lexer::new("!5");
        let tokens = lexer.tokenize();
        assert_eq!(tokens, vec![Token::HistoryIndex(5)]);
    }

    #[test]
    fn test_history_negative_index() {
        let lexer = Lexer::new("!-2");
        let tokens = lexer.tokenize();
        assert_eq!(tokens, vec![Token::HistoryIndex(-2)]);
    }

    #[test]
    fn test_single_quotes() {
        let lexer = Lexer::new("echo 'hello world'");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Word("hello world".to_string()),
            ]
        );
    }

    #[test]
    fn test_double_quotes() {
        let lexer = Lexer::new("echo \"hello world\"");
        let tokens = lexer.tokenize();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Word("hello world".to_string()),
            ]
        );
    }

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
        assert_eq!(pipeline.error, Some(ParseError::UnexpectedPipe { position: 0 }));
    }

    #[test]
    fn test_empty_pipe_middle() {
        let pipeline = parse_input("ls | | grep foo", &[]);
        assert!(pipeline.has_error());
        // tokens: ["ls", "|", "|", "grep", "foo"], second pipe at index 2
        assert_eq!(pipeline.error, Some(ParseError::EmptyPipeStage { position: 2 }));
    }

    #[test]
    fn test_empty_pipe_trailing() {
        let pipeline = parse_input("ls |", &[]);
        assert!(pipeline.has_error());
        // tokens: ["ls", "|"], pipe at index 1
        assert_eq!(pipeline.error, Some(ParseError::TrailingPipe { position: 1 }));
    }

    #[test]
    fn test_valid_pipeline_no_error() {
        let pipeline = parse_input("ls | grep foo | head -5", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands.len(), 3);
    }

    #[test]
    fn test_lexer_iterator_take() {
        // Demonstrate iterator's lazy evaluation with take()
        let lexer = Lexer::new("a b c d e");
        let first_two: Vec<_> = lexer.take(2).collect();
        assert_eq!(first_two.len(), 2);
        assert_eq!(first_two[0], Token::Word("a".to_string()));
        assert_eq!(first_two[1], Token::Word("b".to_string()));
    }

    #[test]
    fn test_lexer_iterator_filter() {
        // Demonstrate iterator's filter capability
        let lexer = Lexer::new("ls | grep | head");
        let non_pipes: Vec<_> = lexer.filter(|t| !matches!(t, Token::Pipe)).collect();
        assert_eq!(non_pipes.len(), 3);
    }

    #[test]
    fn test_lexer_iterator_count() {
        // Demonstrate iterator's count method
        let lexer = Lexer::new("echo hello world");
        assert_eq!(lexer.count(), 3);
    }
}
