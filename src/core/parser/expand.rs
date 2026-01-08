//! Token expansion for variables and history.
//!
//! Handles:
//! - Variable expansion (`$VAR` → value)
//! - History expansion (`!!` → last command, `!n` → nth command)

use super::lexer::{Lexer, Token};
use crate::core::env;

/// Expand variables and history references in tokens.
pub fn expand_tokens(tokens: Vec<Token>, history: &[String]) -> Vec<Token> {
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

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_expansion() {
        let history = vec!["ls -la".to_string(), "pwd".to_string()];
        let tokens = vec![Token::HistoryLast];
        let expanded = expand_tokens(tokens, &history);
        assert_eq!(expanded, vec![Token::Word("pwd".to_string())]);
    }

    #[test]
    fn test_history_index_expansion() {
        let history = vec!["ls -la".to_string(), "pwd".to_string()];
        let tokens = vec![Token::HistoryIndex(0)];
        let expanded = expand_tokens(tokens, &history);
        assert_eq!(
            expanded,
            vec![
                Token::Word("ls".to_string()),
                Token::Word("-la".to_string()),
            ]
        );
    }
}
