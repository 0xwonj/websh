# Phase 2 Track A: Parser — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the shell parser POSIX-correct on three critical edge cases: unclosed quotes emit a syntax error, adjacent word+variable segments concatenate into one argument, and `export` accepts multiple assignments on one line.

**Architecture:** Keep lexer→expand→pipeline three-phase structure. Teach the lexer to (a) detect unclosed quotes and surface them via a new `ParseError::UnclosedQuote`, (b) build a single `Token::Word` from adjacent literal / variable / quoted segments until whitespace or an operator. `Command::Export` becomes `Vec<String>` (one assignment per arg).

**Tech Stack:** Rust 2024, same as prior phases.

**Addresses issues:** C2 (token concat + unclosed quotes), M5 ($UNDEF empty arg), M7 (multi-var export).

## Scope

**In scope:**
- Unclosed `'` or `"` → `ParseError::UnclosedQuote { kind, position }` → `Pipeline.error` set → execute_pipeline returns exit 2.
- `echo foo$BAR` → single arg `"foo<BAR-value>"`.
- `echo x"y"z` → single arg `"xyz"`.
- `echo $UNDEF foo` → argv `["echo", "foo"]` (empty unquoted expansion drops the word).
- `echo "$UNDEF"` → argv `["echo", ""]` (quoted preserves empty).
- `echo x$UNDEF` → argv `["echo", "x"]` (literal content keeps word).
- `export FOO=a BAR=b` → both assignments applied, in order.

**Out of scope (deferred):**
- Backslash escape outside of double quotes (`echo \$HOME`): YAGNI, no current user need.
- Command substitution `$(cmd)`, arithmetic `$((expr))`, glob `*`: YAGNI.
- `export FOO` (show single var) — remains as-is; only multi-arg parsing changes.

## File Structure

| Path | Responsibility | Action |
|---|---|---|
| `src/core/parser/lexer.rs` | Unclosed-quote error + word coalescing + empty-var drop | Modify |
| `src/core/parser/mod.rs` | `ParseError::UnclosedQuote` variant + propagate lexer error to Pipeline | Modify |
| `src/core/parser/expand.rs` | Remove `Token::Variable` handling (now done in lexer) if applicable | Modify |
| `src/core/commands/mod.rs` | `Command::Export(Vec<String>)` | Modify |
| `src/core/commands/execute.rs` | `execute_export` iterates multiple assignments | Modify |

No new files.

---

## Task A.1: Unclosed-quote detection

**Files:**
- Modify: `src/core/parser/mod.rs` (add enum variant)
- Modify: `src/core/parser/lexer.rs` (set error on EOF inside quote)

### Design

Add `ParseError::UnclosedQuote { kind: char, position: usize }` (`kind` is `'` or `"`). Lexer grows a `pub fn error(&self) -> Option<&ParseError>` method + private `error` field. On EOF inside `parse_single_quoted` / `parse_double_quoted`, set `self.error = Some(...)` and return `None` (terminating iteration). `parse_input` checks the error after tokenizing and returns a pipeline with error set.

### Steps

- [ ] **Step 1: Write failing tests**

Add to `src/core/parser/mod.rs`'s test module:

```rust
    #[test]
    fn test_unclosed_single_quote() {
        let pipeline = parse_input("echo 'hello", &[]);
        assert!(pipeline.has_error());
        assert!(matches!(
            pipeline.error,
            Some(ParseError::UnclosedQuote { kind: '\'', .. })
        ));
    }

    #[test]
    fn test_unclosed_double_quote() {
        let pipeline = parse_input("echo \"world", &[]);
        assert!(pipeline.has_error());
        assert!(matches!(
            pipeline.error,
            Some(ParseError::UnclosedQuote { kind: '"', .. })
        ));
    }

    #[test]
    fn test_closed_quotes_ok() {
        let pipeline = parse_input("echo 'hi'", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands[0].args, vec!["hi"]);
    }
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --bin websh core::parser::tests::test_unclosed
```

Expected: tests fail (compile error: `UnclosedQuote` variant missing).

- [ ] **Step 3: Add variant + Display impl**

In `src/core/parser/mod.rs`:

```rust
pub enum ParseError {
    UnexpectedPipe { position: usize },
    EmptyPipeStage { position: usize },
    TrailingPipe { position: usize },
    /// Unclosed single or double quote starting at `position`.
    UnclosedQuote { kind: char, position: usize },
}
```

