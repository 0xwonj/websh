//! Lexer for tokenizing shell input.
//!
//! Handles:
//! - Word tokenization
//! - Pipe operator (`|`)
//! - Variable references (`$VAR`, `${VAR}`)
//! - History expansion (`!!`, `!n`, `!-n`)
//! - Quote handling (single and double quotes)

use crate::core::env;

// =============================================================================
// Token Types
// =============================================================================

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

// =============================================================================
// Lexer
// =============================================================================

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

// =============================================================================
// Tests
// =============================================================================

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
    fn test_lexer_iterator_take() {
        let lexer = Lexer::new("a b c d e");
        let first_two: Vec<_> = lexer.take(2).collect();
        assert_eq!(first_two.len(), 2);
        assert_eq!(first_two[0], Token::Word("a".to_string()));
        assert_eq!(first_two[1], Token::Word("b".to_string()));
    }

    #[test]
    fn test_lexer_iterator_filter() {
        let lexer = Lexer::new("ls | grep | head");
        let non_pipes: Vec<_> = lexer.filter(|t| !matches!(t, Token::Pipe)).collect();
        assert_eq!(non_pipes.len(), 3);
    }

    #[test]
    fn test_lexer_iterator_count() {
        let lexer = Lexer::new("echo hello world");
        assert_eq!(lexer.count(), 3);
    }
}
