//! Lexer for tokenizing shell input.
//!
//! Handles:
//! - Word tokenization
//! - Pipe operator (`|`)
//! - Variable references (`$VAR`, `${VAR}`)
//! - History expansion (`!!`, `!n`, `!-n`)
//! - Quote handling (single and double quotes)

use std::collections::BTreeMap;

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
    env: BTreeMap<String, String>,
    pos: usize,
    error: Option<super::ShellParseError>,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given input
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            env: BTreeMap::new(),
            pos: 0,
            error: None,
        }
    }

    /// Create a new lexer with a target-provided environment snapshot.
    pub fn new_with_env(input: &'a str, env: &BTreeMap<String, String>) -> Self {
        Self {
            input,
            env: env.clone(),
            pos: 0,
            error: None,
        }
    }

    /// Tokenize the entire input into a vector.
    ///
    /// Convenience for callers that don't need lazy evaluation. Consumes the
    /// lexer, so callers that need `.error()` afterwards must use
    /// `(&mut lexer).collect()` instead.
    #[cfg(test)]
    pub fn tokenize(self) -> Vec<Token> {
        self.collect()
    }

    /// Returns a parse error if one was encountered during tokenization
    /// (e.g., an unclosed quote). Callers that need to surface lexer errors
    /// should iterate via `(&mut lexer).collect()` and then check `.error()`.
    pub fn error(&self) -> Option<&super::ShellParseError> {
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
                        self.error = Some(super::ShellParseError::UnclosedQuote {
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
                                    if let Some(value) = self.env.get(&name) {
                                        acc.push_str(value);
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
                        self.error = Some(super::ShellParseError::UnclosedQuote {
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
                            if let Some(v) = self.env.get(&name) {
                                if v.is_empty() {
                                    // empty value: contributes nothing, doesn't
                                    // count as content — preserves empty-drop
                                    // semantics when there's no other content.
                                } else {
                                    acc.push_str(v);
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
    fn tokenizes_words_and_pipe_boundaries() {
        assert_eq!(
            Lexer::new("ls -la /home | grep foo").tokenize(),
            vec![
                Token::Word("ls".into()),
                Token::Word("-la".into()),
                Token::Word("/home".into()),
                Token::Pipe,
                Token::Word("grep".into()),
                Token::Word("foo".into()),
            ],
        );
    }

    #[test]
    fn quoting_and_expansion_preserve_word_boundaries() {
        for (input, words) in [
            ("echo $NOT_A_VAR foo", vec!["echo", "foo"]),
            ("echo x$NOT_A_VAR", vec!["echo", "x"]),
            ("echo \"$NOT_A_VAR\"", vec!["echo", ""]),
            ("echo x\"y\"z", vec!["echo", "xyz"]),
            ("echo 'hello world'", vec!["echo", "hello world"]),
            ("echo \"hello world\"", vec!["echo", "hello world"]),
        ] {
            let expected: Vec<_> = words
                .into_iter()
                .map(|word| Token::Word(word.into()))
                .collect();
            assert_eq!(Lexer::new(input).tokenize(), expected, "{input}");
        }
    }

    #[test]
    fn tokenizes_absolute_and_relative_history_references() {
        for (input, token) in [
            ("!!", Token::HistoryLast),
            ("!5", Token::HistoryIndex(5)),
            ("!-2", Token::HistoryIndex(-2)),
        ] {
            assert_eq!(Lexer::new(input).tokenize(), vec![token], "{input}");
        }
    }
}