Add Display arm:
```rust
            Self::UnclosedQuote { kind, position } => {
                write!(f, "syntax error: unclosed {} quote starting at position {}",
                    if *kind == '"' { "double" } else { "single" },
                    position)
            }
```

- [ ] **Step 4: Surface error from Lexer**

In `src/core/parser/lexer.rs`:

```rust
pub struct Lexer<'a> {
    input: &'a str,
    pos: usize,
    error: Option<super::ParseError>,
}
```

Initialize `error: None` in `new()`. Add accessor:
```rust
pub fn error(&self) -> Option<&super::ParseError> {
    self.error.as_ref()
}
```

Modify `parse_single_quoted` — when the `while` loop exits without finding the closing `'`, set error and stop:

```rust
    fn parse_single_quoted(&mut self) -> Option<Token> {
        let quote_start = self.pos;
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

        self.error = Some(super::ParseError::UnclosedQuote {
            kind: '\'',
            position: quote_start,
        });
        None
    }
```

Apply the analogous change to `parse_double_quoted`. Once `None` is returned from `parse_single_quoted` / `parse_double_quoted`, `next_token` returns `None` and the iterator stops. Tests/callers then check `lexer.error()`.

- [ ] **Step 5: Propagate in `parse_input`**

In `src/core/parser/mod.rs::parse_input`:

```rust
pub fn parse_input(input: &str, history: &[String]) -> Pipeline {
    let mut lexer = Lexer::new(input);
    let tokens: Vec<Token> = (&mut lexer).collect();

    if let Some(err) = lexer.error().cloned() {
        return Pipeline {
            commands: vec![],
            error: Some(err),
        };
    }

    let expanded = expand_tokens(tokens, history);
    parse_pipeline(expanded)
}
```

Note: `Lexer::tokenize(self)` consumes self, which won't let us check `error()` afterward. Replace call site to `(&mut lexer).collect()` as shown. If other callers of `tokenize` exist (see `src/core/parser/expand.rs` and `autocomplete.rs`), they get unchanged behavior because they re-tokenize history-expanded strings which were previously validated.

- [ ] **Step 6: Run tests**

```bash
cargo test --bin websh core::parser
```

Expected: all 3 new tests pass, existing parser tests still pass.

- [ ] **Step 7: Full test suite**

```bash
cargo test --bin websh
```

Expected: 170+3 = 173 pass / 4 pre-existing fail.

- [ ] **Step 8: Commit**

```bash
git add src/core/parser/mod.rs src/core/parser/lexer.rs
git commit -m "feat(parser): detect unclosed quotes, return ParseError::UnclosedQuote"
```

---

## Task A.2: Word coalescing + empty unquoted-var drop

**Files:**
- Modify: `src/core/parser/lexer.rs` (major: word building)
- Modify: `src/core/parser/expand.rs` (remove Variable-handling path if it was inlined into lexer)

### Design

Rewrite `parse_word` to accumulate **segments** until whitespace / `|` / `!` (for history) is reached. Each segment contributes to one final `Token::Word`. Segments:
- Plain literal characters: append as-is.
- `$VAR` / `${VAR}`: look up via `env::get_user_var` and append the value. Track whether this expansion produced a non-empty string. If the variable is undefined, the expansion is empty.
- `"..."` / `'...'`: parse inline (existing helpers), append inner content. Set a `had_quoted_segment` flag.

At the end of a word, decide whether to emit:
- If `had_quoted_segment` OR `had_literal_content` OR `any_var_expanded_nonempty` → emit `Token::Word(acc)`.
- Else (word was entirely empty-unquoted-var expansions) → don't emit — the word disappears from argv.

`Token::Variable` is no longer emitted by the lexer (replaced by inline expansion). Delete `Token::Variable` from the enum. `expand_tokens` simplifies to only handle history.

### Steps

- [ ] **Step 1: Write failing tests**

In `src/core/parser/lexer.rs::tests` — but note that lexer no longer emits `Variable`, so test structure changes. Replace `test_variable` and `test_variable_braces` with versions asserting the final Word value. Since `env::get_user_var` has side effects (reads localStorage in wasm), for native tests we need a way to seed vars. If seeding is infeasible in tests, assert the undefined-var path:

