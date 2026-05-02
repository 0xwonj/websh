//! Lexer for tokenizing shell input.
//!
//! Handles:
//! - Word tokenization
//! - Pipe operator (`|`)
//! - Variable references (`$VAR`, `${VAR}`)
//! - History expansion (`!!`, `!n`, `!-n`)
//! - Quote handling (single and double quotes)

use crate::core::env;

/// Token types produced by the lexer.
///
/// Variable expansion is performed inline by the lexer while building
/// `Word` tokens, so no dedicated `Variable` variant is emitted.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A word (command name or argument)
    Word(String),
    /// Pipe operator `|`
    Pipe,
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

/// Lexer for tokenizing shell input
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    error: Option<super::ParseError>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            error: None,
        }
    }

    /// Tokenize the entire input into a vector.
    ///
    /// This is a convenience method that collects all tokens. It consumes
    /// the lexer, so callers that need to inspect `error()` after iteration
    /// must use `(&mut lexer).collect()` instead.
    /// For lazy evaluation, use the `Iterator` implementation directly.
    #[allow(dead_code)]
    pub fn tokenize(self) -> Vec<Token> {
        self.collect()
    }

    /// Returns a parse error if one was encountered during tokenization
    /// (e.g., an unclosed quote). Callers that need to surface lexer errors
    /// should iterate via `(&mut lexer).collect()` and then check `.error()`.
    pub fn error(&self) -> Option<&super::ParseError> {
        self.error.as_ref()
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
            '!' => self.parse_history(),
            _ => self.parse_word_segment(),
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
        let hist_start = self.pos;
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
            // Not a valid history reference: treat the `!` as a literal prefix
            // and continue word-segment accumulation from where we are.
            // Rewind past `!` and keep `self.pos` pointed at the char after `!`.
            self.pos = hist_start + 1;
            let mut word = String::from("!");
            if let Some(Token::Word(rest)) = self.parse_word_segment() {
                word.push_str(&rest);
            }
            Some(Token::Word(word))
        }
    }

    /// Parse a single word composed of adjacent segments.
    ///
    /// A word accumulates until whitespace, `|`, or `!` (which may start
    /// history expansion). Segments include plain literals,
    /// `$VAR`/`${VAR}` expansions, and `"..."`/`'...'` quoted strings.
    ///
    /// If the word is composed *entirely* of empty unquoted-variable
    /// expansions (e.g., `$UNDEF` alone), it is dropped from the output
    /// (POSIX: an unquoted empty expansion is removed). If any quoted
    /// segment (even empty), any literal char, or any non-empty variable
    /// expansion appears, the word is emitted (possibly empty).
    fn parse_word_segment(&mut self) -> Option<Token> {
        let mut acc = String::new();
        let mut had_quoted = false;
        let mut had_literal = false;
        let mut any_var_nonempty = false;

        while self.pos < self.input.len() {
            let c = self.current_char();
            if c.is_whitespace() || c == '|' || c == '!' {
                break;
            }

            match c {
                '\'' => {
                    let quote_start = self.pos;
                    self.pos += 1; // skip opening '
                    let start = self.pos;
                    let mut closed = false;
                    while self.pos < self.input.len() {
                        let cc = self.current_char();
                        if cc == '\'' {
                            acc.push_str(&self.input[start..self.pos]);
                            self.pos += 1;
                            closed = true;
                            break;
                        }
                        self.pos += cc.len_utf8();
                    }
                    if !closed {
                        self.error = Some(super::ParseError::UnclosedQuote {
                            kind: '\'',
                            position: quote_start,
                        });
                        return None;
                    }
                    had_quoted = true;
                }
                '"' => {
                    let quote_start = self.pos;
                    self.pos += 1; // skip opening "
                    let mut closed = false;
                    while self.pos < self.input.len() {
                        let cc = self.current_char();
                        self.pos += cc.len_utf8();

                        if cc == '"' {
                            closed = true;
                            break;
                        } else if cc == '\\' && self.pos < self.input.len() {
                            let escaped = self.current_char();
                            self.pos += escaped.len_utf8();
                            match escaped {
                                'n' => acc.push('\n'),
                                't' => acc.push('\t'),
                                _ => acc.push(escaped),
                            }
                        } else if cc == '$' && self.pos < self.input.len() {
                            // Variable expansion inside double quotes.
                            // Quoted context: undefined/empty expansion
                            // contributes nothing; the word is still emitted
                            // because `had_quoted` is true (preserving
                            // POSIX semantics of `"$UNDEF"` → empty arg).
                            match self.read_variable_name() {
                                VariableRead::Name(name) => {
                                    if let Some(value) = env::get_user_var(&name) {
                                        acc.push_str(&value);
                                    }
                                    // undefined var → empty, no contribution
                                }
                                VariableRead::Empty => acc.push('$'),
                                VariableRead::UnclosedBrace(partial) => {
                                    acc.push_str(&format!("${{{}", partial));
                                }
                            }
                        } else {
                            acc.push(cc);
                        }
                    }
                    if !closed {
                        self.error = Some(super::ParseError::UnclosedQuote {
                            kind: '"',
                            position: quote_start,
                        });
                        return None;
                    }
                    had_quoted = true;
                }
                '$' => {
                    self.pos += 1; // skip $
                    if self.pos >= self.input.len() {
                        // bare `$` at EOF → literal $
                        acc.push('$');
                        had_literal = true;
                        break;
                    }
                    match self.read_variable_name() {
                        VariableRead::Name(name) => {
                            if let Some(v) = env::get_user_var(&name) {
                                if v.is_empty() {
                                    // empty value: contributes nothing, doesn't
                                    // count as content — preserves empty-drop
                                    // semantics when there's no other content.
                                } else {
                                    acc.push_str(&v);
                                    any_var_nonempty = true;
                                }
                            }
                            // else: undefined var, same treatment as empty.
                        }
                        VariableRead::Empty => {
                            // `$` followed by non-name char → literal $
                            acc.push('$');
                            had_literal = true;
                        }
                        VariableRead::UnclosedBrace(partial) => {
                            acc.push_str(&format!("${{{}", partial));
                            had_literal = true;
                        }
                    }
                }
                _ => {
                    acc.push(c);
                    self.pos += c.len_utf8();
                    had_literal = true;
                }
            }
        }

        if had_quoted || had_literal || any_var_nonempty {
            Some(Token::Word(acc))
        } else {
            // Pure-empty-unquoted-var word → drop.
            None
        }
    }
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.skip_whitespace();
            if self.pos >= self.input.len() {
                return None;
            }
            let before = self.pos;
            if let Some(tok) = self.next_token() {
                return Some(tok);
            }
            // `next_token` returned None. Two cases:
            //   - an error was recorded (unclosed quote) → terminate.
            //   - a word segment was dropped (empty unquoted var) → retry.
            if self.error.is_some() {
                return None;
            }
            // Defensive: if pos didn't advance, break to avoid infinite loop.
            if self.pos == before {
                return None;
            }
        }
    }
}

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
    fn test_variable_undefined_drops_word() {
        // $NOT_A_VAR alone in an unquoted segment → word drops.
        let mut lexer = Lexer::new("echo $NOT_A_VAR foo");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Word("foo".to_string()),
            ]
        );
    }

    #[test]
    fn test_variable_undefined_with_literal_keeps_word() {
        // "x$NOT_A_VAR" → "x" (literal content keeps the word).
        let mut lexer = Lexer::new("echo x$NOT_A_VAR");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Word("x".to_string()),
            ]
        );
    }

    #[test]
    fn test_quoted_empty_variable_keeps_word() {
        // "$UNDEF" quoted → empty word preserved.
        let mut lexer = Lexer::new("echo \"$NOT_A_VAR\"");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(
            tokens,
            vec![Token::Word("echo".to_string()), Token::Word("".to_string()),]
        );
    }

    #[test]
    fn test_adjacent_word_and_quoted() {
        let mut lexer = Lexer::new("echo x\"y\"z");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Word("xyz".to_string()),
            ]
        );
    }

    #[test]
    fn test_adjacent_literal_and_variable() {
        // env var not set → "x$UNDEF" → "x"
        let mut lexer = Lexer::new("echo x$UNDEFINED_HERE");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Word("echo".to_string()),
                Token::Word("x".to_string()),
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