```rust
    #[test]
    fn test_variable_undefined_drops_word() {
        // $NOT_A_VAR alone in an unquoted segment → word drops.
        let mut lexer = Lexer::new("echo $NOT_A_VAR foo");
        let tokens: Vec<_> = (&mut lexer).collect();
        // Expected: "echo" and "foo" only (the $NOT_A_VAR word is dropped).
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("foo".to_string()),
        ]);
    }

    #[test]
    fn test_variable_undefined_with_literal_keeps_word() {
        // "x$NOT_A_VAR" → "x" (literal content keeps the word).
        let mut lexer = Lexer::new("echo x$NOT_A_VAR");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("x".to_string()),
        ]);
    }

    #[test]
    fn test_quoted_empty_variable_keeps_word() {
        // "$UNDEF" quoted → empty word preserved.
        let mut lexer = Lexer::new("echo \"$NOT_A_VAR\"");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("".to_string()),
        ]);
    }

    #[test]
    fn test_adjacent_word_and_quoted() {
        let mut lexer = Lexer::new("echo x\"y\"z");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("xyz".to_string()),
        ]);
    }

    #[test]
    fn test_adjacent_literal_and_variable() {
        // env var not set → "x$UNDEF" → "x"
        let mut lexer = Lexer::new("echo x$UNDEFINED_HERE");
        let tokens: Vec<_> = (&mut lexer).collect();
        assert_eq!(tokens, vec![
            Token::Word("echo".to_string()),
            Token::Word("x".to_string()),
        ]);
    }
```

Delete obsolete tests: `test_variable`, `test_variable_braces` (they asserted `Token::Variable` emission, which no longer happens).

Also add integration tests in `parser/mod.rs`:

```rust
    #[test]
    fn test_unquoted_undef_drops_argv_slot() {
        let pipeline = parse_input("echo $NO_SUCH_VAR hello", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands[0].name, "echo");
        // $NO_SUCH_VAR is unquoted and empty → the word disappears.
        assert_eq!(pipeline.commands[0].args, vec!["hello"]);
    }

    #[test]
    fn test_quoted_undef_keeps_empty_arg() {
        let pipeline = parse_input("echo \"$NO_SUCH_VAR\" hello", &[]);
        assert!(!pipeline.has_error());
        assert_eq!(pipeline.commands[0].args, vec!["", "hello"]);
    }
```

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --bin websh core::parser
```

Expected: many failures (existing tests for `Token::Variable` fail; new concat tests fail).

- [ ] **Step 3: Rewrite word parsing in lexer**

Rewrite `parse_word` / `parse_word_with_prefix` (around lines 255-276) to use a segment accumulator:

```rust
/// Parse a word (possibly spanning multiple segments).
///
/// A word accumulates until whitespace, `|`, or `!` (which may start history
/// expansion). Segments include plain literals, `$VAR`/`${VAR}` expansions,
/// and `"..."`/`'...'` quoted strings.
///
/// If the word is composed entirely of empty unquoted-variable expansions
/// (e.g., `$UNDEF` alone), it is dropped from the output.
fn parse_word_segment(&mut self) -> Option<Token> {
    let mut acc = String::new();
    let mut had_quoted = false;
    let mut had_literal = false;
    let mut any_var_nonempty = false;

    loop {
        if self.pos >= self.input.len() { break; }
        let c = self.current_char();
        if c.is_whitespace() || c == '|' || c == '!' { break; }

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
                    if cc == '"' { closed = true; break; }
                    else if cc == '\\' && self.pos < self.input.len() {
                        let escaped = self.current_char();
                        self.pos += escaped.len_utf8();
                        match escaped {
                            'n' => acc.push('\n'),
                            't' => acc.push('\t'),
                            _ => acc.push(escaped),
                        }
                    } else if cc == '$' && self.pos < self.input.len() {
                        // inline var expansion (quoted context preserves emptiness)
                        match self.read_variable_name() {
                            VariableRead::Name(name) => {
                                if let Some(v) = crate::core::env::get_user_var(&name) {
                                    acc.push_str(&v);
                                }
                                // quoted $UNDEF → nothing, but word still kept
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
                match self.read_variable_name() {
                    VariableRead::Name(name) => {
                        if let Some(v) = crate::core::env::get_user_var(&name) {
                            if !v.is_empty() {
                                acc.push_str(&v);
                                any_var_nonempty = true;
                            }
                        }
                        // else: unquoted empty var — contributes nothing, doesn't count as content
                    }
                    VariableRead::Empty => {
                        // bare `$` → literal $
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

    // Decide whether to emit.
    if had_quoted || had_literal || any_var_nonempty || !acc.is_empty() {
        Some(Token::Word(acc))
    } else {
        // Pure-empty-unquoted-var word → drop.
        None
    }
}
```

Change `next_token` to dispatch `'$'`, `'"'`, `'\''`, and default chars into `parse_word_segment`:

```rust
fn next_token(&mut self) -> Option<Token> {
    let c = self.current_char();

    match c {
        '|' => { self.pos += 1; Some(Token::Pipe) }
        '!' => self.parse_history(),
        _ => self.parse_word_segment(),
    }
}
```

The old `parse_variable`, `parse_double_quoted`, `parse_single_quoted`, `parse_word`, `parse_word_with_prefix` are now dead — delete them. Move their logic into `parse_word_segment`.

Exception: `parse_history` returns `!!` / `!n` etc. When `parse_history` decides the `!` is not a history operator, it currently calls `parse_word_with_prefix("!")`. Replace that call by having `parse_history` rewind `self.pos` and call `parse_word_segment()` with a pre-seeded `"!"` prefix — or simpler, let the segment accumulator start with the `!` literal. Inline the logic:

```rust
fn parse_history(&mut self) -> Option<Token> {
    let hist_start = self.pos;
    self.pos += 1; // skip first !
    if self.pos >= self.input.len() { return Some(Token::Word("!".to_string())); }
    if self.current_char() == '!' { self.pos += 1; return Some(Token::HistoryLast); }

    let num_start = self.pos;
    if self.current_char() == '-' { self.pos += 1; }
    while self.pos < self.input.len() {
        let c = self.current_char();
        if !c.is_ascii_digit() { break; }
        self.pos += 1;
    }

    if let Ok(n) = self.input[num_start..self.pos].parse::<i32>() {
        Some(Token::HistoryIndex(n))
    } else {
        // Rewind: `!` was not a history marker, treat as part of a word.
        self.pos = hist_start;
        // But we need to consume the `!` so we don't loop forever — push literal `!`
        // then continue the segment.
        self.pos += 1; // skip !
        let mut word = String::from("!");
        // continue accumulating using segment logic
        // Simplest: delegate to parse_word_segment which won't re-match `!` at this position
        //   since pos is already past it.
        if let Some(Token::Word(rest)) = self.parse_word_segment() {
            word.push_str(&rest);
        }
        Some(Token::Word(word))
    }
}
```

- [ ] **Step 4: Remove `Token::Variable` variant**

In `src/core/parser/lexer.rs`, delete the `Variable(String)` variant. Fix any match statements that referenced it (in `src/core/parser/expand.rs`).

- [ ] **Step 5: Simplify `expand_tokens`**

In `src/core/parser/expand.rs`, remove the `Token::Variable` arm. It becomes:

```rust
pub fn expand_tokens(tokens: Vec<Token>, history: &[String]) -> Vec<Token> {
    tokens
        .into_iter()
        .flat_map(|token| match token {
            Token::HistoryLast => { /* unchanged */ }
            Token::HistoryIndex(n) => { /* unchanged */ }
            other => vec![other],
        })
        .collect()
}
```

The Variable-test at the bottom can be removed; existing HistoryLast / HistoryIndex tests stay.

- [ ] **Step 6: Fix autocomplete if it references `Token::Variable`**

Run `grep -rn "Token::Variable" src/` — any remaining reference must be removed.

- [ ] **Step 7: Run tests**

```bash
cargo test --bin websh
```

Expected: all parser tests pass including new ones. Total: ~179 pass (173 post-A.1 + 6 new-ish; some obsolete tests removed). Accept ±5 variance depending on how old tests were modified.

- [ ] **Step 8: Commit**

```bash
git add src/core/parser/
git commit -m "feat(parser): coalesce adjacent segments into one word, drop empty unquoted vars"
```

---

## Task A.3: Multi-var export

**Files:**
- Modify: `src/core/commands/mod.rs` (`Command::Export` variant shape + parse)
- Modify: `src/core/commands/execute.rs` (`execute_export` processes Vec)

### Design

`Command::Export(Vec<String>)` — each element is one raw assignment (e.g., `"FOO=a"`) or a single variable name (for display). Parse arm just clones `args` into the Vec. `execute_export` iterates: if empty Vec, show all; else process each assignment.

### Steps

- [ ] **Step 1: Write failing tests**

In `src/core/commands/mod.rs::tests`:

```rust
    #[test]
    fn test_parse_export_multi() {
        assert!(matches!(
            Command::parse("export", &args(&["FOO=a", "BAR=b"])),
            Command::Export(ref v) if v.len() == 2 && v[0] == "FOO=a" && v[1] == "BAR=b"
        ));
    }
```

Update existing `test_parse_export`:

```rust
    #[test]
    fn test_parse_export() {
        assert!(matches!(
            Command::parse("export", &[]),
            Command::Export(ref v) if v.is_empty()
        ));
        assert!(matches!(
            Command::parse("export", &args(&["FOO=bar"])),
            Command::Export(ref v) if v.len() == 1 && v[0] == "FOO=bar"
        ));
    }
```

In `src/core/commands/execute.rs::tests`, add:

```rust
    #[test]
    fn test_execute_export_multi_assigns_all() {
        let (ts, ws, fs) = empty_state();
        let result = execute_command(
            Command::Export(vec!["FOO_P2_A=alpha".to_string(), "BAR_P2_A=beta".to_string()]),
            &ts, &ws, &fs, &AppRoute::Root,
        );
        assert_eq!(result.exit_code, 0);
        // Verify both set
        assert_eq!(crate::core::env::get_user_var("FOO_P2_A"), Some("alpha".to_string()));
        assert_eq!(crate::core::env::get_user_var("BAR_P2_A"), Some("beta".to_string()));
        // Cleanup
        let _ = crate::core::env::unset_user_var("FOO_P2_A");
        let _ = crate::core::env::unset_user_var("BAR_P2_A");
    }
```

Note: env vars persist via localStorage in wasm; on native tests they may use an in-memory HashMap — verify behavior, adapt test cleanup as needed.

- [ ] **Step 2: Run to verify failure**

```bash
cargo test --bin websh core::commands::tests::test_parse_export
```

Expected: fail (old `Export(Option<String>)` shape doesn't match).

- [ ] **Step 3: Change `Command::Export` shape**

In `src/core/commands/mod.rs`:

```rust
pub enum Command {
    ...
    Export(Vec<String>),
    ...
}
```

Update `Command::parse`:

```rust
            "export" => Self::Export(args.to_vec()),
```

- [ ] **Step 4: Update `execute_export`**

In `src/core/commands/execute.rs`, replace `execute_export`:

```rust
fn execute_export(assignments: Vec<String>) -> CommandResult {
    if assignments.is_empty() {
        // No args: show all variables
        let lines = env::format_export_output();
        let mut output = vec![OutputLine::empty()];
        for line in lines {
            output.push(OutputLine::text(line));
        }
        output.push(OutputLine::empty());
        return CommandResult::output(output);
    }

    // Process each assignment sequentially. Accumulate errors; first error sets exit_code.
    let mut output: Vec<OutputLine> = Vec::new();
    let mut exit_code = 0;
    for arg in assignments {
        if let Some((key, value)) = arg.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if let Err(e) = env::set_user_var(key, value) {
                output.push(OutputLine::error(format!("export: {}", e)));
                if exit_code == 0 { exit_code = 1; }
            }
        } else {
            // Just a key without value — show current value
            let key = arg.trim();
            if let Some(value) = env::get_user_var(key) {
                output.push(OutputLine::text(format!("{}={}", key, value)));
            }
            // silent if not set (matches prior behavior)
        }
    }

    CommandResult::output(output).with_exit_code(exit_code)
}
```

Update the match arm in `execute_command`:

```rust
        Command::Export(assignments) => execute_export(assignments),
```

- [ ] **Step 5: Run tests**

```bash
cargo test --bin websh
```

Expected: all previously-passing tests still pass; new export tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/core/commands/mod.rs src/core/commands/execute.rs
git commit -m "fix(export): accept multiple assignments on one line (export FOO=a BAR=b)"
```

---

## Self-Review Checklist

- [x] Spec coverage: C2 (unclosed quotes) → A.1; C2 (concat) + M5 (empty var drop) → A.2; M7 (multi-var export) → A.3.
- [x] No placeholders: every code block is concrete.
- [x] `Token::Variable` removal is explicit and propagated.
- [x] Empty-word-drop logic aligns with POSIX shell behavior.

## Done Criteria

- `cargo test --bin websh`: ~180 pass / 4 pre-existing fail.
- `cargo build --release --target wasm32-unknown-unknown`: clean.
- `echo 'hi` emits syntax error.
- `echo foo$UNDEF` → one arg `"foo"`.
- `echo $UNDEF hi` → one arg `"hi"`.
- `export A=1 B=2` sets both.
